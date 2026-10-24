use crate::{Storage, StorageError};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};

pub(crate) struct EmailInput {
    pub id: String,
    pub event_key: String,
    pub user_id: Option<String>,
    pub kind: String,
    pub recipient: String,
    pub payload: Value,
    pub optional_enabled: bool,
}

pub struct QueuedEmail {
    pub id: String,
    pub recipient: String,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
    pub locked_until: i64,
}

pub enum EmailOutcome {
    Sent,
    Skipped,
    Retry(&'static str),
    Failed(&'static str),
    Deferred(&'static str),
}

#[derive(Serialize)]
pub struct EmailStats {
    pub pending: i64,
    pub sending: i64,
    pub failed: i64,
    pub oldest_pending_at: Option<i64>,
}

fn validate(input: &EmailInput) -> Result<(), StorageError> {
    let field = |v: &str| !v.is_empty() && v.len() <= 160 && !v.chars().any(char::is_control);
    let data = input
        .payload
        .get("notification")
        .and_then(Value::as_object)
        .ok_or("invalid email payload")?;
    let valid = input.payload.as_object().is_some_and(|v| v.len() == 2)
        && input.payload.get("version").and_then(Value::as_u64) == Some(1)
        && data.get("kind").and_then(Value::as_str) == Some(input.kind.as_str())
        && data
            .get("reference")
            .and_then(Value::as_str)
            .is_some_and(field)
        && match input.kind.as_str() {
            "order_confirmed" => data.len() == 2,
            "chat_message" => {
                data.len() == 2
                    || (data.len() == 7
                        && data
                            .get("body")
                            .and_then(Value::as_str)
                            .is_some_and(|v| !v.is_empty() && v.len() <= 6000)
                        && data.get("operator").and_then(Value::as_bool).is_some()
                        && data
                            .get("reply_route")
                            .and_then(Value::as_str)
                            .is_some_and(|v| uuid::Uuid::parse_str(v).is_ok())
                        && data
                            .get("first_sequence")
                            .and_then(Value::as_i64)
                            .is_some_and(|v| v > 0)
                        && data
                            .get("last_sequence")
                            .and_then(Value::as_i64)
                            .is_some_and(|v| v > 0))
            }
            "payment_received" => {
                data.len() == 3 && data.get("amount_grosze").and_then(Value::as_u64).is_some()
            }
            "status_changed" => {
                data.len() == 3
                    && data
                        .get("status")
                        .and_then(Value::as_str)
                        .is_some_and(field)
            }
            _ => false,
        };
    if !valid
        || input.payload.to_string().len() > 16384
        || input.recipient.is_empty()
        || input.recipient.len() > 254
        || input.recipient.chars().any(char::is_control)
        || !input.recipient.contains('@')
        || input.event_key.is_empty()
        || input.event_key.len() > 256
    {
        return Err("invalid email snapshot".into());
    }
    Ok(())
}

/// Called within the business transaction; no public HTTP endpoint may supply these snapshots.
pub(crate) async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    input: &EmailInput,
    now: i64,
) -> Result<bool, StorageError> {
    validate(input)?;
    // Serialize admission so concurrent events cannot exceed the global backlog cap.
    sqlx::query("SELECT pg_advisory_xact_lock(18432005)")
        .execute(&mut **tx)
        .await?;
    if sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM email_outbox WHERE event_key=$1)")
        .bind(&input.event_key)
        .fetch_one(&mut **tx)
        .await?
    {
        return Ok(false);
    }
    let allowed: bool = sqlx::query_scalar("SELECT CASE WHEN $1::text IS NULL THEN $3 ELSE EXISTS(SELECT 1 FROM users WHERE id=$1 AND disabled_at IS NULL AND CASE $2 WHEN 'status_changed' THEN email_order_updates WHEN 'chat_message' THEN email_chat_messages ELSE true END) END")
        .bind(&input.user_id).bind(&input.kind).bind(input.optional_enabled || matches!(input.kind.as_str(), "order_confirmed" | "payment_received"))
        .fetch_one(&mut **tx).await?;
    if !allowed {
        return Ok(false);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM email_outbox WHERE status IN ('pending','sending')",
    )
    .fetch_one(&mut **tx)
    .await?;
    if count >= 10000 {
        return Err("email backlog capacity reached".into());
    }
    sqlx::query("INSERT INTO email_outbox (id,event_key,user_id,kind,recipient,payload,optional_enabled,available_at,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8)")
        .bind(&input.id).bind(&input.event_key).bind(&input.user_id).bind(&input.kind)
        .bind(&input.recipient).bind(&input.payload).bind(input.optional_enabled).bind(now).execute(&mut **tx).await?;
    Ok(true)
}

impl Storage {
    /// Claims one job; the SMTP call must happen after this transaction commits.
    pub async fn claim_email(&self, now: i64) -> Result<Option<QueuedEmail>, StorageError> {
        let mut tx = self.pool.begin().await?;
        // Exhausted crashed attempts must not stay stuck in `sending` forever.
        sqlx::query("UPDATE email_outbox SET status='failed',finished_at=$1,locked_until=NULL,last_error_code='lease_exhausted' WHERE id IN (SELECT id FROM email_outbox WHERE status='sending' AND locked_until<=$1 AND attempts>=5 LIMIT 100 FOR UPDATE SKIP LOCKED)")
            .bind(now).execute(&mut *tx).await?;
        let row = sqlx::query("WITH next AS (SELECT id FROM email_outbox WHERE (status='pending' AND available_at<=$1 OR status='sending' AND locked_until<=$1) AND attempts<5 ORDER BY available_at,id LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE email_outbox e SET status='sending',chat_frozen=true,attempts=attempts+1,locked_until=$1+60 FROM next WHERE e.id=next.id RETURNING e.id,e.recipient,e.kind,e.payload,e.attempts,e.locked_until")
            .bind(now).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        row.map(|r| {
            Ok(QueuedEmail {
                id: r.try_get("id")?,
                recipient: r.try_get("recipient")?,
                kind: r.try_get("kind")?,
                payload: r.try_get("payload")?,
                attempts: r.try_get("attempts")?,
                locked_until: r.try_get("locked_until")?,
            })
        })
        .transpose()
    }

    /// Recheck preferences immediately before SMTP, including changes after enqueue.
    pub async fn email_allowed(&self, mail: &QueuedEmail) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM email_outbox e LEFT JOIN users u ON u.id=e.user_id WHERE e.id=$1 AND e.status='sending' AND e.attempts=$2 AND e.locked_until=$3 AND CASE WHEN e.user_id IS NULL THEN (e.optional_enabled OR e.kind IN ('order_confirmed','payment_received')) ELSE u.disabled_at IS NULL AND CASE e.kind WHEN 'status_changed' THEN u.email_order_updates WHEN 'chat_message' THEN u.email_chat_messages AND u.email=e.recipient AND (NOT COALESCE(e.chat_operator,false) OR u.role='admin') ELSE true END END AND (e.reply_route IS NULL OR EXISTS(SELECT 1 FROM chat_reply_routes r JOIN orders o ON o.id=r.order_id WHERE r.id=e.reply_route AND r.revoked_at IS NULL AND r.expires_at>extract(epoch from now())::bigint AND (r.operator OR o.user_id=r.user_id OR (o.user_id IS NULL AND r.user_id IS NULL)))))")
            .bind(&mail.id).bind(mail.attempts).bind(mail.locked_until).fetch_one(&self.pool).await?)
    }

    /// The attempt/lease comparison prevents a late worker overwriting a reclaimed job.
    pub async fn finish_email(
        &self,
        mail: &QueuedEmail,
        outcome: EmailOutcome,
        now: i64,
    ) -> Result<bool, StorageError> {
        let (status, code, delay, attempts) = match outcome {
            EmailOutcome::Sent => ("sent", None, 0, mail.attempts),
            EmailOutcome::Skipped => ("skipped", None, 0, mail.attempts),
            EmailOutcome::Failed(code) => ("failed", Some(code), 0, mail.attempts),
            EmailOutcome::Retry(code) if mail.attempts >= 5 => {
                ("failed", Some(code), 0, mail.attempts)
            }
            EmailOutcome::Retry(code) => {
                let delay = [60, 300, 1800, 7200][(mail.attempts - 1).clamp(0, 3) as usize];
                let jitter = mail
                    .id
                    .bytes()
                    .fold(0u64, |a, b| a.wrapping_mul(31).wrapping_add(u64::from(b)))
                    % 16;
                ("pending", Some(code), delay + jitter as i64, mail.attempts)
            }
            EmailOutcome::Deferred(code) => ("pending", Some(code), 60, mail.attempts - 1),
        };
        let result = sqlx::query("UPDATE email_outbox SET status=$4,last_error_code=$5,available_at=$6,attempts=$7,locked_until=NULL,sent_at=CASE WHEN $4='sent' THEN $8 ELSE NULL END,finished_at=CASE WHEN $4 IN ('sent','failed','skipped') THEN $8 ELSE NULL END WHERE id=$1 AND status='sending' AND attempts=$2 AND locked_until=$3")
            .bind(&mail.id).bind(mail.attempts).bind(mail.locked_until).bind(status).bind(code)
            .bind(now+delay).bind(attempts).bind(now).execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn email_stats(&self) -> Result<EmailStats, StorageError> {
        let r = sqlx::query("SELECT count(*) FILTER (WHERE status='pending') AS pending,count(*) FILTER (WHERE status='sending') AS sending,count(*) FILTER (WHERE status='failed') AS failed,min(created_at) FILTER (WHERE status='pending') AS oldest_pending_at FROM email_outbox").fetch_one(&self.pool).await?;
        Ok(EmailStats {
            pending: r.try_get("pending")?,
            sending: r.try_get("sending")?,
            failed: r.try_get("failed")?,
            oldest_pending_at: r.try_get("oldest_pending_at")?,
        })
    }

    pub async fn cleanup_email(&self, now: i64) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM email_outbox WHERE id IN (SELECT id FROM email_outbox WHERE finished_at<$1 LIMIT 100 FOR UPDATE SKIP LOCKED)")
            .bind(now-30*86400).execute(&self.pool).await?.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn input(id: &str, user_id: Option<String>, kind: &str) -> EmailInput {
        let mut notification = json!({"kind":kind,"reference":"RM-1"});
        if kind == "status_changed" {
            notification["status"] = json!("ready");
        }
        EmailInput {
            id: id.into(),
            event_key: id.into(),
            user_id,
            kind: kind.into(),
            recipient: "test@example.com".into(),
            payload: json!({"version":1,"notification":notification}),
            optional_enabled: true,
        }
    }

    #[test]
    fn rejects_unversioned_private_or_unexpected_payload() {
        let mut value = input("1", None, "chat_message");
        assert!(validate(&value).is_ok());
        value.payload["notification"]["body"] = json!("private chat contents");
        assert!(validate(&value).is_err());
        value = input("1", None, "login_code");
        assert!(validate(&value).is_err());
        value = input("1", None, "order_confirmed");
        value.payload["version"] = json!(2);
        assert!(validate(&value).is_err());
    }

    #[tokio::test]
    #[ignore = "requires an isolated TEST_DATABASE_URL with migrated users table"]
    async fn durable_queue_deduplicates_recovers_leases_and_checks_preferences() {
        let db = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        sqlx::query("TRUNCATE email_outbox")
            .execute(&db.pool)
            .await
            .unwrap();
        let uid = "outbox-test-user";
        sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES ($1,'outbox@example.com','client',1000) ON CONFLICT(id) DO UPDATE SET email_order_updates=true,email_chat_messages=true,disabled_at=NULL").bind(uid).execute(&db.pool).await.unwrap();
        let mail = input("outbox-1", Some(uid.into()), "status_changed");
        let mut tx = db.pool.begin().await.unwrap();
        assert!(enqueue(&mut tx, &mail, 1000).await.unwrap());
        assert!(!enqueue(&mut tx, &mail, 1000).await.unwrap());
        tx.commit().await.unwrap();
        let first = db.claim_email(1000).await.unwrap().unwrap();
        assert!(db.claim_email(1059).await.unwrap().is_none());
        // A separate Storage instance recovers after process loss.
        let restarted = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let second = restarted.claim_email(1060).await.unwrap().unwrap();
        assert_eq!(second.attempts, 2);
        assert!(
            !db.finish_email(&first, EmailOutcome::Sent, 1061)
                .await
                .unwrap()
        );
        assert!(db.email_allowed(&second).await.unwrap());
        sqlx::query("UPDATE users SET email_order_updates=false WHERE id=$1")
            .bind(uid)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(!db.email_allowed(&second).await.unwrap());
        db.finish_email(&second, EmailOutcome::Skipped, 1061)
            .await
            .unwrap();
        let mut tx = db.pool.begin().await.unwrap();
        assert!(
            !enqueue(
                &mut tx,
                &input("outbox-2", Some(uid.into()), "status_changed"),
                1100
            )
            .await
            .unwrap()
        );
        // Mandatory receipt ignores the optional preference.
        assert!(
            enqueue(
                &mut tx,
                &input("outbox-3", Some(uid.into()), "order_confirmed"),
                1100
            )
            .await
            .unwrap()
        );
        tx.commit().await.unwrap();
        let receipt = db.claim_email(1100).await.unwrap().unwrap();
        assert!(db.email_allowed(&receipt).await.unwrap());
        db.finish_email(&receipt, EmailOutcome::Retry("transient"), 1101)
            .await
            .unwrap();
        assert!(db.claim_email(1160).await.unwrap().is_none());
        let receipt = db.claim_email(1180).await.unwrap().unwrap();
        db.finish_email(&receipt, EmailOutcome::Failed("rejected"), 1181)
            .await
            .unwrap();
        assert_eq!(db.email_stats().await.unwrap().failed, 1);
        assert!(db.claim_email(9000).await.unwrap().is_none());
        let mut tx = db.pool.begin().await.unwrap();
        enqueue(&mut tx, &input("outbox-4", None, "order_confirmed"), 9000)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        for now in [9000, 9060, 9120, 9180, 9240] {
            assert!(db.claim_email(now).await.unwrap().is_some());
        }
        assert!(db.claim_email(9300).await.unwrap().is_none());
        assert_eq!(db.email_stats().await.unwrap().failed, 2);
        assert_eq!(db.cleanup_email(30 * 86400 + 9400).await.unwrap(), 3);
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(uid)
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
