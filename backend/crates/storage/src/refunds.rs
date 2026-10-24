//! Refund reservations survive ambiguous provider responses and process restarts.
use crate::{Storage, StorageError};
use serde::Serialize;
use sqlx::{Row, postgres::PgRow};

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Refund {
    pub id: String,
    pub order_id: String,
    pub request_id: String,
    pub amount_grosze: i64,
    pub state: String,
    pub provider_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip)]
    pub payment_intent: Option<String>,
    #[serde(skip)]
    pub idempotency_key: String,
    #[serde(skip)]
    pub lease_token: Option<String>,
    #[serde(skip)]
    pub attempts: i32,
    #[serde(skip)]
    pub provider_kind: String,
    #[serde(skip)]
    pub original_provider_id: String,
    #[serde(skip)]
    pub correlation_id: String,
    #[serde(skip)]
    pub payment_amount_grosze: i64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RefundPrepareError {
    NotFound,
    NotPaid,
    Amount,
    Conflict,
    Limit,
    Forbidden,
    ShipmentActive,
}
fn refund(row: PgRow) -> Result<Refund, StorageError> {
    Ok(Refund {
        id: row.try_get("id")?,
        order_id: row.try_get("order_id")?,
        request_id: row.try_get("request_id")?,
        amount_grosze: row.try_get("amount_grosze")?,
        state: row.try_get("state")?,
        provider_id: row.try_get("provider_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        payment_intent: row.try_get("payment_intent")?,
        idempotency_key: row.try_get("idempotency_key")?,
        lease_token: row.try_get("lease_token")?,
        attempts: row.try_get("attempts")?,
        provider_kind: row.try_get("provider_kind")?,
        original_provider_id: row.try_get("original_provider_id")?,
        correlation_id: row.try_get("correlation_id")?,
        payment_amount_grosze: row.try_get("payment_amount_grosze")?,
    })
}
fn provider_id(value: &str, prefix: &str) -> bool {
    value.len() > prefix.len()
        && value.len() <= 128
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
impl Storage {
    pub async fn refund_prepare(
        &self,
        order: &str,
        request_id: &str,
        amount: i64,
        actor: &str,
        now: i64,
    ) -> Result<Result<Refund, RefundPrepareError>, StorageError> {
        use RefundPrepareError::*;
        if amount <= 0 || amount > 99999999 || uuid::Uuid::parse_str(request_id).is_err() {
            return Ok(Err(Amount));
        }
        let request_id = uuid::Uuid::parse_str(request_id)?.to_string();
        let mut tx = self.pool.begin().await?;
        // Actor lock also serializes the per-operator daily budget across different orders.
        let admin = sqlx::query(
            "SELECT id FROM users WHERE id=$1 AND role='admin' AND disabled_at IS NULL FOR UPDATE",
        )
        .bind(actor)
        .fetch_optional(&mut *tx)
        .await?;
        if admin.is_none() {
            return Ok(Err(Forbidden));
        }
        let Some(row) = sqlx::query("SELECT * FROM orders WHERE id=$1 FOR UPDATE")
            .bind(order)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(Err(NotFound));
        };
        if let Some(existing) =
            sqlx::query("SELECT * FROM payment_refunds WHERE order_id=$1 AND request_id=$2")
                .bind(order)
                .bind(&request_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            let existing = refund(existing)?;
            return Ok(if existing.amount_grosze == amount {
                Ok(existing)
            } else {
                Err(Conflict)
            });
        }
        if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM shipments WHERE order_id=$1 AND state IN ('queued','creating','ordering'))")
            .bind(order).fetch_one(&mut *tx).await? {
            return Ok(Err(ShipmentActive));
        }
        if row.try_get::<Option<i64>, _>("paid_at")?.is_none()
            || row.try_get::<String, _>("status")? == "awaiting_payment"
        {
            return Ok(Err(NotPaid));
        }
        let total: i64 = row.try_get("amount_grosze")?;
        let kind: String = row.try_get("provider_kind")?;
        let Some(original): Option<String> = row.try_get("provider_id")? else {
            return Ok(Err(NotPaid));
        };
        let attempt_id: Option<String> = row.try_get("payment_attempt_id")?;
        let correlation = attempt_id.as_deref().unwrap_or(order);
        if attempt_id.is_some() {
            let verified: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payment_attempts WHERE id=$1 AND state='paid' AND amount_grosze=$2 AND provider_kind=$3 AND provider_id=$4 AND currency='pln')")
                .bind(correlation).bind(total).bind(&kind).bind(&original).fetch_one(&mut *tx).await?;
            if !verified {
                return Ok(Err(NotPaid));
            }
        }
        if !provider_id(&original, if kind == "intent" { "pi_" } else { "cs_" }) {
            return Ok(Err(NotPaid));
        }
        let reserved: i64 = sqlx::query_scalar("SELECT COALESCE(sum(amount_grosze),0)::bigint FROM payment_refunds WHERE order_id=$1 AND state NOT IN ('failed','canceled')").bind(order).fetch_one(&mut *tx).await?;
        if amount > total - reserved {
            return Ok(Err(Amount));
        }
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM payment_refunds WHERE order_id=$1")
                .bind(order)
                .fetch_one(&mut *tx)
                .await?;
        let daily: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM payment_refunds WHERE actor_id=$1 AND created_at>$2",
        )
        .bind(actor)
        .bind(now - 86400)
        .fetch_one(&mut *tx)
        .await?;
        if count >= 100 || daily >= 30 {
            return Ok(Err(Limit));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let row=sqlx::query("INSERT INTO payment_refunds(id,order_id,request_id,actor_id,amount_grosze,payment_amount_grosze,provider_kind,original_provider_id,correlation_id,payment_intent,idempotency_key,next_attempt,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$12,$12) RETURNING *")
            .bind(&id).bind(order).bind(request_id).bind(actor).bind(amount).bind(total).bind(&kind).bind(&original).bind(correlation).bind(if kind=="intent" {Some(&original)} else {None}).bind(format!("refund:{id}")).bind(now).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(Ok(refund(row)?))
    }
    pub async fn refund_list(&self, order: &str) -> Result<Vec<Refund>, StorageError> {
        sqlx::query(
            "SELECT * FROM payment_refunds WHERE order_id=$1 ORDER BY created_at,id LIMIT 100",
        )
        .bind(order)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(refund)
        .collect()
    }
    pub async fn refund_lease(
        &self,
        id: Option<&str>,
        now: i64,
    ) -> Result<Option<Refund>, StorageError> {
        let mut tx = self.pool.begin().await?;
        let Some(row)=sqlx::query("SELECT * FROM payment_refunds WHERE ($1::text IS NULL OR id=$1) AND state IN ('queued','unknown','pending','requires_action') AND next_attempt<=$2 AND COALESCE(lease_until,0)<=$2 ORDER BY next_attempt,id LIMIT 1 FOR UPDATE SKIP LOCKED").bind(id).bind(now).fetch_optional(&mut *tx).await? else {return Ok(None);};
        let current = refund(row)?;
        if current.attempts >= 1000
            || (current.provider_id.is_none()
                && (current.attempts >= 10 || now - current.created_at >= 23 * 3600))
        {
            sqlx::query("UPDATE payment_refunds SET state='manual_review',lease_token=NULL,lease_until=NULL,updated_at=$2 WHERE id=$1").bind(&current.id).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        }
        let row=sqlx::query("UPDATE payment_refunds SET state=CASE WHEN attempts>0 AND provider_id IS NULL THEN 'unknown' ELSE state END,lease_token=$2,lease_until=$3+120,attempts=attempts+1,updated_at=$3 WHERE id=$1 RETURNING *").bind(current.id).bind(uuid::Uuid::new_v4().to_string()).bind(now).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(refund(row)?))
    }
    pub async fn refund_set_intent(
        &self,
        id: &str,
        lease: &str,
        intent: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !provider_id(intent, "pi_") {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE payment_refunds SET payment_intent=$3,updated_at=$4 WHERE id=$1 AND lease_token=$2 AND lease_until>$4 AND (payment_intent IS NULL OR payment_intent=$3) AND state IN ('queued','unknown','pending','requires_action')").bind(id).bind(lease).bind(intent).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn refund_resolve(
        &self,
        id: &str,
        lease: &str,
        provider: &str,
        state: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !provider_id(provider, "re_")
            || !matches!(
                state,
                "pending"
                    | "requires_action"
                    | "succeeded"
                    | "failed"
                    | "canceled"
                    | "manual_review"
            )
        {
            return Ok(false);
        }
        Ok(sqlx::query("UPDATE payment_refunds SET provider_id=$3,state=$4,updated_at=$5,next_attempt=$5+60,lease_token=NULL,lease_until=NULL WHERE id=$1 AND lease_token=$2 AND lease_until>$5 AND (provider_id IS NULL OR provider_id=$3) AND state IN ('queued','unknown','pending','requires_action')").bind(id).bind(lease).bind(provider).bind(state).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    /// Only the first unambiguous rejection releases money. Later errors cannot disprove a previous create.
    pub async fn refund_retry(
        &self,
        id: &str,
        lease: &str,
        ambiguous: bool,
        now: i64,
    ) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE payment_refunds SET state=CASE WHEN provider_id IS NOT NULL THEN state WHEN NOT $3 AND attempts=1 AND state='queued' THEN 'failed' WHEN NOT $3 OR attempts>=10 OR created_at<=$4-82800 THEN 'manual_review' ELSE 'unknown' END,next_attempt=$4+60,updated_at=$4,lease_token=NULL,lease_until=NULL WHERE id=$1 AND lease_token=$2 AND lease_until>$4 AND state IN ('queued','unknown','pending','requires_action')").bind(id).bind(lease).bind(ambiguous).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn refund_review(
        &self,
        id: &str,
        lease: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE payment_refunds SET state='manual_review',updated_at=$3,lease_token=NULL,lease_until=NULL WHERE id=$1 AND lease_token=$2 AND lease_until>$3 AND state IN ('queued','unknown','pending','requires_action')").bind(id).bind(lease).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn refund_reservations_are_atomic_and_survive_ambiguous_results() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("rapidmaker_test")
        );
        let db = Storage::connect(&url).await.unwrap();
        let actor = uuid::Uuid::new_v4().to_string();
        let order = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,'admin',1000)")
            .bind(&actor)
            .bind(format!("{actor}@example.invalid"))
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO orders(id,owner_hash,contact,lines,amount_grosze,provider_kind,provider_id,status,guest_updates,created_at,paid_at) VALUES($1,$2,'{}','[]',1000,'intent',$3,'paid',false,1000,1000)")
            .bind(&order).bind("a".repeat(64)).bind(format!("pi_{}",order.replace('-',""))).execute(&db.pool).await.unwrap();
        let request = uuid::Uuid::new_v4().to_string();
        let first = db
            .refund_prepare(&order, &request, 600, &actor, 1001)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            db.refund_prepare(&order, &request, 600, &actor, 1002)
                .await
                .unwrap()
                .unwrap()
                .id,
            first.id
        );
        assert_eq!(
            db.refund_prepare(&order, &request, 601, &actor, 1002)
                .await
                .unwrap()
                .unwrap_err(),
            RefundPrepareError::Conflict
        );
        assert_eq!(
            db.refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 0, &actor, 1002)
                .await
                .unwrap()
                .unwrap_err(),
            RefundPrepareError::Amount
        );
        let request2 = uuid::Uuid::new_v4().to_string();
        let request3 = uuid::Uuid::new_v4().to_string();
        let (a, b) = tokio::join!(
            db.refund_prepare(&order, &request2, 400, &actor, 1002),
            db.refund_prepare(&order, &request3, 400, &actor, 1002)
        );
        assert_eq!(
            usize::from(a.unwrap().is_ok()) + usize::from(b.unwrap().is_ok()),
            1,
            "concurrent requests cannot spend the same remainder"
        );
        let leased = db
            .refund_lease(Some(&first.id), 1002)
            .await
            .unwrap()
            .unwrap();
        let lease = leased.lease_token.unwrap();
        assert!(
            db.refund_lease(Some(&first.id), 1003)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !db.refund_set_intent(&first.id, &lease, "pi_changed", 1003)
                .await
                .unwrap()
        );
        assert!(
            db.refund_retry(&first.id, &lease, true, 1003)
                .await
                .unwrap()
        );
        let fresh = Storage::connect(&url).await.unwrap();
        let leased = fresh
            .refund_lease(Some(&first.id), 1064)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(leased.state, "unknown");
        assert_eq!(leased.idempotency_key, first.idempotency_key);
        assert!(
            fresh
                .refund_retry(
                    &first.id,
                    leased.lease_token.as_deref().unwrap(),
                    false,
                    1065
                )
                .await
                .unwrap()
        );
        assert_eq!(
            fresh
                .refund_list(&order)
                .await
                .unwrap()
                .iter()
                .find(|r| r.id == first.id)
                .unwrap()
                .state,
            "manual_review"
        );
        assert_eq!(
            fresh
                .refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 1, &actor, 1066)
                .await
                .unwrap()
                .unwrap_err(),
            RefundPrepareError::Amount
        );
        let second = fresh
            .refund_list(&order)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id != first.id)
            .unwrap();
        let leased = fresh
            .refund_lease(Some(&second.id), 1066)
            .await
            .unwrap()
            .unwrap();
        assert!(
            fresh
                .refund_resolve(
                    &second.id,
                    leased.lease_token.as_deref().unwrap(),
                    "re_test",
                    "pending",
                    1067
                )
                .await
                .unwrap()
        );
        let leased = fresh
            .refund_lease(Some(&second.id), 1128)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !fresh
                .refund_resolve(
                    &second.id,
                    leased.lease_token.as_deref().unwrap(),
                    "re_different",
                    "succeeded",
                    1129
                )
                .await
                .unwrap()
        );
        assert!(
            fresh
                .refund_resolve(
                    &second.id,
                    leased.lease_token.as_deref().unwrap(),
                    "re_test",
                    "failed",
                    1129
                )
                .await
                .unwrap()
        );
        let expiring = fresh
            .refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 400, &actor, 1130)
            .await
            .unwrap()
            .unwrap();
        assert!(
            fresh
                .refund_lease(Some(&expiring.id), 1130 + 23 * 3600)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fresh
                .refund_list(&order)
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.id == expiring.id)
                .unwrap()
                .state,
            "manual_review"
        );
        sqlx::query("UPDATE orders SET paid_at=NULL,status='awaiting_payment' WHERE id=$1")
            .bind(&order)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            fresh
                .refund_prepare(&order, &uuid::Uuid::new_v4().to_string(), 1, &actor, 1131)
                .await
                .unwrap()
                .unwrap_err(),
            RefundPrepareError::NotPaid
        );
    }
}
