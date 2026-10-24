//! A single transaction appends chat, groups notifications and records inbound delivery.
use crate::{Storage, StorageError};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};

#[derive(Serialize, utoipa::ToSchema)]
pub struct MessagePage {
    pub messages: Vec<Value>,
    pub next_after: Option<i64>,
    pub read_sequence: i64,
    pub unread: i64,
}
pub struct ReplyRoute {
    pub id: String,
    pub order_id: String,
    pub email: String,
    pub user_id: Option<String>,
    pub operator: bool,
    pub expires_at: i64,
}
pub struct InboxLease {
    pub mailbox: String,
    pub uidvalidity: i64,
    pub last_uid: i64,
    pub lease_token: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum InboxOutcome {
    Imported,
    Duplicate,
    Rejected,
    Review,
    Ignored,
}
fn participant(user: Option<&str>, owner: Option<&str>, operator: bool) -> String {
    if operator {
        format!("admin:{}", user.unwrap_or("operator"))
    } else if let Some(user) = user {
        format!("user:{user}")
    } else {
        format!("guest:{}", owner.unwrap_or("missing"))
    }
}
fn valid_body(body: &str) -> bool {
    !body.trim().is_empty()
        && body.len() <= 4000
        && !body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
}
async fn authorized(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    user: Option<&str>,
    owner: Option<&str>,
    operator: bool,
    now: i64,
) -> Result<Option<sqlx::postgres::PgRow>, StorageError> {
    Ok(sqlx::query("SELECT user_id,contact,guest_updates FROM orders WHERE id=$1 AND ($4 OR (user_id=$2 AND EXISTS(SELECT 1 FROM users WHERE id=$2 AND disabled_at IS NULL)) OR (user_id IS NULL AND owner_hash=$3 AND created_at>$5-86400)) FOR UPDATE")
        .bind(id).bind(user).bind(owner).bind(operator).bind(now).fetch_optional(&mut **tx).await?)
}
impl Storage {
    #[allow(clippy::too_many_arguments)]
    pub async fn chat_messages(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
        after: i64,
        limit: i64,
    ) -> Result<Option<MessagePage>, StorageError> {
        if after < 0 || !(1..=100).contains(&limit) {
            return Err("invalid message cursor".into());
        }
        let mut tx = self.pool.begin().await?;
        let now: i64 = sqlx::query_scalar("SELECT extract(epoch from now())::bigint")
            .fetch_one(&mut *tx)
            .await?;
        if authorized(&mut tx, id, user, owner, operator, now)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let who = participant(user, owner, operator);
        let read_sequence:i64=sqlx::query_scalar("SELECT COALESCE((SELECT sequence FROM chat_reads WHERE order_id=$1 AND participant=$2),0)").bind(id).bind(&who).fetch_one(&mut *tx).await?;
        let unread:i64=sqlx::query_scalar("SELECT count(*) FROM order_messages WHERE order_id=$1 AND sequence>$2 AND operator!=$3").bind(id).bind(read_sequence).bind(operator).fetch_one(&mut *tx).await?;
        let mut messages:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',id,'sequence',sequence,'operator',operator,'body',body,'origin',origin,'created_at',created_at) FROM order_messages WHERE order_id=$1 AND sequence>$2 ORDER BY sequence LIMIT $3")
            .bind(id).bind(after).bind(limit+1).fetch_all(&mut *tx).await?;
        let more = messages.len() > limit as usize;
        messages.truncate(limit as usize);
        let next_after = if more {
            messages.last().and_then(|m| m["sequence"].as_i64())
        } else {
            None
        };
        tx.commit().await?;
        Ok(Some(MessagePage {
            messages,
            next_after,
            read_sequence,
            unread,
        }))
    }
    pub async fn chat_read(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
        sequence: i64,
    ) -> Result<bool, StorageError> {
        if sequence < 0 {
            return Err("invalid message cursor".into());
        }
        let mut tx = self.pool.begin().await?;
        let now: i64 = sqlx::query_scalar("SELECT extract(epoch from now())::bigint")
            .fetch_one(&mut *tx)
            .await?;
        if authorized(&mut tx, id, user, owner, operator, now)
            .await?
            .is_none()
        {
            return Ok(false);
        }
        let max: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(sequence),0) FROM order_messages WHERE order_id=$1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if sequence > max {
            return Err("message cursor exceeds history".into());
        }
        sqlx::query("INSERT INTO chat_reads(order_id,participant,sequence) VALUES($1,$2,$3) ON CONFLICT(order_id,participant) DO UPDATE SET sequence=GREATEST(chat_reads.sequence,excluded.sequence)")
            .bind(id).bind(participant(user,owner,operator)).bind(sequence).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn chat_web_message(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
        body: &str,
        admin_to: Option<&str>,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !valid_body(body) {
            return Err("invalid message".into());
        }
        let mut tx = self.queue_transaction().await?;
        let Some(row) = authorized(&mut tx, id, user, owner, operator, now).await? else {
            return Ok(false);
        };
        append(
            &mut tx, id, user, operator, body, "web", admin_to, &row, now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    /// Local operator opt-in only; this is not an HTTP authorization mechanism.
    pub async fn chat_operator_email(
        &self,
        email: &str,
        enabled: bool,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let user:Option<String>=sqlx::query_scalar("UPDATE users SET email_reply_enabled=$2 WHERE email=$1 AND role='admin' AND disabled_at IS NULL RETURNING id").bind(email.trim().to_lowercase()).bind(enabled).fetch_optional(&mut *tx).await?;
        if let Some(user) = &user
            && !enabled
        {
            sqlx::query("UPDATE chat_reply_routes SET revoked_at=$2 WHERE user_id=$1 AND operator=true AND revoked_at IS NULL").bind(user).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(user.is_some())
    }
    pub async fn chat_reply_route(
        &self,
        id: &str,
        now: i64,
    ) -> Result<Option<ReplyRoute>, StorageError> {
        let row=sqlx::query("SELECT r.id,r.order_id,r.email,r.user_id,r.operator,r.expires_at FROM chat_reply_routes r JOIN orders o ON o.id=r.order_id LEFT JOIN users u ON u.id=r.user_id WHERE r.id=$1 AND r.revoked_at IS NULL AND r.expires_at>$2 AND ((r.operator AND u.role='admin' AND u.disabled_at IS NULL AND u.email_reply_enabled AND u.email=r.email) OR (NOT r.operator AND ((o.user_id=r.user_id AND u.disabled_at IS NULL AND u.email=r.email) OR (o.user_id IS NULL AND r.user_id IS NULL AND lower(o.contact->>'email')=r.email))))")
            .bind(id).bind(now).fetch_optional(&self.pool).await?;
        row.map(|r| {
            Ok(ReplyRoute {
                id: r.try_get("id")?,
                order_id: r.try_get("order_id")?,
                email: r.try_get("email")?,
                user_id: r.try_get("user_id")?,
                operator: r.try_get("operator")?,
                expires_at: r.try_get("expires_at")?,
            })
        })
        .transpose()
    }
    /// UID checkpoint starts at the explicit caller-supplied point, never backfills on reset.
    pub async fn inbox_claim(
        &self,
        mailbox: &str,
        uidvalidity: i64,
        start_uid: i64,
        now: i64,
    ) -> Result<Option<InboxLease>, StorageError> {
        if mailbox.is_empty() || mailbox.len() > 128 || uidvalidity <= 0 || start_uid < 0 {
            return Err("invalid mailbox checkpoint".into());
        }
        let token = uuid::Uuid::new_v4().to_string();
        let row=sqlx::query("INSERT INTO chat_mailboxes(mailbox,uidvalidity,last_uid,lease_token,locked_until) VALUES($1,$2,$3,$4,$5+120) ON CONFLICT(mailbox) DO UPDATE SET uidvalidity=excluded.uidvalidity,last_uid=CASE WHEN chat_mailboxes.uidvalidity=excluded.uidvalidity THEN chat_mailboxes.last_uid ELSE excluded.last_uid END,lease_token=excluded.lease_token,locked_until=excluded.locked_until WHERE chat_mailboxes.locked_until<=$5 RETURNING last_uid")
            .bind(mailbox).bind(uidvalidity).bind(start_uid).bind(&token).bind(now).fetch_optional(&self.pool).await?;
        Ok(row.map(|r| InboxLease {
            mailbox: mailbox.into(),
            uidvalidity,
            last_uid: r.get("last_uid"),
            lease_token: token,
        }))
    }
    /// Advance only after all earlier UIDs in a bounded, ordered batch were handled.
    pub async fn inbox_advance(
        &self,
        lease: &InboxLease,
        uid: i64,
        now: i64,
    ) -> Result<bool, StorageError> {
        if uid < 0 {
            return Err("invalid mailbox checkpoint".into());
        }
        Ok(sqlx::query("UPDATE chat_mailboxes SET last_uid=GREATEST(last_uid,$4) WHERE mailbox=$1 AND uidvalidity=$2 AND lease_token=$3 AND locked_until>$5")
            .bind(&lease.mailbox).bind(lease.uidvalidity).bind(&lease.lease_token).bind(uid).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn inbox_release(&self, lease: &InboxLease) -> Result<(), StorageError> {
        sqlx::query("UPDATE chat_mailboxes SET locked_until=0,lease_token=NULL WHERE mailbox=$1 AND lease_token=$2").bind(&lease.mailbox).bind(&lease.lease_token).execute(&self.pool).await?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn inbox_receive(
        &self,
        lease: &InboxLease,
        uid: i64,
        digest: &[u8],
        route_id: Option<&str>,
        sender: &str,
        body: Option<&str>,
        reason: Option<&str>,
        admin_to: Option<&str>,
        now: i64,
    ) -> Result<InboxOutcome, StorageError> {
        if uid <= 0 || digest.len() != 32 {
            return Err("invalid inbound identity".into());
        }
        let mut tx = self.queue_transaction().await?;
        let alive:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_mailboxes WHERE mailbox=$1 AND uidvalidity=$2 AND lease_token=$3 AND locked_until>$4 FOR UPDATE)").bind(&lease.mailbox).bind(lease.uidvalidity).bind(&lease.lease_token).bind(now).fetch_one(&mut *tx).await?;
        if !alive {
            return Err("mailbox lease expired".into());
        }
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_inbound WHERE mailbox=$1 AND uidvalidity=$2 AND uid=$3)").bind(&lease.mailbox).bind(lease.uidvalidity).bind(uid).fetch_one(&mut *tx).await?;
        if exists {
            return Ok(InboxOutcome::Duplicate);
        }
        let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_inbound WHERE mailbox=$1 AND digest=$2 AND outcome='imported')").bind(&lease.mailbox).bind(digest).fetch_one(&mut *tx).await?;
        let (mut outcome, mut code) = if duplicate {
            (InboxOutcome::Duplicate, "duplicate")
        } else if reason.is_some() {
            (InboxOutcome::Review, "parser_review")
        } else if route_id.is_none() {
            (InboxOutcome::Ignored, "unrelated")
        } else {
            (InboxOutcome::Rejected, "unauthorized")
        };
        let mut message_id = None;
        if !duplicate
            && reason.is_none()
            && let Some(route_id) = route_id
        {
            // Lock current account, route and order so ownership/role revocation cannot race import.
            let r=sqlx::query("SELECT r.order_id,r.user_id,r.operator,r.email FROM chat_reply_routes r JOIN orders o ON o.id=r.order_id WHERE r.id=$1 AND r.revoked_at IS NULL AND r.expires_at>$2 FOR UPDATE OF r,o").bind(route_id).bind(now).fetch_optional(&mut *tx).await?;
            if let Some(r) = r {
                let order: String = r.try_get("order_id")?;
                let user: Option<String> = r.try_get("user_id")?;
                let operator: bool = r.try_get("operator")?;
                let email: String = r.try_get("email")?;
                let account = if let Some(u) = &user {
                    sqlx::query("SELECT email,role,disabled_at,email_reply_enabled FROM users WHERE id=$1 FOR SHARE").bind(u).fetch_optional(&mut *tx).await?
                } else {
                    None
                };
                let account_ok = if operator {
                    account.as_ref().is_some_and(|u| {
                        u.get::<String, _>("email") == email
                            && u.get::<String, _>("role") == "admin"
                            && u.get::<Option<i64>, _>("disabled_at").is_none()
                            && u.get::<bool, _>("email_reply_enabled")
                    })
                } else {
                    account.as_ref().is_none_or(|u| {
                        u.get::<String, _>("email") == email
                            && u.get::<Option<i64>, _>("disabled_at").is_none()
                    })
                };
                let order_row =
                    sqlx::query("SELECT user_id,contact,guest_updates FROM orders WHERE id=$1")
                        .bind(&order)
                        .fetch_one(&mut *tx)
                        .await?;
                let order_user: Option<String> = order_row.try_get("user_id")?;
                let contact: Value = order_row.try_get("contact")?;
                let owner_ok = operator
                    || (order_user == user
                        && (user.is_some()
                            || contact["email"]
                                .as_str()
                                .is_some_and(|e| e.eq_ignore_ascii_case(&email))));
                if sender.trim().eq_ignore_ascii_case(&email) && account_ok && owner_ok {
                    if let Some(body) = body.filter(|b| valid_body(b)) {
                        let recent:i64=sqlx::query_scalar("SELECT count(*) FROM order_messages WHERE order_id=$1 AND created_at>$2").bind(&order).bind(now-60).fetch_one(&mut *tx).await?;
                        if recent >= 10 {
                            outcome = InboxOutcome::Review;
                            code = "message_limit";
                        } else {
                            message_id = Some(
                                append(
                                    &mut tx,
                                    &order,
                                    user.as_deref(),
                                    operator,
                                    body,
                                    "email",
                                    admin_to,
                                    &order_row,
                                    now,
                                )
                                .await?,
                            );
                            outcome = InboxOutcome::Imported;
                            code = "imported";
                        }
                    } else {
                        outcome = InboxOutcome::Review;
                        code = "invalid_body";
                    }
                }
            }
        }
        let state = match outcome {
            InboxOutcome::Imported => "imported",
            InboxOutcome::Duplicate => "duplicate",
            InboxOutcome::Rejected => "rejected",
            InboxOutcome::Review => "review",
            InboxOutcome::Ignored => "ignored",
        };
        sqlx::query("INSERT INTO chat_inbound(mailbox,uidvalidity,uid,digest,outcome,reason,message_id,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(&lease.mailbox).bind(lease.uidvalidity).bind(uid).bind(digest).bind(state).bind(code).bind(message_id).bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE chat_mailboxes SET last_uid=GREATEST(last_uid,$2) WHERE mailbox=$1")
            .bind(&lease.mailbox)
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        // Metadata only, bounded retention; imported messages remain in order history.
        sqlx::query("DELETE FROM chat_inbound WHERE (mailbox,uidvalidity,uid) IN (SELECT mailbox,uidvalidity,uid FROM chat_inbound WHERE created_at<$1-2592000 LIMIT 100)").bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}

#[allow(clippy::too_many_arguments)]
async fn append(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    user: Option<&str>,
    operator: bool,
    body: &str,
    origin: &str,
    admin_to: Option<&str>,
    row: &sqlx::postgres::PgRow,
    now: i64,
) -> Result<String, StorageError> {
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM order_messages WHERE order_id=$1 AND created_at>$2",
    )
    .bind(id)
    .bind(now - 60)
    .fetch_one(&mut **tx)
    .await?;
    if recent >= 10 {
        return Err("message limit".into());
    }
    let message = uuid::Uuid::new_v4().to_string();
    let sequence:i64=sqlx::query_scalar("INSERT INTO order_messages(id,order_id,author_id,operator,body,origin,created_at) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING sequence")
        .bind(&message).bind(id).bind(user).bind(operator).bind(body.trim()).bind(origin).bind(now).fetch_one(&mut **tx).await?;
    let order_user: Option<String> = row.try_get("user_id")?;
    let contact: Value = row.try_get("contact")?;
    // Account chat uses its verified email, not an independently supplied delivery contact.
    let client_email: Option<String> = if let Some(user) = &order_user {
        sqlx::query_scalar("SELECT email FROM users WHERE id=$1 AND disabled_at IS NULL")
            .bind(user)
            .fetch_optional(&mut **tx)
            .await?
    } else {
        contact["email"].as_str().map(str::to_owned)
    };
    let to = if operator {
        client_email.as_deref()
    } else {
        admin_to
    };
    if let Some(to) = to {
        let to = to.trim().to_lowercase();
        if to.len() > 254 || !to.contains('@') || to.chars().any(char::is_control) {
            return Err("invalid chat recipient".into());
        }
        let recipient_user: Option<String> = if operator {
            order_user
        } else {
            sqlx::query_scalar("SELECT id FROM users WHERE email=$1")
                .bind(&to)
                .fetch_optional(&mut **tx)
                .await?
        };
        let enabled: bool = if let Some(u) = &recipient_user {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND email=$2 AND disabled_at IS NULL AND email_chat_messages AND ($3 OR role='admin'))").bind(u).bind(&to).bind(operator).fetch_one(&mut **tx).await?
        } else if operator {
            row.try_get("guest_updates")?
        } else {
            true
        };
        // Distinct participant routes remain meaningful even when localhost testing uses one mailbox.
        // Only the opposite participant receives mail; matching test addresses are not an identity.
        if enabled {
            let old=sqlx::query("SELECT id,chat_first,chat_last,reply_route,payload FROM email_outbox WHERE chat_order=$1 AND chat_operator=$2 AND recipient=$3 AND status='pending' AND attempts=0 AND NOT chat_frozen AND available_at>$4 ORDER BY created_at LIMIT 1 FOR UPDATE")
                .bind(id).bind(!operator).bind(&to).bind(now).fetch_optional(&mut **tx).await?;
            if let Some(old) = old {
                let mut payload: Value = old.try_get("payload")?;
                let text = payload["notification"]["body"].as_str().unwrap_or("");
                let combined = format!("{text}\n\n{}", body.trim());
                payload["notification"]["body"] = json!(combined);
                payload["notification"]["last_sequence"] = json!(sequence);
                if payload["notification"]["body"]
                    .as_str()
                    .is_some_and(|b| b.len() <= 6000)
                    && payload.to_string().len() <= 16384
                {
                    sqlx::query("UPDATE email_outbox SET payload=$2,chat_last=$3 WHERE id=$1")
                        .bind(old.get::<String, _>("id"))
                        .bind(payload)
                        .bind(sequence)
                        .execute(&mut **tx)
                        .await?;
                    return Ok(message);
                }
            }
            let route = uuid::Uuid::new_v4().to_string();
            let job = uuid::Uuid::new_v4().to_string();
            sqlx::query("INSERT INTO chat_reply_routes(id,order_id,user_id,email,operator,expires_at) VALUES($1,$2,$3,$4,$5,$6)").bind(&route).bind(id).bind(&recipient_user).bind(&to).bind(!operator).bind(now+30*86400).execute(&mut **tx).await?;
            let payload = json!({"version":1,"notification":{"kind":"chat_message","reference":id,"body":body.trim(),"operator":!operator,"reply_route":route,"first_sequence":sequence,"last_sequence":sequence}});
            crate::email::enqueue(
                tx,
                &crate::email::EmailInput {
                    id: job.clone(),
                    event_key: format!("order:{id}:message:{message}"),
                    user_id: recipient_user,
                    kind: "chat_message".into(),
                    recipient: to,
                    payload,
                    optional_enabled: enabled,
                },
                now,
            )
            .await?;
            sqlx::query("UPDATE email_outbox SET chat_order=$2,chat_operator=$3,chat_first=$4,chat_last=$4,reply_route=$5,available_at=$6+60 WHERE id=$1")
                .bind(job).bind(id).bind(!operator).bind(sequence).bind(route).bind(now).execute(&mut **tx).await?;
        }
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn message_boundaries_are_shared_by_web_and_email() {
        assert!(valid_body("Hello\nWorld\t!"));
        assert!(!valid_body(" \n\t"));
        assert!(!valid_body("hidden\0body"));
        assert!(!valid_body(&"a".repeat(4001)));
        assert_eq!(participant(Some("a"), None, true), "admin:a");
        assert_eq!(participant(Some("a"), None, false), "user:a");
    }
    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn chat_is_owned_paginated_grouped_and_inbound_atomic() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("rapidmaker_test")
        );
        let db = Storage::connect(&url).await.unwrap();
        let now: i64 = sqlx::query_scalar("SELECT extract(epoch from now())::bigint")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        let order = uuid::Uuid::new_v4().to_string();
        let customer = uuid::Uuid::new_v4().to_string();
        let admin = uuid::Uuid::new_v4().to_string();
        let client_email = format!("{customer}@example.invalid");
        let admin_email = format!("{admin}@example.invalid");
        for (id, email, role) in [
            (&customer, &client_email, "client"),
            (&admin, &admin_email, "admin"),
        ] {
            sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,$3,$4)")
                .bind(id)
                .bind(email)
                .bind(role)
                .bind(now)
                .execute(&db.pool)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO orders(id,owner_hash,user_id,contact,lines,amount_grosze,provider_kind,guest_updates,created_at) VALUES($1,$2,$3,$4,'[]',100,'intent',true,$5)")
            .bind(&order).bind("ab".repeat(32)).bind(&customer).bind(json!({"email":client_email})).bind(now).execute(&db.pool).await.unwrap();
        assert!(
            !db.order_message(
                &order,
                Some("other"),
                None,
                false,
                "forged",
                Some(&admin_email),
                now
            )
            .await
            .unwrap()
        );
        for text in ["one", "two"] {
            assert!(
                db.order_message(
                    &order,
                    Some(&customer),
                    None,
                    false,
                    text,
                    Some(&admin_email),
                    now
                )
                .await
                .unwrap()
            );
        }
        let jobs: Vec<sqlx::postgres::PgRow> = sqlx::query(
            "SELECT id,payload,available_at,reply_route FROM email_outbox WHERE chat_order=$1",
        )
        .bind(&order)
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].get::<i64, _>("available_at"), now + 60);
        let payload: Value = jobs[0].get("payload");
        assert_eq!(payload["notification"]["body"], "one\n\ntwo");
        assert_eq!(payload["notification"]["operator"], true);
        let route: String = jobs[0].get("reply_route");
        assert!(db.chat_reply_route(&route, now).await.unwrap().is_none());
        assert!(
            db.chat_operator_email(&admin_email, true, now)
                .await
                .unwrap()
        );
        assert!(db.chat_reply_route(&route, now).await.unwrap().is_some());
        let queued = db.claim_email(now + 60).await.unwrap().unwrap();
        assert!(db.email_allowed(&queued).await.unwrap());
        assert!(
            db.finish_email(
                &queued,
                crate::EmailOutcome::Deferred("test_defer"),
                now + 60
            )
            .await
            .unwrap()
        );
        let frozen: bool = sqlx::query_scalar("SELECT chat_frozen FROM email_outbox WHERE id=$1")
            .bind(&queued.id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(
            frozen,
            "even a deferred first claim must freeze its message range"
        );
        let lease = db
            .inbox_claim("test-chat", 123, 100, now)
            .await
            .unwrap()
            .unwrap();
        assert!(
            db.inbox_claim("test-chat", 123, 100, now)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            db.inbox_receive(
                &lease,
                101,
                &[1; 32],
                Some(&route),
                "wrong@example.invalid",
                Some("injected"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Rejected
        );
        assert_eq!(
            db.inbox_receive(
                &lease,
                102,
                &[2; 32],
                Some(&route),
                &admin_email,
                Some("email response"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Imported
        );
        assert_eq!(
            db.inbox_receive(
                &lease,
                103,
                &[2; 32],
                Some(&route),
                &admin_email,
                Some("email response"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Duplicate
        );
        assert!(
            db.chat_operator_email(&admin_email, false, now)
                .await
                .unwrap()
        );
        assert_eq!(
            db.inbox_receive(
                &lease,
                104,
                &[3; 32],
                Some(&route),
                &admin_email,
                Some("revoked"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Rejected
        );
        let customer_route: String = sqlx::query_scalar(
            "SELECT reply_route FROM email_outbox WHERE chat_order=$1 AND chat_operator=false",
        )
        .bind(&order)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        sqlx::query("UPDATE users SET disabled_at=$2 WHERE id=$1")
            .bind(&customer)
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.inbox_receive(
                &lease,
                105,
                &[4; 32],
                Some(&customer_route),
                &client_email,
                Some("disabled"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Rejected
        );
        sqlx::query("UPDATE users SET disabled_at=NULL WHERE id=$1")
            .bind(&customer)
            .execute(&db.pool)
            .await
            .unwrap();
        // Force a new batch and reject its outbox snapshot; neither receipt nor message may commit.
        sqlx::query("UPDATE email_outbox SET chat_frozen=true WHERE chat_order=$1")
            .bind(&order)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE email_outbox ADD CONSTRAINT chat_test_rollback CHECK(payload->'notification'->>'body' IS DISTINCT FROM 'rollback')").execute(&db.pool).await.unwrap();
        assert!(
            db.inbox_receive(
                &lease,
                106,
                &[5; 32],
                Some(&customer_route),
                &client_email,
                Some("rollback"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .is_err()
        );
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM chat_inbound WHERE mailbox='test-chat' AND uid=106",
        )
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(count, 0);
        sqlx::query("ALTER TABLE email_outbox DROP CONSTRAINT chat_test_rollback")
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.inbox_receive(
                &lease,
                106,
                &[5; 32],
                Some(&customer_route),
                &client_email,
                Some("rollback"),
                None,
                Some(&admin_email),
                now
            )
            .await
            .unwrap(),
            InboxOutcome::Imported
        );
        // Seed historical entries without evading current write limits in application code.
        for i in 0..201 {
            sqlx::query("INSERT INTO order_messages(id,order_id,operator,body,created_at) VALUES($1,$2,true,$3,$4)").bind(uuid::Uuid::new_v4().to_string()).bind(&order).bind(format!("history {i}")).bind(now-600).execute(&db.pool).await.unwrap();
        }
        let mut after = 0;
        let mut found = 0;
        let max = loop {
            let page = db
                .chat_messages(&order, Some(&customer), None, false, after, 50)
                .await
                .unwrap()
                .unwrap();
            found += page.messages.len();
            let max = page.messages.last().unwrap()["sequence"].as_i64().unwrap();
            if let Some(next) = page.next_after {
                after = next;
            } else {
                break max;
            }
        };
        assert_eq!(found, 205);
        assert!(
            db.chat_read(&order, Some(&customer), None, false, max)
                .await
                .unwrap()
        );
        assert!(
            db.chat_read(&order, Some(&customer), None, false, 1)
                .await
                .unwrap()
        );
        let page = db
            .chat_messages(&order, Some(&customer), None, false, 0, 50)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(page.unread, 0);
        assert_eq!(page.read_sequence, max);
        assert!(db.inbox_advance(&lease, 110, now).await.unwrap());
        db.inbox_release(&lease).await.unwrap();
        let restart = db
            .inbox_claim("test-chat", 123, 999, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restart.last_uid, 110);
    }
}
