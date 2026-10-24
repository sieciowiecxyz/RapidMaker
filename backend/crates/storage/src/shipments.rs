//! Durable shipping commands. A creation timeout needs reconciliation, never another POST.
use crate::{Storage, StorageError};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Shipment {
    pub id: String,
    pub order_id: String,
    pub version: i64,
    pub state: String,
    pub last_error: Option<String>,
    pub input: Value,
    pub quote: Option<Value>,
    pub request_id: Option<String>,
    pub command_uuid: String,
    pub packages: Vec<ShipmentPackage>,
    #[serde(skip)]
    pub lease_token: Option<String>,
    #[serde(skip)]
    pub attempts: i32,
    pub next_attempt: i64,
}
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
pub struct ShipmentPackage {
    pub id: String,
    pub reference: String,
    pub provider_id: Option<String>,
    pub state: String,
    pub payload: Value,
    pub tracking: Value,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ShipmentError {
    NotFound,
    Forbidden,
    Conflict,
    NotReady,
    Refund,
    Invalid,
}
fn shipment(row: PgRow) -> Result<Shipment, StorageError> {
    Ok(Shipment {
        id: row.try_get("id")?,
        order_id: row.try_get("order_id")?,
        version: row.try_get("version")?,
        state: row.try_get("state")?,
        last_error: row.try_get("last_error")?,
        input: row.try_get("input")?,
        quote: row.try_get("quote")?,
        request_id: row.try_get("request_id")?,
        command_uuid: row.try_get("command_uuid")?,
        packages: vec![],
        lease_token: row.try_get("lease_token")?,
        attempts: row.try_get("attempts")?,
        next_attempt: row.try_get("next_attempt")?,
    })
}
fn tracking_numbers(value: &Value) -> Vec<&str> {
    value["numbers"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_else(|| value["number"].as_str().into_iter().collect())
}
async fn eligible_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    s: &Shipment,
) -> Result<Result<(), ShipmentError>, StorageError> {
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shipments s JOIN users u ON u.id=s.actor_id WHERE s.id=$1 AND u.role='admin' AND u.disabled_at IS NULL)").bind(&s.id).fetch_one(&mut **tx).await?;
    if !active {
        return Ok(Err(ShipmentError::Forbidden));
    }
    if let Err(e) = eligible(
        tx,
        &s.order_id,
        s.input["refund_reviewed"].as_bool().unwrap_or(false),
    )
    .await?
    {
        return Ok(Err(e));
    }
    let refunded:i64=sqlx::query_scalar("SELECT COALESCE(sum(amount_grosze),0)::bigint FROM payment_refunds WHERE order_id=$1 AND state='succeeded'").bind(&s.order_id).fetch_one(&mut **tx).await?;
    Ok(
        if s.input["refund_reviewed_grosze"].as_i64() != Some(refunded) {
            Err(ShipmentError::Refund)
        } else {
            Ok(())
        },
    )
}
async fn packages(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<Vec<ShipmentPackage>, StorageError> {
    sqlx::query("SELECT * FROM shipment_packages WHERE shipment_id=$1 ORDER BY reference")
        .bind(id)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|r| {
            Ok(ShipmentPackage {
                id: r.try_get("id")?,
                reference: r.try_get("reference")?,
                provider_id: r.try_get("provider_id")?,
                state: r.try_get("state")?,
                payload: r.try_get("payload")?,
                tracking: r.try_get("tracking")?,
            })
        })
        .collect()
}
async fn actor(tx: &mut Transaction<'_, Postgres>, id: &str) -> Result<bool, StorageError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND role='admin' AND disabled_at IS NULL)",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?)
}
async fn eligible(
    tx: &mut Transaction<'_, Postgres>,
    order: &str,
    reviewed: bool,
) -> Result<Result<PgRow, ShipmentError>, StorageError> {
    let Some(row) = sqlx::query("SELECT * FROM orders WHERE id=$1 FOR UPDATE")
        .bind(order)
        .fetch_optional(&mut **tx)
        .await?
    else {
        return Ok(Err(ShipmentError::NotFound));
    };
    if row.try_get::<String, _>("status")? != "ready"
        || row.try_get::<Option<i64>, _>("paid_at")?.is_none()
    {
        return Ok(Err(ShipmentError::NotReady));
    }
    let Some(f): Option<Value> = row.try_get("fulfillment")? else {
        return Ok(Err(ShipmentError::Invalid));
    };
    if !matches!(
        f["delivery"]["method"]
            .as_str()
            .or_else(|| f["delivery"].as_str()),
        Some("courier" | "locker")
    ) {
        return Ok(Err(ShipmentError::Invalid));
    }
    let (uncertain,refunded):(bool,i64)=sqlx::query_as("SELECT COALESCE(bool_or(state NOT IN ('succeeded','failed','canceled')),false),COALESCE(sum(amount_grosze) FILTER(WHERE state='succeeded'),0)::bigint FROM payment_refunds WHERE order_id=$1").bind(order).fetch_one(&mut **tx).await?;
    if uncertain
        || refunded >= row.try_get::<i64, _>("amount_grosze")?
        || (refunded > 0 && !reviewed)
    {
        return Ok(Err(ShipmentError::Refund));
    }
    Ok(Ok(row))
}
fn parcels_valid(v: &Value) -> bool {
    v.as_array().is_some_and(|a| {
        !a.is_empty()
            && a.len() <= 30
            && a.iter().all(|p| {
                p.as_object().is_some_and(|o| o.len() == 2)
                    && p["outer_mm"].as_array().is_some_and(|d| {
                        d.len() == 3
                            && d.iter()
                                .all(|n| n.as_u64().is_some_and(|x| x > 0 && x <= 3000))
                    })
                    && p["weight_g"].as_u64().is_some_and(|x| x > 0 && x <= 100000)
            })
    })
}
impl Storage {
    pub async fn shipment_source(&self, order: &str) -> Result<Option<Value>, StorageError> {
        Ok(sqlx::query_scalar("SELECT jsonb_build_object('order_id',id,'status',status,'paid_at',paid_at,'contact',contact,'fulfillment',fulfillment) FROM orders WHERE id=$1").bind(order).fetch_optional(&self.pool).await?)
    }
    pub async fn shipment_problem(
        &self,
        s: &Shipment,
        code: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if code.is_empty()
            || code.len() > 80
            || !code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE shipments SET state='manual_review',last_error=$3,lease_token=NULL,lease_until=NULL,next_attempt=$4+900,updated_at=$4 WHERE id=$1 AND lease_token=$2 AND lease_until>$4 AND state<>'delivered'").bind(&s.id).bind(&s.lease_token).bind(code).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn shipment_get(&self, order: &str) -> Result<Option<Shipment>, StorageError> {
        let mut tx = self.pool.begin().await?;
        let Some(row) = sqlx::query("SELECT * FROM shipments WHERE order_id=$1")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(None);
        };
        let mut s = shipment(row)?;
        s.packages = packages(&mut tx, &s.id).await?;
        tx.commit().await?;
        Ok(Some(s))
    }
    pub async fn shipment_save(
        &self,
        order: &str,
        version: i64,
        parcels: &Value,
        refund_reviewed: bool,
        admin: &str,
        now: i64,
    ) -> Result<Result<Shipment, ShipmentError>, StorageError> {
        if !parcels_valid(parcels) || version < 0 {
            return Ok(Err(ShipmentError::Invalid));
        }
        let mut tx = self.pool.begin().await?;
        if !actor(&mut tx, admin).await? {
            return Ok(Err(ShipmentError::Forbidden));
        }
        let row = match eligible(&mut tx, order, refund_reviewed).await? {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        let refunded:i64=sqlx::query_scalar("SELECT COALESCE(sum(amount_grosze),0)::bigint FROM payment_refunds WHERE order_id=$1 AND state='succeeded'").bind(order).fetch_one(&mut *tx).await?;
        let input = json!({"contact":row.try_get::<Value,_>("contact")?,"fulfillment":row.try_get::<Value,_>("fulfillment")?,"parcels":parcels,"refund_reviewed":refund_reviewed,"refund_reviewed_grosze":refunded});
        let id = uuid::Uuid::new_v4().to_string();
        let command = uuid::Uuid::new_v4().to_string();
        let row = if version == 0 {
            sqlx::query("INSERT INTO shipments(id,order_id,version,state,input,command_uuid,actor_id,next_attempt,created_at,updated_at) VALUES($1,$2,1,'draft',$3,$4,$5,$6,$6,$6) ON CONFLICT(order_id) DO NOTHING RETURNING *").bind(id).bind(order).bind(input).bind(command).bind(admin).bind(now).fetch_optional(&mut *tx).await?
        } else {
            sqlx::query("UPDATE shipments SET input=$3,version=version+1,quote=NULL,actor_id=$4,updated_at=$5 WHERE order_id=$1 AND version=$2 AND state='draft' RETURNING *").bind(order).bind(version).bind(input).bind(admin).bind(now).fetch_optional(&mut *tx).await?
        };
        let Some(row) = row else {
            return Ok(Err(ShipmentError::Conflict));
        };
        let s = shipment(row)?;
        tx.commit().await?;
        Ok(Ok(s))
    }
}
impl Storage {
    pub async fn shipment_quote_save(
        &self,
        order: &str,
        version: i64,
        quote: &Value,
        now: i64,
    ) -> Result<bool, StorageError> {
        let valid = quote.to_string().len() <= 262144
            && quote["id"]
                .as_str()
                .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
            && quote["expires_at"]
                .as_i64()
                .is_some_and(|t| t > now && t <= now + 120)
            && quote["price_grosze"]
                .as_i64()
                .is_some_and(|p| (1..=99999999).contains(&p))
            && quote["packages"].as_array().is_some_and(|p| {
                !p.is_empty()
                    && p.len() <= 30
                    && p.iter()
                        .all(|v| v["body"].is_object() && v["body"].to_string().len() <= 32768)
            });
        if !valid {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE shipments SET quote=$3,updated_at=$4 WHERE order_id=$1 AND version=$2 AND state='draft'").bind(order).bind(version).bind(quote).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn shipment_book(
        &self,
        order: &str,
        version: i64,
        request_id: &str,
        quote_id: &str,
        admin: &str,
        now: i64,
    ) -> Result<Result<Shipment, ShipmentError>, StorageError> {
        let Ok(request) = uuid::Uuid::parse_str(request_id) else {
            return Ok(Err(ShipmentError::Invalid));
        };
        let request = request.to_string();
        let mut tx = self.pool.begin().await?;
        if !actor(&mut tx, admin).await? {
            return Ok(Err(ShipmentError::Forbidden));
        }
        // All shipment/refund mutations acquire the order lock first.
        let exists: Option<String> =
            sqlx::query_scalar("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
                .bind(order)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Ok(Err(ShipmentError::NotFound));
        }
        let Some(row) = sqlx::query("SELECT * FROM shipments WHERE order_id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(Err(ShipmentError::NotFound));
        };
        let mut s = shipment(row)?;
        if s.request_id.as_deref() == Some(&request) {
            if s.version != version
                || s.quote.as_ref().and_then(|q| q["id"].as_str()) != Some(quote_id)
            {
                return Ok(Err(ShipmentError::Conflict));
            }
            s.packages = packages(&mut tx, &s.id).await?;
            tx.commit().await?;
            return Ok(Ok(s));
        }
        if s.version != version || s.state != "draft" {
            return Ok(Err(ShipmentError::Conflict));
        }
        if let Err(e) = eligible_snapshot(&mut tx, &s).await? {
            return Ok(Err(e));
        }
        let Some(q) = s.quote.as_ref() else {
            return Ok(Err(ShipmentError::Conflict));
        };
        if q["id"].as_str() != Some(quote_id) || q["expires_at"].as_i64().is_none_or(|t| t <= now) {
            return Ok(Err(ShipmentError::Conflict));
        }
        let existing = packages(&mut tx, &s.id).await?;
        if !existing.is_empty() {
            let approved = q["packages"].as_array().ok_or("invalid persisted quote")?;
            if existing.len() != approved.len()
                || existing.iter().zip(approved).any(|(old, new)| {
                    let (mut a, mut b) = (old.payload.clone(), new["body"].clone());
                    for v in [&mut a, &mut b] {
                        if let Some(v) = v.as_object_mut() {
                            v.remove("user_reference_number");
                        }
                    }
                    old.state != "created" || a != b
                })
            {
                return Ok(Err(ShipmentError::Conflict));
            }
        }
        for (index, p) in q["packages"]
            .as_array()
            .ok_or("invalid persisted shipping quote")?
            .iter()
            .enumerate()
        {
            if !existing.is_empty() {
                break;
            }
            let id = uuid::Uuid::new_v4().simple().to_string();
            let reference = format!("RM{}-{:02}", &s.id.replace('-', "")[..20], index + 1);
            let mut body = p["body"].clone();
            body["user_reference_number"] = json!(reference);
            sqlx::query("INSERT INTO shipment_packages(id,shipment_id,reference,state,payload,updated_at) VALUES($1,$2,$3,'pending',$4,$5)").bind(&id).bind(&s.id).bind(reference).bind(body).bind(now).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE shipments SET state='queued',request_id=$2,actor_id=$3,next_attempt=$4,updated_at=$4 WHERE id=$1").bind(&s.id).bind(request).bind(admin).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(self
            .shipment_get(order)
            .await?
            .ok_or("shipment vanished")?))
    }
    /// Caller has verified every stored package is still waiting and the old command is absent.
    pub async fn shipment_reopen(
        &self,
        order: &str,
        version: i64,
        admin: &str,
        now: i64,
    ) -> Result<Result<Shipment, ShipmentError>, StorageError> {
        let mut tx = self.pool.begin().await?;
        if !actor(&mut tx, admin).await? {
            return Ok(Err(ShipmentError::Forbidden));
        }
        sqlx::query("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?;
        let row=sqlx::query("UPDATE shipments SET actor_id=$3 WHERE order_id=$1 AND version=$2 AND state='manual_review' AND last_error IN ('provider_terms','provider_rejected','price_or_data_changed','price_unavailable','order_or_refund_changed') AND COALESCE(lease_until,0)<=$4 RETURNING *").bind(order).bind(version).bind(admin).bind(now).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(Err(ShipmentError::Conflict));
        };
        let s = shipment(row)?;
        if let Err(e) = eligible_snapshot(&mut tx, &s).await? {
            return Ok(Err(e));
        }
        let rows = packages(&mut tx, &s.id).await?;
        if rows.is_empty()
            || rows.iter().any(|p| {
                !matches!(p.state.as_str(), "created" | "ordering") || p.provider_id.is_none()
            })
        {
            return Ok(Err(ShipmentError::Conflict));
        }
        sqlx::query(
            "UPDATE shipment_packages SET state='created',updated_at=$2 WHERE shipment_id=$1",
        )
        .bind(&s.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE shipments SET state='draft',version=version+1,quote=NULL,request_id=NULL,last_error=NULL,lease_token=NULL,lease_until=NULL,updated_at=$2 WHERE id=$1").bind(&s.id).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(self
            .shipment_get(order)
            .await?
            .ok_or("shipment vanished")?))
    }
    pub async fn shipment_lease(&self, now: i64) -> Result<Option<Shipment>, StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE shipments SET state='manual_review',last_error='retry_exhausted',lease_token=NULL,lease_until=NULL WHERE id IN (SELECT id FROM shipments WHERE attempts>=1000 AND state NOT IN ('draft','delivered') AND COALESCE(lease_until,0)<=$1 LIMIT 100 FOR UPDATE SKIP LOCKED)").bind(now).execute(&mut *tx).await?;
        let token = uuid::Uuid::new_v4().to_string();
        let row=sqlx::query("UPDATE shipments SET lease_token=$2,lease_until=$1+120,attempts=attempts+1,updated_at=$1 WHERE id=(SELECT id FROM shipments WHERE attempts<1000 AND state NOT IN ('draft','delivered') AND (state<>'manual_review' OR EXISTS(SELECT 1 FROM shipment_packages p WHERE p.shipment_id=shipments.id AND p.state IN ('booked','dispatched'))) AND next_attempt<=$1 AND (lease_until IS NULL OR lease_until<=$1) ORDER BY next_attempt FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *").bind(now).bind(token).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let mut s = shipment(row)?;
        s.packages = packages(&mut tx, &s.id).await?;
        tx.commit().await?;
        Ok(Some(s))
    }
    /// Persist a package transition before HTTP. Only the current lease can commit a result.
    pub async fn shipment_package_transition(
        &self,
        s: &Shipment,
        package: &str,
        from: &str,
        to: &str,
        provider: Option<&str>,
        now: i64,
    ) -> Result<bool, StorageError> {
        let allowed = matches!(
            (from, to),
            ("pending", "creating")
                | ("creating", "created")
                | ("created", "ordering")
                | ("ordering", "booked")
                | ("booked", "dispatched")
                | ("dispatched", "delivered")
        ) || to == "manual_review";
        if !allowed
            || provider
                .is_some_and(|p| p.is_empty() || p.len() > 128 || p.chars().any(char::is_control))
        {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await?;
        if matches!(to, "creating" | "ordering") && eligible_snapshot(&mut tx, s).await?.is_err() {
            sqlx::query("UPDATE shipments SET state='manual_review',lease_token=NULL,lease_until=NULL,updated_at=$3 WHERE id=$1 AND lease_token=$2").bind(&s.id).bind(&s.lease_token).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(false);
        }
        let changed=sqlx::query("UPDATE shipment_packages p SET state=$4,provider_id=COALESCE($5,p.provider_id),updated_at=$6 FROM shipments s WHERE p.shipment_id=s.id AND s.id=$1 AND s.lease_token=$2 AND s.lease_until>$6 AND p.id=$3 AND p.state=$7 AND ($4 NOT IN ('created','ordering','booked') OR COALESCE($5,p.provider_id) IS NOT NULL)").bind(&s.id).bind(&s.lease_token).bind(package).bind(to).bind(provider).bind(now).bind(from).execute(&mut *tx).await?.rows_affected()==1;
        tx.commit().await?;
        Ok(changed)
    }
    /// Global command state is durable before PUT; an expired ordering lease reconciles that command UUID.
    pub async fn shipment_state(
        &self,
        s: &Shipment,
        state: &str,
        delay: i64,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !matches!(
            state,
            "queued"
                | "creating"
                | "ordering"
                | "booked"
                | "dispatched"
                | "delivered"
                | "manual_review"
        ) || !(0..=86400).contains(&delay)
        {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE shipments SET state=CASE WHEN state='delivered' THEN state ELSE $3 END,next_attempt=$4+$5,lease_token=NULL,lease_until=NULL,updated_at=$4 WHERE id=$1 AND lease_token=$2 AND lease_until>$4").bind(&s.id).bind(&s.lease_token).bind(state).bind(now).bind(delay).execute(&self.pool).await?.rows_affected()==1)
    }
    /// Set command phase without releasing the lease; never changes a previously stored command ID.
    pub async fn shipment_order_start(&self, s: &Shipment, now: i64) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        if eligible_snapshot(&mut tx, s).await?.is_err() {
            return Ok(false);
        }
        let changed=sqlx::query("UPDATE shipments SET state='ordering',updated_at=$3 WHERE id=$1 AND lease_token=$2 AND lease_until>$3 AND state IN ('queued','creating') AND NOT EXISTS(SELECT 1 FROM shipment_packages WHERE shipment_id=$1 AND state<>'created')").bind(&s.id).bind(&s.lease_token).bind(now).execute(&mut *tx).await?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE shipment_packages SET state='ordering',updated_at=$2 WHERE shipment_id=$1 AND state='created'").bind(&s.id).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
}
impl Storage {
    pub async fn shipment_retry(
        &self,
        s: &Shipment,
        delay: i64,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !(1..=86400).contains(&delay) {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE shipments SET next_attempt=$3+$4,lease_token=NULL,lease_until=NULL,updated_at=$3 WHERE id=$1 AND lease_token=$2 AND lease_until>$3").bind(&s.id).bind(&s.lease_token).bind(now).bind(delay).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn shipment_tracking(
        &self,
        s: &Shipment,
        package: &str,
        tracking: &Value,
        dispatched: bool,
        delivered: bool,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !tracking.is_object()
            || tracking.to_string().len() > 32768
            || ((dispatched || delivered) && tracking_numbers(tracking).is_empty())
            || tracking_numbers(tracking)
                .iter()
                .any(|s| s.is_empty() || s.len() > 160 || s.chars().any(char::is_control))
        {
            return Ok(false);
        }
        let mut tx = self.queue_transaction().await?;
        // Same order-first lock order as refunds and handover.
        sqlx::query("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
            .bind(&s.order_id)
            .fetch_optional(&mut *tx)
            .await?;
        let changed=sqlx::query("UPDATE shipment_packages p SET tracking=CASE WHEN p.state='delivered' THEN p.tracking ELSE $4 END,state=CASE WHEN $6 THEN 'delivered' WHEN $5 AND p.state='booked' THEN 'dispatched' ELSE p.state END,updated_at=$7 FROM shipments s WHERE p.shipment_id=s.id AND s.id=$1 AND s.lease_token=$2 AND s.lease_until>$7 AND p.id=$3 AND p.state IN ('booked','dispatched','delivered')").bind(&s.id).bind(&s.lease_token).bind(package).bind(tracking).bind(dispatched).bind(delivered).bind(now).execute(&mut *tx).await?.rows_affected()==1;
        if changed {
            progress(&mut tx, &s.order_id, &s.id, now).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    /// Operator handover is a physical confirmation, separate from label creation.
    pub async fn shipment_handover(
        &self,
        order: &str,
        ids: &[String],
        admin: &str,
        now: i64,
    ) -> Result<Result<Shipment, ShipmentError>, StorageError> {
        if ids.is_empty() || ids.len() > 30 {
            return Ok(Err(ShipmentError::Invalid));
        }
        let mut tx = self.queue_transaction().await?;
        if !actor(&mut tx, admin).await? {
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
        let Some(row) = sqlx::query("SELECT * FROM shipments WHERE order_id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(Err(ShipmentError::NotFound));
        };
        let s = shipment(row)?;
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM shipment_packages WHERE shipment_id=$1 AND id=ANY($2) AND state IN ('booked','dispatched','delivered') AND ((jsonb_typeof(tracking->'numbers')='array' AND jsonb_array_length(tracking->'numbers')>0) OR length(tracking->>'number')>0)").bind(&s.id).bind(ids).fetch_one(&mut *tx).await?;
        let distinct = ids.iter().collect::<std::collections::HashSet<_>>().len();
        if count as usize != distinct {
            return Ok(Err(ShipmentError::Conflict));
        }
        sqlx::query("UPDATE shipment_packages SET state='dispatched',updated_at=$3 WHERE shipment_id=$1 AND id=ANY($2) AND state='booked'").bind(&s.id).bind(ids).bind(now).execute(&mut *tx).await?;
        progress(&mut tx, order, &s.id, now).await?;
        tx.commit().await?;
        Ok(Ok(self
            .shipment_get(order)
            .await?
            .ok_or("shipment vanished")?))
    }
    /// Manual recovery requires externally verified provider identity; never sends another POST.
    pub async fn shipment_recover(
        &self,
        order: &str,
        package: &str,
        provider: &str,
        admin: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if provider.is_empty() || provider.len() > 128 || provider.chars().any(char::is_control) {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await?;
        if !actor(&mut tx, admin).await? {
            return Ok(false);
        }
        sqlx::query("SELECT id FROM orders WHERE id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?;
        let changed=sqlx::query("UPDATE shipment_packages p SET provider_id=$3,state='created',updated_at=$4 FROM shipments s WHERE p.shipment_id=s.id AND s.order_id=$1 AND p.id=$2 AND NOT EXISTS(SELECT 1 FROM shipment_packages other WHERE other.provider_id=$3 AND other.id<>p.id) AND p.state IN ('creating','manual_review') AND p.provider_id IS NULL AND (s.lease_until IS NULL OR s.lease_until<=$4)").bind(order).bind(package).bind(provider).bind(now).execute(&mut *tx).await?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE shipments SET state='queued',lease_token=NULL,lease_until=NULL,next_attempt=$2,updated_at=$2 WHERE order_id=$1").bind(order).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
}
async fn progress(
    tx: &mut Transaction<'_, Postgres>,
    order: &str,
    shipment: &str,
    now: i64,
) -> Result<(), StorageError> {
    let rows = packages(tx, shipment).await?;
    let handed: Vec<_> = rows
        .iter()
        .filter(|p| matches!(p.state.as_str(), "dispatched" | "delivered"))
        .collect();
    if handed.is_empty() {
        return Ok(());
    }
    let orderrow =
        sqlx::query("SELECT user_id,contact,guest_updates,status FROM orders WHERE id=$1")
            .bind(order)
            .fetch_one(&mut **tx)
            .await?;
    let user: Option<String> = orderrow.try_get("user_id")?;
    let contact: Value = orderrow.try_get("contact")?;
    // Deterministic cumulative set makes retries and tracking polls enqueue only once.
    let key = handed.len();
    let tracking: Vec<&str> = handed
        .iter()
        .flat_map(|p| tracking_numbers(&p.tracking))
        .collect();
    let carrier = handed
        .iter()
        .find_map(|p| p.tracking["carrier"].as_str())
        .unwrap_or("Kurier");
    crate::orders::enqueue(tx,order,&format!("shipment:{key}"),user.as_deref(),contact["email"].as_str().ok_or("invalid order contact")?,"shipment_dispatched",json!({"reference":order,"carrier":carrier,"tracking_numbers":tracking,"remaining_packages":rows.len()-handed.len()}),orderrow.try_get("guest_updates")?,now).await?;
    if handed.len() == rows.len() {
        let delivered = rows.iter().all(|p| p.state == "delivered");
        sqlx::query("UPDATE orders SET status=$2 WHERE id=$1 AND status IN ('ready','shipped')")
            .bind(order)
            .bind(if delivered { "completed" } else { "shipped" })
            .execute(&mut **tx)
            .await?;
        sqlx::query("UPDATE shipments SET state=$2,updated_at=$3 WHERE id=$1")
            .bind(shipment)
            .bind(if delivered { "delivered" } else { "dispatched" })
            .bind(now)
            .execute(&mut **tx)
            .await?;
        if delivered {
            sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
                .bind(order)
                .bind(
                    now.checked_add(crate::order_files::COMPLETED_FILE_RETENTION)
                        .ok_or("retention overflow")?,
                )
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn shipments_reserve_once_recover_and_dispatch_atomically() {
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
        sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,'admin',$3)")
            .bind(&admin)
            .bind(format!("{admin}@example.invalid"))
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO orders(id,owner_hash,contact,lines,amount_grosze,provider_kind,provider_id,status,guest_updates,created_at,paid_at,fulfillment) VALUES($1,$2,$3,'[]',10000,'intent',$4,'ready',true,$5,$5,$6)").bind(&order).bind("a".repeat(64)).bind(json!({"email":"void@sieciowiec.xyz"})).bind(format!("pi_{}",order.replace('-',""))).bind(now).bind(json!({"delivery":{"method":"courier"}})).execute(&db.pool).await.unwrap();
        let parcels = json!([{"outer_mm":[120,120,80],"weight_g":600},{"outer_mm":[120,120,80],"weight_g":600}]);
        let s = db
            .shipment_save(&order, 0, &parcels, false, &admin, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            db.shipment_save(&order, 0, &parcels, false, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Conflict
        );
        assert_eq!(
            db.shipment_save(&order, 1, &parcels, false, "no-user", now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Forbidden
        );
        let qid = uuid::Uuid::new_v4().to_string();
        let quote = json!({"id":qid,"expires_at":now+120,"price_grosze":2500,"packages":[{"body":{"service":"dpd"}},{"body":{"service":"dpd"}}]});
        assert!(
            db.shipment_quote_save(&order, s.version, &quote, now)
                .await
                .unwrap()
        );
        let request = uuid::Uuid::new_v4().to_string();
        let refund = db
            .refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 1000, &admin, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            db.shipment_book(&order, 1, &request, &qid, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            ShipmentError::Refund
        );
        sqlx::query("UPDATE payment_refunds SET state='failed' WHERE id=$1")
            .bind(refund.id)
            .execute(&db.pool)
            .await
            .unwrap();
        let (a, b) = tokio::join!(
            db.shipment_book(&order, 1, &request, &qid, &admin, now),
            db.shipment_book(&order, 1, &request, &qid, &admin, now)
        );
        let s = a.unwrap().unwrap();
        assert_eq!(s.packages.len(), 2);
        assert_eq!(s.packages[0].id, b.unwrap().unwrap().packages[0].id);
        assert_eq!(
            db.refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 100, &admin, now)
                .await
                .unwrap()
                .unwrap_err(),
            crate::RefundPrepareError::ShipmentActive
        );
        assert!(s.packages.iter().all(|p| p.reference.len() <= 25));
        assert!(!db.order_status(&order, "shipped", now).await.unwrap());
        assert!(!db.order_status(&order, "completed", now).await.unwrap());
        let lease = db.shipment_lease(now).await.unwrap().unwrap();
        assert!(db.shipment_lease(now).await.unwrap().is_none());
        let first = &lease.packages[0].id;
        assert!(
            db.shipment_package_transition(&lease, first, "pending", "creating", None, now)
                .await
                .unwrap()
        );
        assert!(
            !db.shipment_package_transition(&lease, first, "pending", "creating", None, now)
                .await
                .unwrap()
        );
        assert!(db.shipment_retry(&lease, 1, now).await.unwrap());
        let lease = db.shipment_lease(now + 1).await.unwrap().unwrap();
        assert_eq!(lease.packages[0].state, "creating");
        assert!(
            db.shipment_package_transition(
                &lease,
                first,
                "creating",
                "created",
                Some("provider-1"),
                now + 1
            )
            .await
            .unwrap()
        );
        let second = &lease.packages[1].id;
        assert!(
            db.shipment_package_transition(&lease, second, "pending", "creating", None, now + 1)
                .await
                .unwrap()
        );
        assert!(
            db.shipment_package_transition(
                &lease,
                second,
                "creating",
                "created",
                Some("provider-2"),
                now + 1
            )
            .await
            .unwrap()
        );
        assert!(
            db.shipment_problem(&lease, "price_or_data_changed", now + 1)
                .await
                .unwrap()
        );
        let reopened = db
            .shipment_reopen(&order, 1, &admin, now + 1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reopened.version, 2);
        assert!(
            db.shipment_quote_save(&order, 2, &quote, now + 1)
                .await
                .unwrap()
        );
        let rebooked = db
            .shipment_book(
                &order,
                2,
                &uuid::Uuid::new_v4().to_string(),
                &qid,
                &admin,
                now + 1,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rebooked.packages.len(), 2);
        assert_eq!(&rebooked.packages[0].id, first);
        let lease = db.shipment_lease(now + 1).await.unwrap().unwrap();
        assert!(db.shipment_order_start(&lease, now + 1).await.unwrap());
        assert!(
            db.shipment_package_transition(&lease, first, "ordering", "booked", None, now + 1)
                .await
                .unwrap()
        );
        assert!(
            db.shipment_package_transition(
                &lease,
                second,
                "ordering",
                "manual_review",
                None,
                now + 1
            )
            .await
            .unwrap()
        );
        assert!(
            db.shipment_state(&lease, "manual_review", 1, now + 1)
                .await
                .unwrap()
        );
        let lease = db.shipment_lease(now + 2).await.unwrap().unwrap();
        assert!(
            db.shipment_tracking(
                &lease,
                first,
                &json!({"number":"TRACK1","carrier":"DPD"}),
                true,
                false,
                now + 2
            )
            .await
            .unwrap()
        );
        let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id=$1")
            .bind(&order)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, "ready");
        let mails: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_outbox WHERE kind='shipment_dispatched'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(mails, 1);
        assert!(
            db.shipment_tracking(
                &lease,
                first,
                &json!({"number":"TRACK1","carrier":"DPD"}),
                true,
                true,
                now + 2
            )
            .await
            .unwrap()
        );
        let mails: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_outbox WHERE kind='shipment_dispatched'",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(mails, 1);
        assert!(
            !db.shipment_package_transition(
                &lease,
                second,
                "manual_review",
                "booked",
                Some("provider-2"),
                now + 2
            )
            .await
            .unwrap()
        );
        // A later command result can confirm the remaining consignment; labels alone never shipped it.
        sqlx::query("UPDATE shipment_packages SET state='ordering' WHERE id=$1")
            .bind(second)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            db.shipment_package_transition(&lease, second, "ordering", "booked", None, now + 2)
                .await
                .unwrap()
        );
        assert!(
            db.shipment_tracking(
                &lease,
                second,
                &json!({"numbers":["TRACK2","TRACK3"],"carrier":"DPD"}),
                true,
                true,
                now + 2
            )
            .await
            .unwrap()
        );
        let status: String = sqlx::query_scalar("SELECT status FROM orders WHERE id=$1")
            .bind(&order)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(status, "completed");
        let state = db.shipment_get(&order).await.unwrap().unwrap();
        assert_eq!(state.state, "delivered");
        assert!(
            db.shipment_tracking(
                &lease,
                second,
                &json!({"numbers":["OLD"],"carrier":"DPD"}),
                false,
                false,
                now + 2
            )
            .await
            .unwrap()
        );
        let state = db.shipment_get(&order).await.unwrap().unwrap();
        assert_eq!(
            state.packages[1].tracking["numbers"],
            json!(["TRACK2", "TRACK3"])
        );
        let payload:Value=sqlx::query_scalar("SELECT payload FROM email_outbox WHERE kind='shipment_dispatched' AND payload->'notification'->>'remaining_packages'='0'").fetch_one(&db.pool).await.unwrap();
        assert_eq!(
            payload["notification"]["tracking_numbers"],
            json!(["TRACK1", "TRACK2", "TRACK3"])
        );
        sqlx::query("UPDATE shipments SET state='booked',attempts=1000,next_attempt=0,lease_until=0 WHERE order_id=$1")
            .bind(&order).execute(&db.pool).await.unwrap();
        assert!(db.shipment_lease(now + 1000).await.unwrap().is_none());
        assert_eq!(
            db.shipment_get(&order)
                .await
                .unwrap()
                .unwrap()
                .last_error
                .as_deref(),
            Some("retry_exhausted")
        );
    }
}
