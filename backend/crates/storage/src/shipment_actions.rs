//! Durable provider commands serialized by the existing shipment lease.
use crate::{Shipment, ShipmentError, Storage, StorageError};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Row, postgres::PgRow};
use std::collections::HashSet;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ShipmentAction {
    pub id: String,
    pub shipment_id: String,
    pub kind: String,
    pub request_id: String,
    pub input: Value,
    pub state: String,
    pub result: Value,
    pub created_at: i64,
    pub updated_at: i64,
}
fn action(r: PgRow) -> Result<ShipmentAction, StorageError> {
    Ok(ShipmentAction {
        id: r.try_get("id")?,
        shipment_id: r.try_get("shipment_id")?,
        kind: r.try_get("kind")?,
        request_id: r.try_get("request_id")?,
        input: r.try_get("input")?,
        state: r.try_get("state")?,
        result: r.try_get("result")?,
        created_at: r.try_get("created_at")?,
        updated_at: r.try_get("updated_at")?,
    })
}
fn ids(value: &Value) -> Option<Vec<&str>> {
    let ids: Vec<_> = value
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect::<Option<_>>()?;
    (ids.len() <= 30
        && !ids.is_empty()
        && ids
            .iter()
            .all(|id| !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control))
        && ids.iter().copied().collect::<HashSet<_>>().len() == ids.len())
    .then_some(ids)
}
impl Storage {
    pub async fn shipment_action_authorized(&self, id: &str) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shipment_actions a JOIN users u ON u.id=a.actor_id WHERE a.id=$1 AND u.role='admin' AND u.disabled_at IS NULL)")
            .bind(id).fetch_one(&self.pool).await?)
    }
    /// Resume reconciliation only. `submitting` is never submitted again by the worker.
    pub async fn shipment_action_resume(
        &self,
        order: &str,
        version: i64,
        id: &str,
        admin: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        if !super::shipments::actor(&mut tx, admin).await? {
            return Ok(false);
        }
        sqlx::query("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?;
        let sid: Option<String> = sqlx::query_scalar("SELECT id FROM shipments WHERE order_id=$1 AND version=$2 AND state IN ('booked','manual_review') AND COALESCE(lease_until,0)<=$3 FOR UPDATE")
            .bind(order).bind(version).bind(now).fetch_optional(&mut *tx).await?;
        let Some(sid) = sid else { return Ok(false) };
        let changed = sqlx::query("UPDATE shipment_actions SET state='submitting',updated_at=$3 WHERE id=$1 AND shipment_id=$2 AND state='manual_review' AND NOT EXISTS(SELECT 1 FROM shipment_actions WHERE shipment_id=$2 AND state IN ('queued','submitting','pending'))")
            .bind(id).bind(&sid).bind(now).execute(&mut *tx).await?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE shipments SET version=version+1,attempts=0,next_attempt=$2,updated_at=$2 WHERE id=$1").bind(sid).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    pub async fn shipment_actions(&self, order: &str) -> Result<Vec<ShipmentAction>, StorageError> {
        sqlx::query("SELECT a.* FROM shipment_actions a JOIN shipments s ON s.id=a.shipment_id WHERE s.order_id=$1 ORDER BY a.created_at,a.id")
            .bind(order).fetch_all(&self.pool).await?.into_iter().map(action).collect()
    }
    pub async fn shipment_action_busy(&self, order: &str) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shipment_actions a JOIN shipments s ON s.id=a.shipment_id WHERE s.order_id=$1 AND a.state IN ('queued','submitting','pending','manual_review'))")
            .bind(order).fetch_one(&self.pool).await?)
    }
    pub async fn shipment_action_active(
        &self,
        s: &Shipment,
    ) -> Result<Option<ShipmentAction>, StorageError> {
        sqlx::query("SELECT * FROM shipment_actions WHERE shipment_id=$1 AND state IN ('queued','submitting','pending')")
            .bind(&s.id).fetch_optional(&self.pool).await?.map(action).transpose()
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn shipment_action_enqueue(
        &self,
        order: &str,
        version: i64,
        kind: &str,
        request_id: &str,
        input: &Value,
        admin: &str,
        now: i64,
    ) -> Result<Result<ShipmentAction, ShipmentError>, StorageError> {
        let Ok(request) = uuid::Uuid::parse_str(request_id) else {
            return Ok(Err(ShipmentError::Invalid));
        };
        let request = request.to_string();
        let (Some(local), Some(provider)) =
            (ids(&input["package_ids"]), ids(&input["provider_ids"]))
        else {
            return Ok(Err(ShipmentError::Invalid));
        };
        if !matches!(kind, "pickup" | "cancel")
            || input.to_string().len() > 32768
            || local.len() != provider.len()
        {
            return Ok(Err(ShipmentError::Invalid));
        }
        let mut tx = self.pool.begin().await?;
        if !super::shipments::actor(&mut tx, admin).await? {
            return Ok(Err(ShipmentError::Forbidden));
        }
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
                .bind(order)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Ok(Err(ShipmentError::NotFound));
        }
        let Some(parent) = sqlx::query("SELECT * FROM shipments WHERE order_id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(Err(ShipmentError::NotFound));
        };
        let shipment_id: String = parent.try_get("id")?;
        if let Some(row) = sqlx::query("SELECT * FROM shipment_actions WHERE request_id=$1")
            .bind(&request)
            .fetch_optional(&mut *tx)
            .await?
        {
            let a = action(row)?;
            return Ok(
                if a.shipment_id == shipment_id && a.kind == kind && a.input == *input {
                    Ok(a)
                } else {
                    Err(ShipmentError::Conflict)
                },
            );
        }
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM shipment_actions WHERE shipment_id=$1")
                .bind(&shipment_id)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 100 {
            return Ok(Err(ShipmentError::Conflict));
        }
        let parent_state: String = parent.try_get("state")?;
        let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shipment_actions WHERE shipment_id=$1 AND state IN ('queued','submitting','pending','manual_review'))")
            .bind(&shipment_id).fetch_one(&mut *tx).await?;
        if parent.try_get::<i64, _>("version")? != version
            || parent
                .try_get::<Option<i64>, _>("lease_until")?
                .is_some_and(|t| t > now)
            || busy
            || !(parent_state == "booked" || (kind == "cancel" && parent_state == "manual_review"))
        {
            return Ok(Err(ShipmentError::Conflict));
        }
        let selected: Vec<String> = local.iter().map(|s| (*s).to_owned()).collect();
        let rows = sqlx::query("SELECT id,provider_id,state FROM shipment_packages WHERE shipment_id=$1 AND id=ANY($2)")
            .bind(&shipment_id).bind(&selected).fetch_all(&mut *tx).await?;
        if rows.len() != local.len() {
            return Ok(Err(ShipmentError::Invalid));
        }
        for row in rows {
            let id: String = row.try_get("id")?;
            let Some(index) = local.iter().position(|local| *local == id) else {
                return Ok(Err(ShipmentError::Invalid));
            };
            if row.try_get::<String, _>("state")? != "booked"
                || row.try_get::<Option<String>, _>("provider_id")?.as_deref()
                    != Some(provider[index])
            {
                return Ok(Err(ShipmentError::Conflict));
            }
        }
        let row = sqlx::query("INSERT INTO shipment_actions(id,shipment_id,kind,request_id,input,state,actor_id,created_at,updated_at) VALUES($1,$2,$3,$4,$5,'queued',$6,$7,$7) ON CONFLICT(request_id) DO NOTHING RETURNING *")
            .bind(uuid::Uuid::new_v4().to_string()).bind(&shipment_id).bind(kind).bind(request).bind(input).bind(admin).bind(now).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(Err(ShipmentError::Conflict));
        };
        sqlx::query("UPDATE shipments SET version=version+1,attempts=0,next_attempt=$2,updated_at=$2 WHERE id=$1")
            .bind(&shipment_id).bind(now).execute(&mut *tx).await?;
        let result = action(row)?;
        tx.commit().await?;
        Ok(Ok(result))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn shipment_action_update(
        &self,
        s: &Shipment,
        id: &str,
        state: &str,
        result: &Value,
        canceled_local_ids: &[String],
        now: i64,
    ) -> Result<bool, StorageError> {
        if result.to_string().len() > 262144 || canceled_local_ids.len() > 30 {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
            .bind(&s.order_id)
            .fetch_optional(&mut *tx)
            .await?;
        let current: Option<String> = sqlx::query_scalar(
            "SELECT id FROM shipments WHERE id=$1 AND lease_token=$2 AND lease_until>$3 FOR UPDATE",
        )
        .bind(&s.id)
        .bind(&s.lease_token)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        if current.is_none() {
            return Ok(false);
        }
        let Some(row) =
            sqlx::query("SELECT * FROM shipment_actions WHERE id=$1 AND shipment_id=$2 FOR UPDATE")
                .bind(id)
                .bind(&s.id)
                .fetch_optional(&mut *tx)
                .await?
        else {
            return Ok(false);
        };
        let a = action(row)?;
        let allowed = matches!(
            (a.state.as_str(), state),
            ("queued", "submitting" | "failed" | "manual_review")
                | (
                    "submitting",
                    "submitting" | "pending" | "succeeded" | "partial" | "failed" | "manual_review"
                )
                | (
                    "pending",
                    "pending" | "succeeded" | "partial" | "failed" | "manual_review"
                )
        );
        let Some(selected) = ids(&a.input["package_ids"]) else {
            return Ok(false);
        };
        if !allowed
            || (!canceled_local_ids.is_empty()
                && (a.kind != "cancel"
                    || !matches!(state, "pending" | "succeeded" | "partial")
                    || canceled_local_ids
                        .iter()
                        .any(|id| !selected.contains(&id.as_str()))
                    || canceled_local_ids.iter().collect::<HashSet<_>>().len()
                        != canceled_local_ids.len()))
        {
            return Ok(false);
        }
        sqlx::query("UPDATE shipment_actions SET state=$2,result=$3,updated_at=$4 WHERE id=$1")
            .bind(id)
            .bind(state)
            .bind(result)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        if !canceled_local_ids.is_empty() {
            sqlx::query("UPDATE shipment_packages SET state='canceled',updated_at=$3 WHERE shipment_id=$1 AND id=ANY($2) AND state IN ('booked','canceled')")
                .bind(&s.id).bind(canceled_local_ids).bind(now).execute(&mut *tx).await?;
        }
        if a.kind == "cancel" {
            // A canceled subset cannot fulfill the original order automatically.
            sqlx::query("UPDATE shipments SET state=CASE WHEN NOT EXISTS(SELECT 1 FROM shipment_packages WHERE shipment_id=$1 AND state<>'canceled') AND $3 IN ('succeeded','partial') THEN 'canceled' ELSE 'manual_review' END,last_error='shipment_canceled',updated_at=$2 WHERE id=$1 AND EXISTS(SELECT 1 FROM shipment_packages WHERE shipment_id=$1 AND state='canceled')")
                .bind(&s.id).bind(now).bind(state).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn shipment_actions_are_idempotent_and_cancellation_cannot_revive_parcels() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("rapidmaker_test")
        );
        let db = Storage::connect(&url).await.unwrap();
        let now = 100000;
        let admin = uuid::Uuid::new_v4().to_string();
        let order = uuid::Uuid::new_v4().to_string();
        let sid = uuid::Uuid::new_v4().to_string();
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,'admin',$3)")
            .bind(&admin)
            .bind(format!("{admin}@example.invalid"))
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO orders(id,owner_hash,contact,lines,amount_grosze,provider_kind,provider_id,status,guest_updates,created_at,paid_at,fulfillment) VALUES($1,$2,$3,'[]',10000,'intent',$4,'ready',true,$5,$5,'{}')")
            .bind(&order).bind("a".repeat(64)).bind(json!({"email":"void@sieciowiec.xyz"})).bind(format!("pi_{}",order.replace('-',""))).bind(now).execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO shipments(id,order_id,version,state,input,command_uuid,actor_id,next_attempt,created_at,updated_at) VALUES($1,$2,1,'booked','{}',$3,$4,$5,$5,$5)")
            .bind(&sid).bind(&order).bind(uuid::Uuid::new_v4().to_string()).bind(&admin).bind(now).execute(&db.pool).await.unwrap();
        for (id, provider) in [(&first, "provider-1"), (&second, "provider-2")] {
            sqlx::query("INSERT INTO shipment_packages(id,shipment_id,reference,provider_id,state,payload,tracking,updated_at) VALUES($1,$2,$3,$4,'booked','{}',$5,$6)")
                .bind(id).bind(&sid).bind(format!("RM{}",&id[..20])).bind(provider).bind(json!({"numbers":["TRACK"]})).bind(now).execute(&db.pool).await.unwrap();
        }
        let request = uuid::Uuid::new_v4().to_string();
        let input =
            json!({"package_ids":[first,second],"provider_ids":["provider-1","provider-2"]});
        let bad = json!({"package_ids":[first,"foreign"],"provider_ids":["provider-1","foreign"]});
        assert_eq!(
            db.shipment_action_enqueue(&order, 1, "cancel", &request, &bad, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Invalid
        );
        assert_eq!(
            db.shipment_action_enqueue(&order, 1, "cancel", &request, &input, "unknown", now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Forbidden
        );
        let (a, b) = tokio::join!(
            db.shipment_action_enqueue(&order, 1, "cancel", &request, &input, &admin, now),
            db.shipment_action_enqueue(&order, 1, "cancel", &request, &input, &admin, now)
        );
        let a = a.unwrap().unwrap();
        assert_eq!(a.id, b.unwrap().unwrap().id);
        assert_eq!(db.shipment_get(&order).await.unwrap().unwrap().version, 2);
        assert!(db.shipment_action_busy(&order).await.unwrap());
        assert_eq!(db.shipment_actions(&order).await.unwrap().len(), 1);
        assert_eq!(
            db.shipment_action_enqueue(&order, 2, "pickup", &request, &input, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Conflict
        );
        assert_eq!(
            db.shipment_handover(&order, std::slice::from_ref(&first), &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Conflict
        );
        let mut lease = db.shipment_lease(now).await.unwrap().unwrap();
        assert_eq!(
            db.shipment_action_active(&lease).await.unwrap().unwrap().id,
            a.id
        );
        assert!(
            !db.shipment_action_update(&lease, &a.id, "submitting", &json!({}), &[], now + 120)
                .await
                .unwrap()
        );
        assert!(
            db.shipment_action_update(&lease, &a.id, "submitting", &json!({}), &[], now)
                .await
                .unwrap()
        );
        assert!(db.shipment_action_authorized(&a.id).await.unwrap());
        assert!(
            db.shipment_action_update(
                &lease,
                &a.id,
                "manual_review",
                &json!({"error":"command_not_found"}),
                &[],
                now
            )
            .await
            .unwrap()
        );
        // Reconciliation is read-only recovery, not a new queued provider write.
        assert!(
            !db.shipment_action_resume(&order, 2, &a.id, &admin, now)
                .await
                .unwrap()
        );
        db.shipment_retry(&lease, 1, now).await.unwrap();
        assert!(
            !db.shipment_action_resume(&order, 1, &a.id, &admin, now)
                .await
                .unwrap()
        );
        assert!(
            !db.shipment_action_resume(&order, 2, &a.id, "unknown", now)
                .await
                .unwrap()
        );
        assert!(
            db.shipment_action_resume(&order, 2, &a.id, &admin, now)
                .await
                .unwrap()
        );
        assert_eq!(
            db.shipment_actions(&order).await.unwrap()[0].state,
            "submitting"
        );
        lease = db.shipment_lease(now).await.unwrap().unwrap();
        assert!(
            !db.shipment_action_update(
                &lease,
                &a.id,
                "pending",
                &json!({}),
                &["foreign".into()],
                now
            )
            .await
            .unwrap()
        );
        assert!(
            db.shipment_action_update(
                &lease,
                &a.id,
                "pending",
                &json!({}),
                std::slice::from_ref(&first),
                now
            )
            .await
            .unwrap()
        );
        assert!(
            !db.shipment_tracking(
                &lease,
                &first,
                &json!({"numbers":["TRACK"]}),
                true,
                true,
                now
            )
            .await
            .unwrap()
        );
        assert!(
            db.shipment_action_update(
                &lease,
                &a.id,
                "succeeded",
                &json!({}),
                std::slice::from_ref(&second),
                now
            )
            .await
            .unwrap()
        );
        assert!(!db.shipment_action_busy(&order).await.unwrap());
        let finished = db.shipment_get(&order).await.unwrap().unwrap();
        assert_eq!(finished.state, "canceled");
        assert!(finished.packages.iter().all(|p| p.state == "canceled"));
        assert_eq!(
            db.shipment_handover(&order, &[first, second], &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Conflict
        );
        assert_eq!(
            db.shipment_reopen(&order, 2, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Conflict
        );
        assert!(
            !db.shipment_tracking(
                &lease,
                &finished.packages[0].id,
                &json!({"numbers":["TRACK"]}),
                true,
                true,
                now
            )
            .await
            .unwrap()
        );
        assert!(db.shipment_lease(now + 121).await.unwrap().is_none());
        let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id=$1")
            .bind(&order)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, "ready");
        assert_eq!(
            db.shipment_action_enqueue(&order, 1, "cancel", &request, &input, &admin, now)
                .await
                .unwrap()
                .unwrap()
                .state,
            "succeeded"
        );
    }
}
