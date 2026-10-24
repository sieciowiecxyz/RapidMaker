//! Durable checkout state; no transaction is held across provider HTTP requests.
use crate::{
    Storage, StorageError,
    order_files::hold_files,
    orders::{enqueue, mark_paid},
};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};

#[derive(Debug, Serialize)]
pub struct CheckoutDraft {
    pub id: String,
    pub version: i64,
    pub input: Value,
    pub quote: Option<Value>,
    pub quote_expires_at: Option<i64>,
    pub expires_at: i64,
    pub locked: bool,
}
pub struct PaymentAttempt {
    pub id: String,
    pub draft_id: String,
    pub state: String,
    pub provider_kind: String,
    pub offer_id: Option<String>,
    pub provider_id: Option<String>,
    pub amount_grosze: i64,
    pub provider_request: Value,
    pub idempotency_key: String,
    pub provider_response: Option<Value>,
    pub lease_token: Option<String>,
    pub attempts: i32,
    pub created_at: i64,
}
pub struct AttemptInput<'a> {
    pub id: &'a str,
    pub draft_id: &'a str,
    pub owner_hash: &'a str,
    pub version: i64,
    pub offer_id: &'a str,
    pub client_key_hash: &'a str,
    pub fingerprint: &'a str,
    pub provider_kind: &'a str,
    pub provider_request: &'a Value,
}
#[derive(Debug)]
pub struct StripeEvent {
    pub id: String,
    pub event_type: String,
    pub provider_kind: String,
    pub object_id: String,
    pub payload: Value,
    pub lease_token: String,
    pub attempts: i32,
}
fn draft(row: PgRow) -> Result<CheckoutDraft, StorageError> {
    Ok(CheckoutDraft {
        id: row.try_get("id")?,
        version: row.try_get("version")?,
        input: row.try_get("input")?,
        quote: row.try_get("quote")?,
        quote_expires_at: row.try_get("quote_expires_at")?,
        expires_at: row.try_get("expires_at")?,
        locked: row.try_get("locked")?,
    })
}
fn attempt(row: PgRow) -> Result<PaymentAttempt, StorageError> {
    Ok(PaymentAttempt {
        id: row.try_get("id")?,
        draft_id: row.try_get("draft_id")?,
        state: row.try_get("state")?,
        provider_kind: row.try_get("provider_kind")?,
        offer_id: row.try_get("offer_id")?,
        provider_id: row.try_get("provider_id")?,
        amount_grosze: row.try_get("amount_grosze")?,
        provider_request: row.try_get("provider_request")?,
        idempotency_key: row.try_get("idempotency_key")?,
        provider_response: row.try_get("provider_response")?,
        lease_token: row.try_get("lease_token")?,
        attempts: row.try_get("attempts")?,
        created_at: row.try_get("created_at")?,
    })
}
fn bounded(value: &Value, max: usize) -> bool {
    value.is_object() && value.to_string().len() <= max
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
impl Storage {
    /// Bound paid external lookups across processes and guest identities.
    pub async fn checkout_provider_allowed(
        &self,
        owner: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !hash(owner) {
            return Ok(false);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(1380794709)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM auth_limits WHERE scope_hash IN (SELECT scope_hash FROM auth_limits WHERE window_start<$1 LIMIT 100)").bind(now-86400).execute(&mut *tx).await?;
        let allowed = crate::auth::limit(
            &mut tx,
            "checkout-provider-owner",
            owner.as_bytes(),
            30,
            now,
            60,
        )
        .await?
            && crate::auth::limit(&mut tx, "checkout-provider-global", b"all", 300, now, 60)
                .await?;
        tx.commit().await?;
        Ok(allowed)
    }
    pub async fn checkout_create(
        &self,
        owner: &str,
        user: Option<&str>,
        input: &Value,
        now: i64,
    ) -> Result<CheckoutDraft, StorageError> {
        if !hash(owner) || !bounded(input, 65536) {
            return Err("invalid checkout input".into());
        }
        let mut tx = self.queue_transaction().await?;
        sqlx::query("DELETE FROM checkout_drafts d WHERE expires_at<=$1 AND NOT locked AND NOT EXISTS(SELECT 1 FROM payment_attempts WHERE draft_id=d.id)").bind(now).execute(&mut *tx).await?;
        let global: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM checkout_drafts WHERE expires_at>$1 AND NOT locked",
        )
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if global >= 10000 {
            return Err("global checkout quota".into());
        }
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM checkout_drafts WHERE owner_hash=$1 AND created_at>$2-86400",
        )
        .bind(owner)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if count >= 30 {
            return Err("checkout quota".into());
        }
        let row=sqlx::query("INSERT INTO checkout_drafts(id,owner_hash,user_id,input,expires_at,created_at) VALUES($1,$2,$3,$4,$5,$6) RETURNING *")
            .bind(uuid::Uuid::new_v4().to_string()).bind(owner).bind(user).bind(input).bind(now+86400).bind(now).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        draft(row)
    }
    pub async fn checkout_get(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<Option<CheckoutDraft>, StorageError> {
        sqlx::query("SELECT * FROM checkout_drafts WHERE id=$1 AND owner_hash=$2 AND (expires_at>$3 OR locked)").bind(id).bind(owner).bind(now).fetch_optional(&self.pool).await?.map(draft).transpose()
    }
    pub async fn checkout_update(
        &self,
        id: &str,
        owner: &str,
        version: i64,
        input: &Value,
        now: i64,
    ) -> Result<Option<CheckoutDraft>, StorageError> {
        if !bounded(input, 65536) {
            return Err("invalid checkout input".into());
        }
        sqlx::query("UPDATE checkout_drafts SET input=$4,version=version+1,quote=NULL,quote_expires_at=NULL WHERE id=$1 AND owner_hash=$2 AND version=$3 AND NOT locked AND expires_at>$5 RETURNING *")
            .bind(id).bind(owner).bind(version).bind(input).bind(now).fetch_optional(&self.pool).await?.map(draft).transpose()
    }
    pub async fn checkout_quote(
        &self,
        id: &str,
        owner: &str,
        version: i64,
        quote: &Value,
        valid_until: i64,
        now: i64,
    ) -> Result<Option<CheckoutDraft>, StorageError> {
        if !bounded(quote, 131072) || valid_until <= now || valid_until > now + 3600 {
            return Err("invalid checkout quote".into());
        }
        sqlx::query("UPDATE checkout_drafts SET quote=$4,quote_expires_at=$5,version=version+1 WHERE id=$1 AND owner_hash=$2 AND version=$3 AND NOT locked AND expires_at>$6 RETURNING *")
            .bind(id).bind(owner).bind(version).bind(quote).bind(valid_until).bind(now).fetch_optional(&self.pool).await?.map(draft).transpose()
    }
    /// Persist the exact provider request, immutable order and model holds before network IO.
    pub async fn checkout_prepare(
        &self,
        input: &AttemptInput<'_>,
        now: i64,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        if uuid::Uuid::parse_str(input.id).is_err()
            || !hash(input.client_key_hash)
            || !hash(input.fingerprint)
            || (!input.provider_request.is_array()
                || input.provider_request.to_string().len() > 65536)
            || !matches!(input.provider_kind, "session" | "intent")
        {
            return Err("invalid payment attempt".into());
        }
        let mut tx = self.queue_transaction().await?;
        if let Some(row) = sqlx::query(
            "SELECT * FROM payment_attempts WHERE owner_hash=$1 AND client_key_hash=$2 FOR UPDATE",
        )
        .bind(input.owner_hash)
        .bind(input.client_key_hash)
        .fetch_optional(&mut *tx)
        .await?
        {
            if row.get::<String, _>("fingerprint") != input.fingerprint
                || row.get::<String, _>("draft_id") != input.draft_id
                || row.get::<String, _>("provider_kind") != input.provider_kind
            {
                return Ok(None);
            }
            return Ok(Some(attempt(row)?));
        }
        let pending:i64=sqlx::query_scalar("SELECT count(*) FROM payment_attempts WHERE state IN ('creating','unknown','open','processing','manual_review')").fetch_one(&mut *tx).await?;
        if pending >= 10000 {
            return Ok(None);
        }
        let recent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM payment_attempts WHERE owner_hash=$1 AND created_at>$2-86400",
        )
        .bind(input.owner_hash)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if recent >= 30 {
            return Ok(None);
        }
        let Some(row)=sqlx::query("SELECT * FROM checkout_drafts WHERE id=$1 AND owner_hash=$2 AND version=$3 AND expires_at>$4 AND quote_expires_at>$4 AND NOT locked FOR UPDATE").bind(input.draft_id).bind(input.owner_hash).bind(input.version).bind(now).fetch_optional(&mut *tx).await? else{return Ok(None)};
        let user: Option<String> = row.try_get("user_id")?;
        let quote: Value = row.try_get("quote")?;
        let Some(offer) = quote["offers"].as_array().and_then(|offers| {
            offers
                .iter()
                .find(|o| o["id"].as_str() == Some(input.offer_id))
        }) else {
            return Ok(None);
        };
        let Some(amount) = offer["total_grosze"]
            .as_i64()
            .filter(|v| (1..=99999999).contains(v))
        else {
            return Ok(None);
        };
        if input.offer_id.is_empty() || input.offer_id.len() > 128 {
            return Err("invalid fulfillment offer".into());
        }
        let draft_input: Value = row.try_get("input")?;
        let fulfillment = json!({"delivery":draft_input["delivery"],"offer":offer,"packing":quote["packing"],"point":quote["point"],"packaging_grosze":quote["packaging_grosze"]});
        if !bounded(&fulfillment, 131072) {
            return Err("fulfillment snapshot too large".into());
        }
        let ids: Vec<String> = serde_json::from_value(quote["quote_ids"].clone())?;
        // Also block first-time insertion of DB rates while using immutable process fallback.
        sqlx::query("LOCK TABLE pricing_config IN SHARE MODE")
            .execute(&mut *tx)
            .await?;
        let policy: Option<Value> =
            sqlx::query_scalar("SELECT value FROM pricing_config WHERE id=true FOR SHARE")
                .fetch_optional(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT q.request,m.production_enabled,m.cost_net_grosze_per_kg,c.in_stock FROM quotes q JOIN catalog_materials m ON m.id=q.request->>'material_id' JOIN catalog_colors c ON c.material_id=m.id AND c.code=q.request->>'color_code' WHERE q.id=ANY($1) ORDER BY m.id,c.code FOR SHARE OF m,c")
            .bind(&ids).fetch_all(&mut *tx).await?;
        let mut valid = 0;
        for row in rows {
            let snapshot: Value = row.try_get("request")?;
            let cost: Option<i64> = row.try_get("cost_net_grosze_per_kg")?;
            if row.get::<bool, _>("production_enabled")
                && row.get::<bool, _>("in_stock")
                && cost.is_some_and(|v| v > 0)
                && snapshot["purchase_net_grosze_per_kg"].as_i64() == cost
                && policy.as_ref().is_none_or(|p| snapshot["rates"] == *p)
            {
                valid += 1;
            }
        }
        if valid != ids.len() || !hold_files(&mut tx, input.id, input.owner_hash, &ids, now).await?
        {
            return Ok(None);
        }
        let contact = &quote["contact"];
        let recipient = contact["email"]
            .as_str()
            .filter(|s| s.len() <= 254 && s.contains('@') && !s.chars().any(char::is_control))
            .ok_or("invalid checkout email")?;
        let lines = &quote["lines"];
        if contact.to_string().len() > 8192 || lines.to_string().len() > 65536 {
            return Err("order snapshot too large".into());
        }
        let row=sqlx::query("INSERT INTO payment_attempts(id,draft_id,owner_hash,client_key_hash,fingerprint,state,provider_kind,amount_grosze,idempotency_key,provider_request,next_attempt,created_at,updated_at,offer_id) VALUES($1,$2,$3,$4,$5,'creating',$6,$7,$8,$9,$10,$10,$10,$11) RETURNING *")
            .bind(input.id).bind(input.draft_id).bind(input.owner_hash).bind(input.client_key_hash).bind(input.fingerprint).bind(input.provider_kind).bind(amount).bind(format!("rapidmaker:{}",input.id)).bind(input.provider_request).bind(now).bind(input.offer_id).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO orders(id,owner_hash,user_id,contact,lines,amount_grosze,provider_kind,provider_id,payment_attempt_id,guest_updates,created_at,fulfillment) VALUES($1,$2,$3,$4,$5,$6,$7,NULL,$1,$8,$9,$10)")
            .bind(input.id).bind(input.owner_hash).bind(user.as_deref()).bind(contact).bind(lines).bind(amount).bind(input.provider_kind).bind(quote["guest_updates"].as_bool().unwrap_or(true)).bind(now).bind(&fulfillment).execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            input.id,
            "confirmed",
            user.as_deref(),
            recipient,
            "order_confirmed",
            json!({"reference":input.id}),
            true,
            now,
        )
        .await?;
        sqlx::query("UPDATE checkout_drafts SET locked=true WHERE id=$1")
            .bind(input.draft_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(attempt(row)?))
    }
    pub async fn checkout_attempt(
        &self,
        id: &str,
        owner: &str,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        sqlx::query("SELECT * FROM payment_attempts WHERE id=$1 AND owner_hash=$2")
            .bind(id)
            .bind(owner)
            .fetch_optional(&self.pool)
            .await?
            .map(attempt)
            .transpose()
    }
    /// Workers reclaim abandoned leases. Creation is never retried beyond Stripe's 24h key window.
    pub async fn checkout_lease(
        &self,
        id: Option<&str>,
        now: i64,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE payment_attempts SET state='manual_review',lease_token=NULL,lease_until=NULL WHERE state IN ('creating','unknown','open','processing') AND ((provider_id IS NULL AND created_at<=$1-82800) OR attempts>=1000) AND (lease_until IS NULL OR lease_until<=$1)").bind(now).execute(&mut *tx).await?;
        let row=sqlx::query("UPDATE payment_attempts SET lease_token=$3,lease_until=$2+120,attempts=attempts+1 WHERE id=(SELECT id FROM payment_attempts WHERE ($1::text IS NULL OR id=$1) AND state IN ('creating','unknown','open','processing') AND next_attempt<=$2 AND (lease_until IS NULL OR lease_until<=$2) AND attempts<1000 ORDER BY next_attempt FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *")
            .bind(id).bind(now).bind(uuid::Uuid::new_v4().to_string()).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        row.map(attempt).transpose()
    }
    /// Record a creation response under lease; stale workers cannot overwrite a webhook result.
    pub async fn checkout_provider(
        &self,
        id: &str,
        lease: &str,
        provider: &str,
        response: &Value,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !bounded(response, 65536)
            || provider.len() > 128
            || !provider
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid provider result".into());
        }
        let mut tx = self.queue_transaction().await?;
        let changed=sqlx::query("UPDATE payment_attempts SET provider_id=$3,provider_response=$4,state='open',lease_token=NULL,lease_until=NULL,next_attempt=$5+30,updated_at=$5 WHERE id=$1 AND lease_token=$2 AND lease_until>$5 AND state IN ('creating','unknown') AND (provider_id IS NULL OR provider_id=$3)")
            .bind(id).bind(lease).bind(provider).bind(response).bind(now).execute(&mut *tx).await?.rows_affected()==1;
        if changed {
            sqlx::query("UPDATE orders SET provider_id=$2 WHERE payment_attempt_id=$1")
                .bind(id)
                .bind(provider)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }
    pub async fn checkout_retry(
        &self,
        id: &str,
        lease: &str,
        rejected: bool,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let row=sqlx::query("UPDATE payment_attempts SET state=CASE WHEN $3 AND state='creating' AND attempts=1 AND provider_id IS NULL THEN 'failed' WHEN $3 THEN 'manual_review' WHEN provider_id IS NULL THEN 'unknown' ELSE state END,lease_token=NULL,lease_until=NULL,next_attempt=$4+LEAST(3600,15*(1::bigint<<LEAST(attempts,8))),updated_at=$4 WHERE id=$1 AND lease_token=$2 AND lease_until>$4 AND state IN ('creating','unknown','open','processing') RETURNING draft_id,state")
            .bind(id).bind(lease).bind(rejected).bind(now).fetch_optional(&mut *tx).await?;
        if row
            .as_ref()
            .is_some_and(|r| r.get::<String, _>("state") == "failed")
        {
            sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
                .bind(id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE checkout_drafts SET locked=false WHERE id=$1")
                .bind(
                    row.as_ref()
                        .ok_or("missing failed attempt")?
                        .get::<String, _>("draft_id"),
                )
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(row.is_some())
    }
    /// Internal recovery for orders created before durable checkout attempts existed.
    pub async fn checkout_legacy_order(
        &self,
        kind: &str,
        provider: &str,
    ) -> Result<Option<Value>, StorageError> {
        Ok(sqlx::query_scalar("SELECT jsonb_build_object('id',id,'amount_grosze',amount_grosze,'provider_kind',provider_kind,'provider_id',provider_id,'status',status) FROM orders WHERE provider_kind=$1 AND provider_id=$2 AND payment_attempt_id IS NULL")
            .bind(kind).bind(provider).fetch_optional(&self.pool).await?)
    }
    /// Internal reconciliation lookup; HTTP callers must use the owner-scoped method.
    pub async fn checkout_attempt_id(
        &self,
        id: &str,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        sqlx::query("SELECT * FROM payment_attempts WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(attempt)
            .transpose()
    }
    pub async fn checkout_attempt_for_draft(
        &self,
        id: &str,
        owner: &str,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        sqlx::query("SELECT * FROM payment_attempts WHERE draft_id=$1 AND owner_hash=$2 ORDER BY (state NOT IN ('failed','expired')) DESC,created_at DESC,id DESC LIMIT 1").bind(id).bind(owner).fetch_optional(&self.pool).await?.map(attempt).transpose()
    }
    pub async fn checkout_attempt_provider(
        &self,
        kind: &str,
        provider: &str,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        sqlx::query("SELECT * FROM payment_attempts WHERE provider_kind=$1 AND provider_id=$2")
            .bind(kind)
            .bind(provider)
            .fetch_optional(&self.pool)
            .await?
            .map(attempt)
            .transpose()
    }
    /// A verified delayed-payment failure needs operator review; its intent may remain uncertain.
    pub async fn checkout_async_failed(
        &self,
        id: &str,
        provider: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE payment_attempts SET state='manual_review',lease_token=NULL,lease_until=NULL,updated_at=$3 WHERE id=$1 AND provider_kind='session' AND provider_id=$2 AND state='processing'")
            .bind(id).bind(provider).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    /// The caller retrieves a verified Stripe object. Never infer payment from a return URL.
    pub async fn checkout_resolve(
        &self,
        id: &str,
        kind: &str,
        object: &Value,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let Some(row) = sqlx::query("SELECT * FROM payment_attempts WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
        else {
            return Ok(false);
        };
        let Some(provider) = object["id"].as_str().filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        }) else {
            return Ok(false);
        };
        let existing: Option<String> = row.try_get("provider_id")?;
        let amount: i64 = row.try_get("amount_grosze")?;
        if row.get::<String, _>("provider_kind") != kind
            || existing.as_deref().is_some_and(|p| p != provider)
            || object["metadata"]["quote_id"].as_str() != Some(id)
            || object["livemode"] != false
            || object["currency"] != "pln"
            || object[if kind == "session" {
                "amount_total"
            } else {
                "amount"
            }]
            .as_i64()
                != Some(amount)
        {
            return Ok(false);
        }
        let paid = if kind == "session" {
            object["payment_status"] == "paid" && object["status"] == "complete"
        } else {
            object["status"] == "succeeded"
        };
        let closed = if kind == "session" {
            object["status"] == "expired" && object["payment_status"] != "paid"
        } else {
            object["status"] == "canceled"
        };
        let processing = if kind == "session" {
            object["status"] == "complete" && !paid
        } else {
            object["status"] == "processing"
        };
        let state: String = row.try_get("state")?;
        // Paid is absorbing. Late failure/expiry events must not undo payment or release files.
        if state == "paid" {
            tx.commit().await?;
            return Ok(true);
        }
        if !bounded(object, 65536) {
            return Err("provider object too large".into());
        }
        let next = if paid {
            "paid"
        } else if closed {
            "expired"
        } else if processing {
            "processing"
        } else if matches!(state.as_str(), "expired" | "failed") {
            state.as_str()
        } else {
            "open"
        };
        sqlx::query("UPDATE payment_attempts SET provider_id=$2,provider_response=$3,state=$4,lease_token=NULL,lease_until=NULL,next_attempt=$5+60,updated_at=$5 WHERE id=$1")
            .bind(id).bind(provider).bind(object).bind(next).bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE orders SET provider_id=$2 WHERE payment_attempt_id=$1")
            .bind(id)
            .bind(provider)
            .execute(&mut *tx)
            .await?;
        if paid {
            mark_paid(&mut tx, id, provider, amount, now).await?;
            sqlx::query("UPDATE order_files SET retain_until=NULL WHERE order_id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        } else if closed {
            sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
                .bind(id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE checkout_drafts SET locked=false WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM payment_attempts WHERE draft_id=$1 AND state IN ('creating','unknown','open','processing','paid','manual_review'))")
                .bind(row.get::<String,_>("draft_id")).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        if state != next {
            tracing::info!(event="payment_state_changed", attempt_id=id, from=%state, to=next);
        }
        Ok(true)
    }
    /// Acknowledge only after durable insertion. Duplicate events have no additional side effects.
    #[allow(clippy::too_many_arguments)]
    pub async fn stripe_event_accept(
        &self,
        id: &str,
        event_type: &str,
        kind: &str,
        object_id: &str,
        payload: &Value,
        now: i64,
    ) -> Result<bool, StorageError> {
        if id.is_empty()
            || id.len() > 128
            || event_type.is_empty()
            || event_type.len() > 128
            || object_id.is_empty()
            || object_id.len() > 128
            || !matches!(kind, "session" | "intent")
            || !bounded(payload, 262144)
            || payload["livemode"] != false
        {
            return Err("invalid Stripe event".into());
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(72617069803)")
            .execute(&mut *tx)
            .await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM stripe_events WHERE id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if exists {
            return Ok(false);
        }
        sqlx::query("DELETE FROM stripe_events WHERE state='done' AND finished_at<$1-2592000")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM stripe_events WHERE state<>'done'")
                .fetch_one(&mut *tx)
                .await?;
        if count >= 10000 {
            return Err("Stripe event quota".into());
        }
        sqlx::query("INSERT INTO stripe_events(id,event_type,provider_kind,object_id,payload,next_attempt,created_at) VALUES($1,$2,$3,$4,$5,$6,$6)").bind(id).bind(event_type).bind(kind).bind(object_id).bind(payload).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn stripe_event_lease(&self, now: i64) -> Result<Option<StripeEvent>, StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE stripe_events SET state='failed',lease_token=NULL,lease_until=NULL WHERE attempts>=20 AND state='processing' AND lease_until<=$1").bind(now).execute(&mut *tx).await?;
        let row=sqlx::query("UPDATE stripe_events SET state='processing',attempts=attempts+1,lease_token=$2,lease_until=$1+120 WHERE id=(SELECT id FROM stripe_events WHERE state IN ('pending','processing') AND next_attempt<=$1 AND (lease_until IS NULL OR lease_until<=$1) AND attempts<20 ORDER BY next_attempt FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *")
            .bind(now).bind(uuid::Uuid::new_v4().to_string()).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(StripeEvent {
                id: r.try_get("id")?,
                event_type: r.try_get("event_type")?,
                provider_kind: r.try_get("provider_kind")?,
                object_id: r.try_get("object_id")?,
                payload: r.try_get("payload")?,
                lease_token: r.try_get("lease_token")?,
                attempts: r.try_get("attempts")?,
            })
        })
        .transpose()
    }
    pub async fn stripe_event_finish(
        &self,
        id: &str,
        lease: &str,
        success: bool,
        now: i64,
    ) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE stripe_events SET state=CASE WHEN $3 THEN 'done' WHEN attempts>=20 THEN 'failed' ELSE 'pending' END,lease_token=NULL,lease_until=NULL,next_attempt=$4+LEAST(3600,15*(1::bigint<<LEAST(attempts,8))),finished_at=CASE WHEN $3 OR attempts>=20 THEN $4 ELSE NULL END WHERE id=$1 AND lease_token=$2 AND state='processing' AND lease_until>$4")
            .bind(id).bind(lease).bind(success).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn checkout_is_atomic_owned_idempotent_and_recovers_leases() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        let db = Storage::connect(&url).await.unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let id = uuid::Uuid::new_v4().to_string();
        let owner = crate::account_owner(&id);
        let material = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO catalog_materials(id,name,density_g_cm3,cost_net_grosze_per_kg,production_enabled,source) VALUES($1,'Test',1.2,5000,true,'{}')").bind(&material).execute(&db.pool).await.unwrap();
        sqlx::query("INSERT INTO catalog_colors(material_id,code,name,rgba,in_stock) VALUES($1,'test','Test','[\"#FFFFFFFF\"]',true)").bind(&material).execute(&db.pool).await.unwrap();
        let model = crate::StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: owner.clone(),
            user_id: None,
            filename: "test.stl".into(),
            extension: "stl".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 100,
            sha256: "a".repeat(64),
            dimensions_mm: [10.; 3],
            expires_at: 20000,
        };
        assert!(db.insert_model(&model).await.unwrap());
        let quoteid = uuid::Uuid::new_v4().to_string();
        let policy = db
            .pricing_config()
            .await
            .unwrap()
            .unwrap_or(json!({"version":"checkout-test"}));
        db.save_quote(&quoteid,&json!({"purchase_net_grosze_per_kg":5000,"rates":policy,"owner_hash":owner,"model_id":model.id,"model_sha256":model.sha256,"expires_at":20000,"material_id":material,"color_code":"test"}),&json!({})).await.unwrap();
        let draft = db
            .checkout_create(&owner, None, &json!({"parts":[]}), 10000)
            .await
            .unwrap();
        assert!(
            db.checkout_get(&draft.id, &"f".repeat(64), 10001)
                .await
                .unwrap()
                .is_none()
        );
        let update = json!({"parts":[],"delivery":"courier"});
        let (a, b) = tokio::join!(
            db.checkout_update(&draft.id, &owner, 1, &update, 10001),
            db.checkout_update(&draft.id, &owner, 1, &update, 10001)
        );
        assert_eq!(
            usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
            1
        );
        let quote = json!({"offers":[{"id":"pickup","name":"Selected carrier","total_grosze":2500},{"id":"other","name":"Other carrier","total_grosze":2500}],"packing":{"parcels":[{"weight_g":500,"dimensions_mm":[100,200,300]}]},"packaging_grosze":250,"quote_ids":[quoteid],"contact":{"email":"checkout@example.invalid"},"lines":[],"guest_updates":true});
        let saved = db
            .checkout_quote(&draft.id, &owner, 2, &quote, 10600, 10002)
            .await
            .unwrap()
            .unwrap();
        let params = json!([["amount", "2500"], ["metadata[quote_id]", id]]);
        let key = "a".repeat(64);
        let fingerprint = "b".repeat(64);
        let input = AttemptInput {
            id: &id,
            draft_id: &draft.id,
            owner_hash: &owner,
            version: saved.version,
            offer_id: "pickup",
            client_key_hash: &key,
            fingerprint: &fingerprint,
            provider_kind: "intent",
            provider_request: &params,
        };
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=6000 WHERE id=$1")
            .bind(&material)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db.checkout_prepare(&input, 10003).await.unwrap().is_none());
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=5000 WHERE id=$1")
            .bind(&material)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO pricing_config(id,value) VALUES(true,$1) ON CONFLICT(id) DO UPDATE SET value=EXCLUDED.value").bind(json!({"version":"changed-during-checkout"})).execute(&db.pool).await.unwrap();
        assert!(db.checkout_prepare(&input, 10003).await.unwrap().is_none());
        sqlx::query("UPDATE pricing_config SET value=$1 WHERE id=true")
            .bind(&policy)
            .execute(&db.pool)
            .await
            .unwrap();
        let (first, second) = tokio::join!(
            db.checkout_prepare(&input, 10003),
            db.checkout_prepare(&input, 10003)
        );
        assert_eq!(first.unwrap().unwrap().id, second.unwrap().unwrap().id);
        assert!(
            db.checkout_update(&draft.id, &owner, 3, &json!({}), 10004)
                .await
                .unwrap()
                .is_none()
        );
        let changed = AttemptInput {
            fingerprint: &"c".repeat(64),
            ..input
        };
        assert!(
            db.checkout_prepare(&changed, 10004)
                .await
                .unwrap()
                .is_none()
        );
        let lease = db.checkout_lease(Some(&id), 10005).await.unwrap().unwrap();
        assert!(db.checkout_lease(Some(&id), 10006).await.unwrap().is_none());
        assert!(
            db.checkout_retry(&id, lease.lease_token.as_deref().unwrap(), false, 10006)
                .await
                .unwrap()
        );
        assert!(
            db.checkout_get(&draft.id, &owner, 10007)
                .await
                .unwrap()
                .unwrap()
                .locked
        );
        let fresh = Storage::connect(&url).await.unwrap();
        let recovered = fresh
            .checkout_lease(Some(&id), 10100)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(recovered.offer_id.as_deref(), Some("pickup"));
        let fulfillment: Value = sqlx::query_scalar("SELECT fulfillment FROM orders WHERE id=$1")
            .bind(&id)
            .fetch_one(&fresh.pool)
            .await
            .unwrap();
        assert_eq!(fulfillment["offer"]["name"], "Selected carrier");
        assert_eq!(fulfillment["delivery"], "courier");
        assert_eq!(fulfillment["packing"]["parcels"][0]["weight_g"], 500);
        assert_eq!(recovered.state, "unknown");
        assert_eq!(recovered.provider_request, params);
        let stale = recovered.lease_token.clone().unwrap();
        let replacement = fresh
            .checkout_lease(Some(&id), 10221)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(replacement.lease_token, recovered.lease_token);
        assert!(
            !fresh
                .checkout_provider(&id, &stale, "pi_test", &json!({}), 10222)
                .await
                .unwrap()
        );
        let mut paid = json!({"id":"pi_test","object":"payment_intent","metadata":{"quote_id":id},"amount":2500,"currency":"pln","livemode":false,"status":"succeeded"});
        paid["amount"] = json!(1);
        assert!(
            !fresh
                .checkout_resolve(&id, "intent", &paid, 10223)
                .await
                .unwrap()
        );
        paid["amount"] = json!(2500);
        assert!(
            fresh
                .checkout_resolve(&id, "intent", &paid, 10223)
                .await
                .unwrap()
        );
        assert!(
            fresh
                .checkout_resolve(&id, "intent", &paid, 10224)
                .await
                .unwrap()
        );
        paid["status"] = json!("canceled");
        assert!(
            fresh
                .checkout_resolve(&id, "intent", &paid, 10225)
                .await
                .unwrap()
        );
        assert_eq!(
            fresh.checkout_attempt_id(&id).await.unwrap().unwrap().state,
            "paid"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT status FROM orders WHERE id=$1")
                .bind(&id)
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            "paid"
        );
        let mails: i64 = sqlx::query_scalar("SELECT count(*) FROM email_outbox WHERE event_key=$1")
            .bind(format!("order:{id}:paid"))
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(mails, 1);
        let until: Option<i64> =
            sqlx::query_scalar("SELECT retain_until FROM order_files WHERE order_id=$1")
                .bind(&id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(until.is_none());
        assert!(
            !fresh
                .checkout_provider(
                    &id,
                    replacement.lease_token.as_deref().unwrap(),
                    "pi_test",
                    &json!({}),
                    10226
                )
                .await
                .unwrap()
        );
        for (rejected_after_unknown, offset) in [(false, 0), (true, 100)] {
            let draft = db
                .checkout_create(&owner, None, &json!({}), 12000 + offset)
                .await
                .unwrap();
            let quoted = db
                .checkout_quote(&draft.id, &owner, 1, &quote, 12500 + offset, 12001 + offset)
                .await
                .unwrap()
                .unwrap();
            let new_id = uuid::Uuid::new_v4().to_string();
            let key = crate::account_owner(&new_id);
            let next = AttemptInput {
                id: &new_id,
                draft_id: &draft.id,
                owner_hash: &owner,
                version: quoted.version,
                offer_id: "pickup",
                client_key_hash: &key,
                fingerprint: &fingerprint,
                provider_kind: "intent",
                provider_request: &params,
            };
            db.checkout_prepare(&next, 12002 + offset)
                .await
                .unwrap()
                .unwrap();
            let first = db
                .checkout_lease(Some(&new_id), 12003 + offset)
                .await
                .unwrap()
                .unwrap();
            if rejected_after_unknown {
                db.checkout_retry(
                    &new_id,
                    first.lease_token.as_deref().unwrap(),
                    false,
                    12004 + offset,
                )
                .await
                .unwrap();
                let second = db
                    .checkout_lease(Some(&new_id), 12050 + offset)
                    .await
                    .unwrap()
                    .unwrap();
                db.checkout_retry(
                    &new_id,
                    second.lease_token.as_deref().unwrap(),
                    true,
                    12051 + offset,
                )
                .await
                .unwrap();
            } else {
                db.checkout_retry(
                    &new_id,
                    first.lease_token.as_deref().unwrap(),
                    true,
                    12004 + offset,
                )
                .await
                .unwrap();
            }
            let final_attempt = db.checkout_attempt_id(&new_id).await.unwrap().unwrap();
            assert_eq!(
                final_attempt.state,
                if rejected_after_unknown {
                    "manual_review"
                } else {
                    "failed"
                }
            );
            assert_eq!(
                db.checkout_get(&draft.id, &owner, 12100 + offset)
                    .await
                    .unwrap()
                    .unwrap()
                    .locked,
                rejected_after_unknown
            );
            let retained: Option<i64> =
                sqlx::query_scalar("SELECT retain_until FROM order_files WHERE order_id=$1")
                    .bind(&new_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(retained.is_none(), rejected_after_unknown);
        }
        assert!(
            !db.checkout_async_failed(&id, "pi_test", 13000)
                .await
                .unwrap()
        );
        let delayed_draft = db
            .checkout_create(&owner, None, &json!({"delivery":"courier"}), 13000)
            .await
            .unwrap();
        let delayed_quote = db
            .checkout_quote(&delayed_draft.id, &owner, 1, &quote, 13500, 13001)
            .await
            .unwrap()
            .unwrap();
        let delayed_id = uuid::Uuid::new_v4().to_string();
        let delayed_key = crate::account_owner(&delayed_id);
        let delayed_input = AttemptInput {
            id: &delayed_id,
            draft_id: &delayed_draft.id,
            owner_hash: &owner,
            version: delayed_quote.version,
            offer_id: "pickup",
            client_key_hash: &delayed_key,
            fingerprint: &fingerprint,
            provider_kind: "session",
            provider_request: &params,
        };
        db.checkout_prepare(&delayed_input, 13002)
            .await
            .unwrap()
            .unwrap();
        let delayed = json!({"id":"cs_delayed","metadata":{"quote_id":delayed_id},"amount_total":2500,"currency":"pln","livemode":false,"status":"complete","payment_status":"unpaid"});
        assert!(
            db.checkout_resolve(&delayed_id, "session", &delayed, 13003)
                .await
                .unwrap()
        );
        assert!(
            !db.checkout_async_failed(&delayed_id, "cs_other", 13004)
                .await
                .unwrap()
        );
        assert!(
            db.checkout_async_failed(&delayed_id, "cs_delayed", 13004)
                .await
                .unwrap()
        );
        assert_eq!(
            db.checkout_attempt_id(&delayed_id)
                .await
                .unwrap()
                .unwrap()
                .state,
            "manual_review"
        );
        let delayed_retention: Option<i64> =
            sqlx::query_scalar("SELECT retain_until FROM order_files WHERE order_id=$1")
                .bind(&delayed_id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert!(delayed_retention.is_none());
        let user = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,13000)")
            .bind(&user)
            .bind(format!("{user}@example.invalid"))
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db.claim_guest(&user, &owner, 13001).await.unwrap() > 0);
        let account = crate::account_owner(&user);
        assert!(db.checkout_attempt(&id, &owner).await.unwrap().is_none());
        assert!(db.checkout_attempt(&id, &account).await.unwrap().is_some());
        let list = db.orders(Some(&user), None).await.unwrap();
        assert_eq!(
            list.iter().find(|o| o["id"] == id).unwrap()["payment_state"],
            "paid"
        );
        let stored = db.order(&id, Some(&user), None).await.unwrap().unwrap();
        assert_eq!(stored["payment_state"], "paid");
        assert!(stored.get("owner_hash").is_none());
        assert!(
            db.admin_orders()
                .await
                .unwrap()
                .iter()
                .any(|o| o["payment_state"] == "manual_review")
        );
        assert!(
            db.checkout_legacy_order("intent", "pi_test")
                .await
                .unwrap()
                .is_none()
        );
        let legacy = uuid::Uuid::new_v4().to_string();
        assert!(
            db.hold_order_files(&legacy, &account, &[quoteid], 13002)
                .await
                .unwrap()
        );
        db.create_order(
            &crate::OrderInput {
                id: &legacy,
                owner_hash: &account,
                user_id: Some(&user),
                contact: &json!({"email":"checkout@example.invalid"}),
                lines: &json!([]),
                amount_grosze: 2500,
                provider_kind: "intent",
                provider_id: "pi_legacy",
                guest_updates: true,
            },
            13002,
        )
        .await
        .unwrap();
        let legacy_order = db
            .checkout_legacy_order("intent", "pi_legacy")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(legacy_order["id"], legacy);
        assert_eq!(legacy_order["amount_grosze"], 2500);
        assert!(
            db.checkout_legacy_order("session", "pi_legacy")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !db.checkout_provider_allowed("invalid", 20000)
                .await
                .unwrap()
        );
        for _ in 0..30 {
            assert!(db.checkout_provider_allowed(&owner, 20000).await.unwrap());
        }
        assert!(!db.checkout_provider_allowed(&owner, 20000).await.unwrap());
        assert!(db.checkout_provider_allowed(&owner, 20060).await.unwrap());
        let eventid = format!("evt_{id}");
        let payload =
            json!({"livemode":false,"data":{"object":{"id":"pi_test","metadata":{"quote_id":id}}}});
        assert!(
            db.stripe_event_accept(
                &eventid,
                "payment_intent.succeeded",
                "intent",
                "pi_test",
                &payload,
                11000
            )
            .await
            .unwrap()
        );
        assert!(
            !db.stripe_event_accept(
                &eventid,
                "payment_intent.succeeded",
                "intent",
                "pi_test",
                &payload,
                11000
            )
            .await
            .unwrap()
        );
        let event = db.stripe_event_lease(11001).await.unwrap().unwrap();
        assert!(db.stripe_event_lease(11002).await.unwrap().is_none());
        assert!(
            !db.stripe_event_finish(&eventid, "stale", true, 11003)
                .await
                .unwrap()
        );
        assert!(
            db.stripe_event_finish(&eventid, &event.lease_token, true, 11003)
                .await
                .unwrap()
        );
        assert!(db.stripe_event_lease(11004).await.unwrap().is_none());
    }
}
