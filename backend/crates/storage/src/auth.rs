//! PostgreSQL login codes, persistent sessions and notification preferences.
use crate::{Storage, StorageError};
use rapidmaker_auth::{AuthKey, constant_eq, hash_token, valid_token};
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

#[derive(Debug, Serialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub role: String,
    pub email_order_updates: bool,
    pub email_chat_messages: bool,
}

#[derive(Debug, Serialize)]
pub struct AuthSession {
    pub user: AuthUser,
    pub expires_at: i64,
    pub admin_verified_at: Option<i64>,
    pub stage: String,
    pub second_factor: String,
    pub factor_verified_at: Option<i64>,
}

impl Storage {
    #[allow(clippy::too_many_arguments)]
    pub async fn auth_start(
        &self,
        id: &str,
        email: &str,
        browser_hash: &[u8],
        verifier: &[u8],
        ip_hash: &[u8],
        now: i64,
        legal_version: &str,
    ) -> Result<bool, StorageError> {
        if legal_version.is_empty()
            || legal_version.len() > 32
            || legal_version.chars().any(char::is_control)
        {
            return Err("invalid legal version".into());
        }
        if !email.is_ascii()
            || email.len() > 254
            || email.len() < 3
            || email != email.trim().to_lowercase()
            || !email.contains('@')
            || browser_hash.len() != 32
            || verifier.len() != 32
            || ip_hash.len() != 32
        {
            return Err("invalid auth challenge".into());
        }
        let mut tx = self.pool.begin().await?;
        // Serializes resend checks and admission across processes, before inserting a challenge.
        sqlx::query("SELECT pg_advisory_xact_lock(1380794709)")
            .execute(&mut *tx)
            .await?;
        cleanup(&mut tx, now).await?;
        let allowed = limits(&mut tx, "send", email, ip_hash, now).await?;
        let recent: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM auth_challenges WHERE email=$1 AND created_at>$2)",
        )
        .bind(email)
        .bind(now - 60)
        .fetch_one(&mut *tx)
        .await?;
        let capacity: bool = sqlx::query_scalar("SELECT count(*)<10000 FROM auth_challenges")
            .fetch_one(&mut *tx)
            .await?;
        if !allowed || recent || !capacity {
            tx.commit().await?;
            return Ok(false);
        }
        sqlx::query("UPDATE auth_challenges SET consumed_at=$3 WHERE email=$1 AND browser_hash=$2 AND consumed_at IS NULL")
            .bind(email).bind(browser_hash).bind(now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO auth_challenges(id,email,browser_hash,verifier,created_at,expires_at,legal_version) VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(id).bind(email).bind(browser_hash).bind(verifier).bind(now).bind(now+600).bind(legal_version)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn auth_invalidate(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE auth_challenges SET consumed_at=expires_at WHERE id=$1 AND consumed_at IS NULL",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn auth_verify(
        &self,
        key: &AuthKey,
        id: &str,
        browser_token: &str,
        code: &str,
        session_token: &str,
        ip_hash: &[u8],
        previous_session: Option<&str>,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if !valid_token(browser_token) || !valid_token(session_token) || ip_hash.len() != 32 {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        // One short DB transaction serializes rate admission and bounded cleanup across processes.
        sqlx::query("SELECT pg_advisory_xact_lock(1380794709)")
            .execute(&mut *tx)
            .await?;
        cleanup(&mut tx, now).await?;
        if !limit(&mut tx, "verify-global", b"all", 1000, now, 900).await?
            || !limit(&mut tx, "verify-ip", ip_hash, 60, now, 900).await?
        {
            tx.commit().await?;
            return Ok(None);
        }
        let row = sqlx::query("SELECT email,browser_hash,verifier,attempts,expires_at,consumed_at,legal_version FROM auth_challenges WHERE id=$1 FOR UPDATE")
            .bind(id).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let Some(legal_version): Option<String> = row.try_get("legal_version")? else {
            return Ok(None);
        };
        let email: String = row.try_get("email")?;
        let failure_scope = scope_hash("verify-email", email.as_bytes());
        let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM auth_limits WHERE scope_hash=$1 AND window_start>$2 AND attempts>=20)")
            .bind(failure_scope.as_slice()).bind(now-900).fetch_one(&mut *tx).await?;
        let browser_hash: Vec<u8> = row.try_get("browser_hash")?;
        let verifier: Vec<u8> = row.try_get("verifier")?;
        let attempts: i16 = row.try_get("attempts")?;
        let expires_at: i64 = row.try_get("expires_at")?;
        let consumed: Option<i64> = row.try_get("consumed_at")?;
        let bound = constant_eq(&browser_hash, &hash_token(browser_token));
        if blocked || !bound || attempts >= 5 || expires_at <= now || consumed.is_some() {
            tx.commit().await?;
            return Ok(None);
        }
        let valid = code.len() == 8
            && code.bytes().all(|b| b.is_ascii_digit())
            && constant_eq(&verifier, &key.verifier(id, &email, &browser_hash, code));
        if !valid {
            limit(&mut tx, "verify-email", email.as_bytes(), 20, now, 900).await?;
            sqlx::query("UPDATE auth_challenges SET attempts=attempts+1 WHERE id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(None);
        }
        let capacity: bool = sqlx::query_scalar("SELECT count(*)<10000 FROM sessions")
            .fetch_one(&mut *tx)
            .await?;
        if !capacity {
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query("UPDATE auth_challenges SET consumed_at=$2 WHERE id=$1")
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO users(id,email,created_at) VALUES($1,$2,$3) ON CONFLICT(email) DO NOTHING",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&email)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query("SELECT id,email,role,email_order_updates,email_chat_messages,legal_version FROM users WHERE email=$1 AND disabled_at IS NULL FOR UPDATE")
            .bind(email).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let user = user(&row)?;
        let credentialed: bool = sqlx::query_scalar("SELECT password_hash IS NOT NULL OR totp_secret IS NOT NULL OR EXISTS(SELECT 1 FROM passkeys WHERE user_id=users.id) FROM users WHERE id=$1")
            .bind(&user.id).fetch_one(&mut *tx).await?;
        if credentialed {
            tx.commit().await?;
            return Ok(None);
        }
        if row.try_get::<Option<String>, _>("legal_version")?.is_none() {
            sqlx::query("UPDATE users SET legal_version=$2,legal_accepted_at=$3 WHERE id=$1")
                .bind(&user.id)
                .bind(&legal_version)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            crate::orders::enqueue(&mut tx, &user.id, "account-terms", Some(&user.id), &user.email,
                "legal_documents", serde_json::json!({"reference":user.id,"legal_version":legal_version,"details":"Utworzono bezpłatne konto RapidMaker. Poniżej warunki zaakceptowane przy rejestracji i informacja o prywatności."}), true, now).await?;
        }
        if let Some(previous) = previous_session {
            sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
                .bind(hash_token(previous).as_slice())
                .execute(&mut *tx)
                .await?;
        }
        // A bounded number of active sessions prevents unbounded state for frequent logins.
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE user_id=$1 ORDER BY created_at DESC OFFSET 19)")
            .bind(&user.id).execute(&mut *tx).await?;
        let expires_at = now + 600;
        sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at) VALUES($1,$2,$3,$3,$4)")
            .bind(hash_token(session_token).as_slice()).bind(&user.id).bind(now).bind(expires_at)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(AuthSession {
            user,
            expires_at,
            admin_verified_at: None,
            stage: "setup".into(),
            second_factor: "email".into(),
            factor_verified_at: None,
        }))
    }

    pub async fn auth_session(
        &self,
        token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        Ok(self
            .auth_pending(token, now)
            .await?
            .filter(|s| s.stage == "authenticated"))
    }

    pub async fn auth_pending(
        &self,
        token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if !valid_token(token) {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT u.id,u.email,u.role,u.email_order_updates,u.email_chat_messages,u.totp_secret IS NOT NULL AS has_totp,s.expires_at,s.admin_verified_at,s.stage,s.factor_verified_at FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>$2 AND s.last_seen_at>$2-CASE WHEN u.role='admin' THEN 1800 ELSE 604800 END AND u.disabled_at IS NULL")
            .bind(hash_token(token).as_slice()).bind(now)
            .fetch_optional(&mut *tx).await?;
        let result = row
            .map(|row| {
                Ok::<_, StorageError>(AuthSession {
                    user: user(&row)?,
                    expires_at: row.try_get("expires_at")?,
                    admin_verified_at: row.try_get("admin_verified_at")?,
                    stage: row.try_get("stage")?,
                    second_factor: if row.try_get::<bool, _>("has_totp")? {
                        "totp"
                    } else {
                        "email"
                    }
                    .into(),
                    factor_verified_at: row.try_get("factor_verified_at")?,
                })
            })
            .transpose()?;
        if result.is_some() {
            sqlx::query(
                "UPDATE sessions SET last_seen_at=$2 WHERE token_hash=$1 AND last_seen_at<=$3",
            )
            .bind(hash_token(token).as_slice())
            .bind(now)
            .bind(now - 300)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(result)
    }

    pub async fn auth_logout(&self, token: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(hash_token(token).as_slice())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn auth_logout_all(&self, user_id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn auth_preferences(
        &self,
        user_id: &str,
        orders: bool,
        chat: bool,
    ) -> Result<Option<AuthUser>, StorageError> {
        let row = sqlx::query("UPDATE users SET email_order_updates=$2,email_chat_messages=$3 WHERE id=$1 AND disabled_at IS NULL RETURNING id,email,role,email_order_updates,email_chat_messages")
            .bind(user_id).bind(orders).bind(chat).fetch_optional(&self.pool).await?;
        row.as_ref().map(user).transpose()
    }
}

pub(crate) fn user(row: &PgRow) -> Result<AuthUser, StorageError> {
    Ok(AuthUser {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        role: row.try_get("role")?,
        email_order_updates: row.try_get("email_order_updates")?,
        email_chat_messages: row.try_get("email_chat_messages")?,
    })
}

async fn limits(
    tx: &mut Transaction<'_, Postgres>,
    prefix: &str,
    email: &str,
    ip: &[u8],
    now: i64,
) -> Result<bool, StorageError> {
    if !limit(tx, &format!("{prefix}-global"), b"all", 100, now, 900).await?
        || !limit(tx, &format!("{prefix}-ip"), ip, 20, now, 900).await?
    {
        return Ok(false);
    }
    limit(
        tx,
        &format!("{prefix}-email"),
        email.as_bytes(),
        3,
        now,
        900,
    )
    .await
}

pub(crate) async fn limit(
    tx: &mut Transaction<'_, Postgres>,
    scope: &str,
    value: &[u8],
    maximum: i64,
    now: i64,
    seconds: i64,
) -> Result<bool, StorageError> {
    let count:Option<i64> = sqlx::query_scalar("INSERT INTO auth_limits(scope_hash,window_start,attempts) SELECT $1,$2,1 WHERE EXISTS(SELECT 1 FROM auth_limits WHERE scope_hash=$1) OR (SELECT count(*) FROM auth_limits)<10000 ON CONFLICT(scope_hash) DO UPDATE SET window_start=CASE WHEN auth_limits.window_start<=$3 THEN $2 ELSE auth_limits.window_start END,attempts=CASE WHEN auth_limits.window_start<=$3 THEN 1 ELSE LEAST(auth_limits.attempts+1,1000000) END RETURNING attempts")
        .bind(scope_hash(scope,value).as_slice()).bind(now).bind(now-seconds).fetch_optional(&mut **tx).await?;
    Ok(count.is_some_and(|count| count <= maximum))
}

fn scope_hash(scope: &str, value: &[u8]) -> [u8; 32] {
    hash_token(&format!(
        "{scope}:{}",
        value.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}

async fn cleanup(tx: &mut Transaction<'_, Postgres>, now: i64) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM auth_challenges WHERE id IN (SELECT id FROM auth_challenges WHERE expires_at<$1 LIMIT 100)")
        .bind(now-86400).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE expires_at<$1 OR last_seen_at<$2 LIMIT 100)")
        .bind(now).bind(now-7*86400).execute(&mut **tx).await?;
    sqlx::query("DELETE FROM auth_limits WHERE scope_hash IN (SELECT scope_hash FROM auth_limits WHERE window_start<$1 LIMIT 100)")
        .bind(now-86400).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapidmaker_auth::random_token;

    #[tokio::test]
    #[ignore = "requires dedicated AUTH_TEST_DATABASE_URL PostgreSQL database"]
    async fn codes_are_bound_limited_and_consumed_atomically() {
        let url = std::env::var("AUTH_TEST_DATABASE_URL").expect("dedicated test database");
        let db = Storage::connect(&url).await.unwrap();
        let key = AuthKey::from_hex(&"ab".repeat(32)).unwrap();
        let browser = random_token().unwrap();
        let other_browser = random_token().unwrap();
        let ip = hash_token(&random_token().unwrap());
        let id = uuid::Uuid::new_v4().to_string();
        let email = format!("{}@example.invalid", uuid::Uuid::new_v4());
        let now = 1_800_000_000;
        let verifier = key.verifier(&id, &email, &hash_token(&browser), "00000001");
        assert!(
            db.auth_start(
                &id,
                &email,
                &hash_token(&browser),
                &verifier,
                &ip,
                now,
                "2026-10-25"
            )
            .await
            .unwrap()
        );
        assert!(
            !db.auth_start(
                "resend",
                &email,
                &hash_token(&browser),
                &verifier,
                &ip,
                now + 1,
                "2026-10-25"
            )
            .await
            .unwrap()
        );
        let token = random_token().unwrap();
        assert!(
            db.auth_verify(
                &key,
                &id,
                &other_browser,
                "00000001",
                &token,
                &ip,
                None,
                now + 1
            )
            .await
            .unwrap()
            .is_none()
        );
        for _ in 0..5 {
            assert!(
                db.auth_verify(&key, &id, &browser, "bad", &token, &ip, None, now + 1)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            db.auth_verify(&key, &id, &browser, "00000001", &token, &ip, None, now + 1)
                .await
                .unwrap()
                .is_none()
        );
        let attempts: i16 = sqlx::query_scalar("SELECT attempts FROM auth_challenges WHERE id=$1")
            .bind(&id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(attempts, 5);
        let second = uuid::Uuid::new_v4().to_string();
        let verifier = key.verifier(&second, &email, &hash_token(&browser), "00000002");
        assert!(
            db.auth_start(
                &second,
                &email,
                &hash_token(&browser),
                &verifier,
                &ip,
                now + 61,
                "2026-10-25"
            )
            .await
            .unwrap()
        );
        let a = random_token().unwrap();
        let b = random_token().unwrap();
        let (a_result, b_result) = tokio::join!(
            db.auth_verify(&key, &second, &browser, "00000002", &a, &ip, None, now + 62),
            db.auth_verify(&key, &second, &browser, "00000002", &b, &ip, None, now + 62)
        );
        assert_eq!(
            usize::from(a_result.unwrap().is_some()) + usize::from(b_result.unwrap().is_some()),
            1
        );
        let session = if db.auth_pending(&a, now + 63).await.unwrap().is_some() {
            a
        } else {
            b
        };
        let user = db
            .auth_pending(&session, now + 64)
            .await
            .unwrap()
            .unwrap()
            .user;
        let acceptance: (String, i64) =
            sqlx::query_as("SELECT legal_version,legal_accepted_at FROM users WHERE id=$1")
                .bind(&user.id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(acceptance, ("2026-10-25".into(), now + 62));
        let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM email_outbox WHERE user_id=$1 AND kind='legal_documents' AND payload->'notification'->>'legal_version'='2026-10-25'")
            .bind(&user.id).fetch_one(&db.pool).await.unwrap();
        assert_eq!(copies, 1); // concurrent verification must not duplicate the agreement or its copy
        let preferences = db
            .auth_preferences(&user.id, false, true)
            .await
            .unwrap()
            .unwrap();
        assert!(!preferences.email_order_updates);
        assert!(preferences.email_chat_messages);
        // The persisted session is valid through a fresh pool and has a fixed idle deadline.
        let reconnected = Storage::connect(&url).await.unwrap();
        assert!(
            reconnected
                .auth_pending(&session, now + 65)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            reconnected
                .auth_pending(&session, now + 65 + 7 * 86400)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
            .bind(&user.id)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db.auth_pending(&session, now + 66).await.unwrap().is_none());
        db.auth_logout_all(&user.id).await.unwrap();
        assert!(db.auth_pending(&session, now + 66).await.unwrap().is_none());
        sqlx::query("DELETE FROM email_outbox WHERE user_id=$1")
            .bind(&user.id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(&user.id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM auth_challenges WHERE email=$1")
            .bind(&email)
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
