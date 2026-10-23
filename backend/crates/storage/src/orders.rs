//! Business writes and email enqueue share a PostgreSQL transaction.
use crate::email::EmailInput;
use crate::{Storage, StorageError};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};

pub struct OrderInput<'a> {
    pub id: &'a str,
    pub owner_hash: &'a str,
    pub user_id: Option<&'a str>,
    pub contact: &'a Value,
    pub lines: &'a Value,
    pub amount_grosze: i64,
    pub provider_kind: &'a str,
    pub provider_id: &'a str,
    pub guest_updates: bool,
}
impl Storage {
    pub async fn poll_order_payment(&self, id: &str, now: i64) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE orders SET payment_checked_at=$2 WHERE id=$1 AND status='awaiting_payment' AND payment_checked_at<=$2-15")
            .bind(id).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn create_order(&self, o: &OrderInput<'_>, now: i64) -> Result<(), StorageError> {
        if uuid::Uuid::parse_str(o.id).is_err()
            || o.provider_id.len() > 128
            || !o
                .provider_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid order identifier".into());
        }
        let recipient = o.contact["email"]
            .as_str()
            .filter(|s| s.len() <= 254 && s.contains('@') && !s.chars().any(char::is_control))
            .ok_or("invalid order email")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(72617069801)")
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query(
            "SELECT owner_hash,amount_grosze,provider_id FROM orders WHERE id=$1 FOR UPDATE",
        )
        .bind(o.id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = existing {
            if row.get::<String, _>("owner_hash") != o.owner_hash
                || row.get::<i64, _>("amount_grosze") != o.amount_grosze
                || row.get::<String, _>("provider_id") != o.provider_id
            {
                return Err("order changed".into());
            }
            tx.commit().await?;
            return Ok(());
        }
        let recent: i64 =
            sqlx::query_scalar("SELECT count(*) FROM orders WHERE owner_hash=$1 AND created_at>$2")
                .bind(o.owner_hash)
                .bind(now - 86400)
                .fetch_one(&mut *tx)
                .await?;
        if recent >= 30 {
            return Err("order limit".into());
        }
        sqlx::query("INSERT INTO orders(id,owner_hash,user_id,contact,lines,amount_grosze,provider_kind,provider_id,guest_updates,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(o.id).bind(o.owner_hash).bind(o.user_id).bind(o.contact).bind(o.lines).bind(o.amount_grosze).bind(o.provider_kind).bind(o.provider_id).bind(o.guest_updates).bind(now).execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            o.id,
            "confirmed",
            o.user_id,
            recipient,
            "order_confirmed",
            json!({"reference":o.id}),
            true,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn orders(
        &self,
        user: Option<&str>,
        owner: Option<&str>,
    ) -> Result<Vec<Value>, StorageError> {
        let rows=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'status',status,'amount_grosze',amount_grosze,'created_at',created_at,'paid_at',paid_at) FROM orders WHERE user_id=$1 OR (user_id IS NULL AND owner_hash=$2 AND created_at>extract(epoch from now())::bigint-86400) ORDER BY created_at DESC LIMIT 100")
            .bind(user).bind(owner).fetch_all(&self.pool).await?;
        Ok(rows)
    }
    pub async fn order(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
    ) -> Result<Option<Value>, StorageError> {
        Ok(sqlx::query_scalar("SELECT to_jsonb(o)-'owner_hash'-'user_id' FROM orders o WHERE id=$1 AND (user_id=$2 OR (user_id IS NULL AND owner_hash=$3 AND created_at>extract(epoch from now())::bigint-86400))")
            .bind(id).bind(user).bind(owner).fetch_optional(&self.pool).await?)
    }
    pub async fn admin_orders(&self) -> Result<Vec<Value>, StorageError> {
        Ok(sqlx::query_scalar(
            "SELECT to_jsonb(o)-'owner_hash' FROM orders o ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?)
    }
    pub async fn order_paid(
        &self,
        id: &str,
        provider: &str,
        amount: i64,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let row=sqlx::query("SELECT user_id,contact FROM orders WHERE id=$1 AND provider_id=$2 AND amount_grosze=$3 AND status='awaiting_payment' FOR UPDATE")
            .bind(id).bind(provider).bind(amount).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(false);
        };
        let user: Option<String> = row.try_get("user_id")?;
        let contact: Value = row.try_get("contact")?;
        sqlx::query("UPDATE orders SET status='paid',paid_at=$2 WHERE id=$1")
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        enqueue(
            &mut tx,
            id,
            "paid",
            user.as_deref(),
            contact["email"].as_str().ok_or("invalid order contact")?,
            "payment_received",
            json!({"reference":id,"amount_grosze":amount}),
            true,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn order_status(
        &self,
        id: &str,
        status: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT status,user_id,contact,guest_updates FROM orders WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else { return Ok(false) };
        let old: String = row.try_get("status")?;
        let allowed = matches!(
            (old.as_str(), status),
            ("paid", "in_production")
                | ("in_production", "ready")
                | ("ready", "shipped")
                | ("ready", "completed")
                | ("shipped", "completed")
        );
        if !allowed {
            return Ok(false);
        }
        let user: Option<String> = row.try_get("user_id")?;
        let contact: Value = row.try_get("contact")?;
        sqlx::query("UPDATE orders SET status=$2 WHERE id=$1")
            .bind(id)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        enqueue(
            &mut tx,
            id,
            status,
            user.as_deref(),
            contact["email"].as_str().ok_or("invalid order contact")?,
            "status_changed",
            json!({"reference":id,"status":status}),
            row.try_get("guest_updates")?,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn order_messages(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
    ) -> Result<Option<Vec<Value>>, StorageError> {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM orders WHERE id=$1 AND ($4 OR user_id=$2 OR (user_id IS NULL AND owner_hash=$3 AND created_at>extract(epoch from now())::bigint-86400)))").bind(id).bind(user).bind(owner).bind(operator).fetch_one(&self.pool).await?;
        if !exists {
            return Ok(None);
        }
        Ok(Some(sqlx::query_scalar("SELECT jsonb_build_object('id',id,'operator',operator,'body',body,'created_at',created_at) FROM order_messages WHERE order_id=$1 ORDER BY created_at,id LIMIT 200").bind(id).fetch_all(&self.pool).await?))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn order_message(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
        body: &str,
        admin_to: Option<&str>,
        now: i64,
    ) -> Result<bool, StorageError> {
        if body.trim().is_empty()
            || body.len() > 4000
            || body
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("invalid message".into());
        }
        let mut tx = self.pool.begin().await?;
        let row=sqlx::query("SELECT user_id,contact,guest_updates FROM orders WHERE id=$1 AND ($4 OR user_id=$2 OR (user_id IS NULL AND owner_hash=$3 AND created_at>extract(epoch from now())::bigint-86400)) FOR UPDATE")
            .bind(id).bind(user).bind(owner).bind(operator).fetch_optional(&mut *tx).await?;
        let Some(row) = row else { return Ok(false) };
        let counts=sqlx::query("SELECT count(*) AS total,count(*) FILTER(WHERE created_at>$2) AS recent FROM order_messages WHERE order_id=$1").bind(id).bind(now-60).fetch_one(&mut *tx).await?;
        if counts.get::<i64, _>("total") >= 200 || counts.get::<i64, _>("recent") >= 10 {
            return Err("message limit".into());
        }
        let message_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO order_messages(id,order_id,author_id,operator,body,created_at) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(&message_id).bind(id).bind(user).bind(operator).bind(body).bind(now).execute(&mut *tx).await?;
        let order_user: Option<String> = row.try_get("user_id")?;
        let contact: Value = row.try_get("contact")?;
        let recipient = if operator {
            contact["email"].as_str()
        } else {
            admin_to
        };
        if let Some(to) = recipient {
            enqueue(
                &mut tx,
                id,
                &format!("message:{message_id}"),
                if operator {
                    order_user.as_deref()
                } else {
                    None
                },
                to,
                "chat_message",
                json!({"reference":id}),
                if operator {
                    row.try_get("guest_updates")?
                } else {
                    true
                },
                now,
            )
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }
    pub async fn claim_orders(&self, user: &str, owner: &str) -> Result<u64, StorageError> {
        let mut tx = self.pool.begin().await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "UPDATE orders SET user_id=$1 WHERE owner_hash=$2 AND user_id IS NULL AND created_at>extract(epoch from now())::bigint-86400 RETURNING id",
        )
        .bind(user)
        .bind(owner)
        .fetch_all(&mut *tx)
        .await?;
        for id in &rows {
            sqlx::query("UPDATE email_outbox SET user_id=$1 WHERE event_key LIKE $2 AND user_id IS NULL AND status IN ('pending','sending')").bind(user).bind(format!("order:{id}:%")).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(rows.len() as u64)
    }
}

#[allow(clippy::too_many_arguments)]
async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    event: &str,
    user: Option<&str>,
    recipient: &str,
    kind: &str,
    fields: Value,
    enabled: bool,
    now: i64,
) -> Result<(), StorageError> {
    let mut notification = fields;
    notification["kind"] = json!(kind);
    super::email::enqueue(
        tx,
        &EmailInput {
            id: uuid::Uuid::new_v4().to_string(),
            event_key: format!("order:{id}:{event}"),
            user_id: user.map(str::to_owned),
            kind: kind.into(),
            recipient: recipient.into(),
            payload: json!({"version":1,"notification":notification}),
            optional_enabled: enabled,
        },
        now,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires an isolated TEST_DATABASE_URL"]
    async fn order_events_are_owned_atomic_and_respect_preferences() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("rapidmaker_test")
        );
        let db = Storage::connect(&url).await.unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let user = uuid::Uuid::new_v4().to_string();
        let owner = "ab".repeat(32);
        let contact = json!({"email":"order@example.invalid"});
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,1000)")
            .bind(&user)
            .bind(format!("{user}@example.invalid"))
            .execute(&db.pool)
            .await
            .unwrap();
        let o = OrderInput {
            id: &id,
            owner_hash: &owner,
            user_id: Some(&user),
            contact: &contact,
            lines: &json!([]),
            amount_grosze: 1234,
            provider_kind: "intent",
            provider_id: &format!("pi_{}", id.replace('-', "")),
            guest_updates: true,
        };
        db.create_order(&o, 1000).await.unwrap();
        db.create_order(&o, 1000).await.unwrap();
        assert!(db.poll_order_payment(&id, 1000).await.unwrap());
        assert!(!db.poll_order_payment(&id, 1001).await.unwrap());
        assert!(db.order(&id, Some("other"), None).await.unwrap().is_none());
        assert!(
            !db.order_message(&id, Some("other"), None, false, "forged", None, 1001)
                .await
                .unwrap()
        );
        assert!(
            db.order(&id, None, Some(&owner)).await.unwrap().is_none(),
            "account order must not fall back to guest cookie"
        );
        assert!(!db.order_paid(&id, o.provider_id, 1235, 1002).await.unwrap());
        assert!(db.order_paid(&id, o.provider_id, 1234, 1002).await.unwrap());
        assert!(!db.order_paid(&id, o.provider_id, 1234, 1002).await.unwrap());
        db.auth_preferences(&user, false, false).await.unwrap();
        assert!(db.order_status(&id, "in_production", 1003).await.unwrap());
        assert!(!db.order_status(&id, "paid", 1004).await.unwrap());
        assert!(
            db.order_message(&id, None, None, true, "Private content", None, 1005)
                .await
                .unwrap()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM email_outbox WHERE event_key LIKE $1")
                .bind(format!("order:{id}:%"))
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            count, 2,
            "only mandatory confirmation and payment, neither optional mail"
        );
        // A failed queue write must roll back the matching business transition.
        sqlx::query(&format!("ALTER TABLE email_outbox ADD CONSTRAINT reject_test_event CHECK(kind!='status_changed' OR payload->'notification'->>'reference' != '{id}')")).execute(&db.pool).await.unwrap();
        db.auth_preferences(&user, true, true).await.unwrap();
        assert!(db.order_status(&id, "ready", 1006).await.is_err());
        assert_eq!(
            db.order(&id, Some(&user), None).await.unwrap().unwrap()["status"],
            "in_production"
        );
        sqlx::query("ALTER TABLE email_outbox DROP CONSTRAINT reject_test_event")
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
