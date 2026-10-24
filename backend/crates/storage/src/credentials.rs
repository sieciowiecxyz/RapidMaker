//! Two-factor state transitions are serialized with credentials and revoke stale sessions.
use crate::{
    AuthSession, Storage, StorageError,
    auth::{limit, user},
};
use rapidmaker_auth::{
    AuthKey, constant_eq,
    credentials::{open_totp, verify_totp},
    hash_token, valid_token,
};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

pub(crate) async fn lock(tx: &mut Transaction<'_, Postgres>) -> Result<(), StorageError> {
    sqlx::query("SELECT pg_advisory_xact_lock(1380794709)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub(crate) fn fresh(row: &PgRow, now: i64) -> Result<bool, StorageError> {
    Ok(row.try_get::<String, _>("stage")? == "authenticated"
        && row
            .try_get::<Option<i64>, _>("primary_verified_at")?
            .is_some_and(|t| t <= now && t > now - 300)
        && row
            .try_get::<Option<i64>, _>("factor_verified_at")?
            .is_some_and(|t| t <= now && t > now - 300))
}
pub(crate) async fn credentialless(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<bool, StorageError> {
    Ok(sqlx::query_scalar("SELECT password_hash IS NULL AND totp_secret IS NULL AND NOT EXISTS(SELECT 1 FROM passkeys WHERE user_id=users.id) FROM users WHERE id=$1").bind(id).fetch_one(&mut **tx).await?)
}
pub(crate) async fn row(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    now: i64,
) -> Result<Option<PgRow>, StorageError> {
    if !valid_token(token) {
        return Ok(None);
    }
    Ok(sqlx::query("SELECT u.*,s.stage,s.created_at AS session_created_at,s.primary_verified_at,s.factor_verified_at,s.factor_verifier,s.factor_sent_at,s.factor_expires_at,s.factor_attempts FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.expires_at>$2 AND s.last_seen_at>$2-CASE WHEN u.role='admin' THEN 1800 ELSE 604800 END AND u.disabled_at IS NULL FOR UPDATE OF u,s").bind(hash_token(token).as_slice()).bind(now).fetch_optional(&mut **tx).await?)
}
pub(crate) async fn rotate(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    id: &str,
    new_token: &str,
    stage: &str,
    now: i64,
) -> Result<AuthSession, StorageError> {
    if !valid_token(new_token) || !matches!(stage, "primary" | "second_factor" | "authenticated") {
        return Err("invalid session rotation".into());
    }
    // Credential edits rotate the cookie but never extend the fresh-MFA window.
    let previous = sqlx::query("SELECT stage,created_at,primary_verified_at,factor_verified_at FROM sessions WHERE token_hash=$1 AND user_id=$2")
        .bind(hash_token(token).as_slice()).bind(id).fetch_optional(&mut **tx).await?;
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(hash_token(token).as_slice())
        .execute(&mut **tx)
        .await?;
    let row=sqlx::query("SELECT id,email,role,email_order_updates,email_chat_messages,totp_secret IS NOT NULL AS has_totp FROM users WHERE id=$1 AND disabled_at IS NULL").bind(id).fetch_one(&mut **tx).await?;
    let user = user(&row)?;
    let full = stage == "authenticated";
    let expires_at = now
        + if !full {
            600
        } else if user.role == "admin" {
            8 * 3600
        } else {
            30 * 86400
        };
    let mut primary = if stage == "primary" { None } else { Some(now) };
    let mut factor = full.then_some(now);
    if full && let Some(old) = previous {
        let old_stage: String = old.try_get("stage")?;
        if old_stage == "setup" {
            factor = Some(old.try_get("created_at")?);
        } else {
            primary = old.try_get("primary_verified_at")?;
            if old_stage == "authenticated" {
                factor = old.try_get("factor_verified_at")?;
            }
        }
    }
    let admin = if full && user.role == "admin" {
        factor
    } else {
        None
    };
    sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at,admin_verified_at,stage,primary_verified_at,factor_verified_at) VALUES($1,$2,$3,$3,$4,$5,$6,$7,$8)")
        .bind(hash_token(new_token).as_slice()).bind(id).bind(now).bind(expires_at).bind(admin).bind(stage).bind(primary).bind(factor).execute(&mut **tx).await?;
    if full {
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE user_id=$1 AND stage='authenticated' AND token_hash<>$2 ORDER BY created_at DESC OFFSET 19)")
            .bind(id).bind(hash_token(new_token).as_slice()).execute(&mut **tx).await?;
    }
    Ok(AuthSession {
        user,
        expires_at,
        admin_verified_at: admin,
        stage: stage.into(),
        second_factor: if row.try_get::<bool, _>("has_totp")? {
            "totp"
        } else {
            "email"
        }
        .into(),
        factor_verified_at: factor,
    })
}
pub(crate) async fn revoke_others(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    token: &str,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND token_hash<>$2")
        .bind(id)
        .bind(hash_token(token).as_slice())
        .execute(&mut **tx)
        .await?;
    sqlx::query("DELETE FROM passkey_challenges WHERE user_id=$1")
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

impl Storage {
    pub async fn auth_enrollment_allowed(&self, email: &str) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM users u WHERE u.email=$1 AND (u.disabled_at IS NOT NULL OR u.password_hash IS NOT NULL OR u.totp_secret IS NOT NULL OR EXISTS(SELECT 1 FROM passkeys WHERE user_id=u.id)))").bind(email).fetch_one(&self.pool).await?)
    }
    /// Rate admission precedes expensive password verification or WebAuthn challenges.
    pub async fn auth_login_record(
        &self,
        email: &str,
        ip: &[u8],
        now: i64,
    ) -> Result<Option<Option<String>>, StorageError> {
        if email.len() > 254 || ip.len() != 32 {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        sqlx::query("DELETE FROM auth_limits WHERE scope_hash IN (SELECT scope_hash FROM auth_limits WHERE window_start<$1 LIMIT 1000)").bind(now-900).execute(&mut *tx).await?;
        let allowed = limit(&mut tx, "primary-ip", ip, 30, now, 900).await?
            && limit(&mut tx, "primary-email", email.as_bytes(), 15, now, 900).await?
            && limit(&mut tx, "primary-global", b"all", 500, now, 900).await?;
        if !allowed {
            tx.commit().await?;
            return Ok(None);
        }
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM users WHERE email=$1 AND disabled_at IS NULL",
        )
        .bind(email)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
        tx.commit().await?;
        Ok(Some(hash))
    }
    pub async fn auth_primary(
        &self,
        email: &str,
        password_hash: Option<&str>,
        token: &str,
        previous: Option<&str>,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if !valid_token(token) {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let r=sqlx::query("SELECT id,password_hash,EXISTS(SELECT 1 FROM passkeys WHERE user_id=users.id) AS has_passkey FROM users WHERE email=$1 AND disabled_at IS NULL FOR UPDATE").bind(email).fetch_optional(&mut *tx).await?;
        let Some(r) = r else {
            return Ok(None);
        };
        if let Some(expected) = password_hash {
            if r.try_get::<Option<String>, _>("password_hash")?.as_deref() != Some(expected) {
                return Ok(None);
            }
        } else if !r.try_get::<bool, _>("has_passkey")? {
            return Ok(None);
        }
        let id: String = r.try_get("id")?;
        sqlx::query("DELETE FROM sessions WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        let capacity: bool = sqlx::query_scalar("SELECT count(*)<10000 FROM sessions")
            .fetch_one(&mut *tx)
            .await?;
        if !capacity {
            return Ok(None);
        }
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE user_id=$1 AND stage<>'authenticated' ORDER BY created_at DESC OFFSET 4)").bind(&id).execute(&mut *tx).await?;
        let result = rotate(
            &mut tx,
            previous.unwrap_or(token),
            &id,
            token,
            if password_hash.is_some() {
                "second_factor"
            } else {
                "primary"
            },
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(Some(result))
    }
    pub async fn auth_factor_start(
        &self,
        key: &AuthKey,
        token: &str,
        code: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(false);
        };
        if r.try_get::<String, _>("stage")? != "second_factor"
            || r.try_get::<Option<Vec<u8>>, _>("totp_secret")?.is_some()
            || r.try_get::<Option<i64>, _>("factor_sent_at")?
                .is_some_and(|t| t > now - 60)
        {
            return Ok(false);
        }
        let id: String = r.try_get("id")?;
        let email: String = r.try_get("email")?;
        if !limit(&mut tx, "factor-send-user", id.as_bytes(), 5, now, 900).await? {
            tx.commit().await?;
            return Ok(false);
        }
        let verifier = key.verifier("second-factor", &email, &hash_token(token), code);
        sqlx::query("UPDATE sessions SET factor_verifier=$2,factor_sent_at=$3,factor_expires_at=$3+600 WHERE token_hash=$1").bind(hash_token(token).as_slice()).bind(verifier.as_slice()).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn auth_factor_verify(
        &self,
        key: &AuthKey,
        token: &str,
        code: &str,
        new_token: &str,
        ip: &[u8],
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        if !limit(&mut tx, "factor-ip", ip, 60, now, 900).await? {
            tx.commit().await?;
            return Ok(None);
        }
        let Some(r) = row(&mut tx, token, now).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if r.try_get::<String, _>("stage")? != "second_factor"
            || r.try_get::<Option<i64>, _>("primary_verified_at")?
                .is_none_or(|t| t > now || t <= now - 600)
            || r.try_get::<i16, _>("factor_attempts")? >= 5
        {
            tx.commit().await?;
            return Ok(None);
        }
        let id: String = r.try_get("id")?;
        let email: String = r.try_get("email")?;
        // A stolen primary credential must not reset guessing limits with fresh browsers/IPs.
        if !limit(&mut tx, "factor-user", id.as_bytes(), 20, now, 900).await?
            || !limit(&mut tx, "factor-global", b"all", 1000, now, 900).await?
        {
            tx.commit().await?;
            return Ok(None);
        }
        let mut valid = false;
        if let Some(sealed) = r.try_get::<Option<Vec<u8>>, _>("totp_secret")? {
            let secret = open_totp(key, &id, &sealed).map_err(|_| "TOTP secret unavailable")?;
            if let Some(step) = verify_totp(&secret, code, now as u64)
                .map_err(|_| "TOTP verification unavailable")?
            {
                let last_step: i64 = r.try_get("totp_last_step")?;
                if i64::try_from(step)
                    .ok()
                    .is_some_and(|step| step > last_step)
                {
                    valid = sqlx::query(
                        "UPDATE users SET totp_last_step=$2 WHERE id=$1 AND totp_last_step<$2",
                    )
                    .bind(&id)
                    .bind(step as i64)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected()
                        == 1;
                }
            } else {
                let mut hashes: Vec<String> =
                    serde_json::from_value(r.try_get("recovery_hashes")?)?;
                let digest = hash_token(&format!("totp-recovery:{id}:{code}"));
                if let Some(index) = hashes
                    .iter()
                    .position(|h| constant_eq(h.as_bytes(), hex(&digest).as_bytes()))
                {
                    hashes.remove(index);
                    sqlx::query("UPDATE users SET recovery_hashes=$2 WHERE id=$1")
                        .bind(&id)
                        .bind(json!(hashes))
                        .execute(&mut *tx)
                        .await?;
                    valid = true;
                }
            }
        } else if r
            .try_get::<Option<i64>, _>("factor_expires_at")?
            .is_some_and(|t| t > now)
        {
            let verifier = key.verifier("second-factor", &email, &hash_token(token), code);
            valid = code.len() == 8
                && code.bytes().all(|b| b.is_ascii_digit())
                && r.try_get::<Option<Vec<u8>>, _>("factor_verifier")?
                    .is_some_and(|v| constant_eq(&v, &verifier));
        }
        if !valid {
            sqlx::query("UPDATE sessions SET factor_attempts=LEAST(factor_attempts+1,5) WHERE token_hash=$1").bind(hash_token(token).as_slice()).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        }
        let result = rotate(&mut tx, token, &id, new_token, "authenticated", now).await?;
        tx.commit().await?;
        Ok(Some(result))
    }
    /// Admit hashing before using a scarce Argon2 worker; recheck state again at the write.
    pub async fn auth_password_admit(&self, token: &str, now: i64) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(false);
        };
        let id: String = r.try_get("id")?;
        let setup = r.try_get::<String, _>("stage")? == "setup"
            && r.try_get::<i64, _>("session_created_at")? > now - 300
            && credentialless(&mut tx, &id).await?;
        if !setup && !fresh(&r, now)? {
            return Ok(false);
        }
        let allowed = limit(&mut tx, "password-set-user", id.as_bytes(), 5, now, 900).await?
            && limit(&mut tx, "password-set-global", b"all", 100, now, 900).await?;
        tx.commit().await?;
        Ok(allowed)
    }
    pub async fn auth_password_set(
        &self,
        token: &str,
        hash: &str,
        new_token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if hash.len() > 256 {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(None);
        };
        let id: String = r.try_get("id")?;
        let setup = r.try_get::<String, _>("stage")? == "setup"
            && r.try_get::<i64, _>("session_created_at")? > now - 300
            && credentialless(&mut tx, &id).await?;
        if !setup && !fresh(&r, now)? {
            return Ok(None);
        }
        sqlx::query("UPDATE users SET password_hash=$2 WHERE id=$1")
            .bind(&id)
            .bind(hash)
            .execute(&mut *tx)
            .await?;
        revoke_others(&mut tx, &id, token).await?;
        let result = rotate(&mut tx, token, &id, new_token, "authenticated", now).await?;
        tx.commit().await?;
        Ok(Some(result))
    }
    pub async fn auth_security(
        &self,
        token: &str,
        now: i64,
    ) -> Result<Option<Value>, StorageError> {
        let mut tx = self.pool.begin().await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(None);
        };
        if !matches!(
            r.try_get::<String, _>("stage")?.as_str(),
            "authenticated" | "setup"
        ) {
            return Ok(None);
        }
        let id: String = r.try_get("id")?;
        let keys = sqlx::query(
            "SELECT id,name,created_at FROM passkeys WHERE user_id=$1 ORDER BY created_at,id",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?;
        Ok(Some(
            json!({"has_password":r.try_get::<Option<String>,_>("password_hash")?.is_some(),"totp_enabled":r.try_get::<Option<Vec<u8>>,_>("totp_secret")?.is_some(),"recovery_codes_remaining":r.try_get::<Value,_>("recovery_hashes")?.as_array().map_or(0,Vec::len),"passkeys":keys.iter().map(|k| Ok::<_,StorageError>(json!({"id":k.try_get::<String,_>("id")?,"name":k.try_get::<String,_>("name")?,"created_at":k.try_get::<i64,_>("created_at")?}))).collect::<Result<Vec<_>,_>>()?}),
        ))
    }
    pub async fn auth_totp_start(
        &self,
        token: &str,
        sealed: &[u8],
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(false);
        };
        if !fresh(&r, now)? || r.try_get::<Option<Vec<u8>>, _>("totp_secret")?.is_some() {
            return Ok(false);
        }
        sqlx::query("UPDATE users SET totp_pending=$2,totp_pending_until=$3 WHERE id=$1")
            .bind(r.try_get::<String, _>("id")?)
            .bind(sealed)
            .bind(now + 300)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn auth_totp_confirm(
        &self,
        key: &AuthKey,
        token: &str,
        code: &str,
        new_token: &str,
        codes: &[String],
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(None);
        };
        let id: String = r.try_get("id")?;
        if !fresh(&r, now)?
            || r.try_get::<Option<Vec<u8>>, _>("totp_secret")?.is_some()
            || r.try_get::<Option<i64>, _>("totp_pending_until")?
                .is_none_or(|t| t <= now)
        {
            return Ok(None);
        }
        if !limit(&mut tx, "totp-enroll", id.as_bytes(), 5, now, 300).await? {
            tx.commit().await?;
            return Ok(None);
        }
        let Some(sealed) = r.try_get::<Option<Vec<u8>>, _>("totp_pending")? else {
            return Ok(None);
        };
        let secret = open_totp(key, &id, &sealed).map_err(|_| "TOTP secret unavailable")?;
        let Some(step) =
            verify_totp(&secret, code, now as u64).map_err(|_| "TOTP verification unavailable")?
        else {
            tx.commit().await?;
            return Ok(None);
        };
        let hashes: Vec<_> = codes
            .iter()
            .map(|code| hex(&hash_token(&format!("totp-recovery:{id}:{code}"))))
            .collect();
        sqlx::query("UPDATE users SET totp_secret=$2,totp_pending=NULL,totp_pending_until=NULL,totp_last_step=$3,recovery_hashes=$4 WHERE id=$1").bind(&id).bind(sealed).bind(step as i64).bind(json!(hashes)).execute(&mut *tx).await?;
        revoke_others(&mut tx, &id, token).await?;
        let result = rotate(&mut tx, token, &id, new_token, "authenticated", now).await?;
        tx.commit().await?;
        Ok(Some(result))
    }
    pub async fn auth_totp_disable(
        &self,
        token: &str,
        new_token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(r) = row(&mut tx, token, now).await? else {
            return Ok(None);
        };
        if !fresh(&r, now)? || r.try_get::<Option<Vec<u8>>, _>("totp_secret")?.is_none() {
            return Ok(None);
        }
        let id: String = r.try_get("id")?;
        sqlx::query("UPDATE users SET totp_secret=NULL,totp_pending=NULL,totp_pending_until=NULL,totp_last_step=-1,recovery_hashes='[]' WHERE id=$1").bind(&id).execute(&mut *tx).await?;
        revoke_others(&mut tx, &id, token).await?;
        let result = rotate(&mut tx, token, &id, new_token, "authenticated", now).await?;
        tx.commit().await?;
        Ok(Some(result))
    }
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapidmaker_auth::{
        credentials::{hash_password, seal_totp},
        random_token,
    };
    #[tokio::test]
    #[ignore = "requires dedicated AUTH_TEST_DATABASE_URL PostgreSQL database"]
    async fn passwords_email_and_totp_enforce_both_factors_and_atomic_recovery() {
        let db =
            Storage::connect(&std::env::var("AUTH_TEST_DATABASE_URL").expect("dedicated test DB"))
                .await
                .unwrap();
        let now = 1_800_000_000;
        let id = uuid::Uuid::new_v4().to_string();
        let email = format!("{id}@example.invalid");
        let key = AuthKey::from_hex(&"ab".repeat(32)).unwrap();
        let ip = hash_token("test-ip");
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,$3)")
            .bind(&id)
            .bind(&email)
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        let setup = random_token().unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at) VALUES($1,$2,$3,$3,$4)").bind(hash_token(&setup).as_slice()).bind(&id).bind(now).bind(now+600).execute(&db.pool).await.unwrap();
        assert!(db.auth_session(&setup, now).await.unwrap().is_none());
        let hash = hash_password("this is a test password").unwrap();
        let full = random_token().unwrap();
        assert!(
            db.auth_password_set(&setup, &hash, &full, now)
                .await
                .unwrap()
                .is_some()
        );
        assert!(db.auth_pending(&setup, now).await.unwrap().is_none());
        assert!(!db.auth_enrollment_allowed(&email).await.unwrap());
        // A password alone gives a browser-bound pending session, never business access.
        let pending = random_token().unwrap();
        assert_eq!(
            db.auth_primary(&email, Some(&hash), &pending, None, now + 1)
                .await
                .unwrap()
                .unwrap()
                .stage,
            "second_factor"
        );
        assert!(db.auth_session(&pending, now + 1).await.unwrap().is_none());
        assert!(
            db.auth_password_set(&pending, &hash, &random_token().unwrap(), now + 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.auth_factor_start(&key, &pending, "12345678", now + 1)
                .await
                .unwrap()
        );
        assert!(
            !db.auth_factor_start(&key, &pending, "87654321", now + 2)
                .await
                .unwrap()
        );
        let wrong = random_token().unwrap();
        assert!(
            db.auth_factor_verify(
                &key,
                &wrong,
                "12345678",
                &random_token().unwrap(),
                &ip,
                now + 2
            )
            .await
            .unwrap()
            .is_none()
        );
        let verified = random_token().unwrap();
        assert!(
            db.auth_factor_verify(&key, &pending, "12345678", &verified, &ip, now + 2)
                .await
                .unwrap()
                .is_some()
        );
        assert!(db.auth_pending(&pending, now + 2).await.unwrap().is_none());
        // Security changes rotate without extending the original primary/MFA freshness window.
        let edited = random_token().unwrap();
        let edited_state = db
            .auth_password_set(&verified, &hash, &edited, now + 60)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(edited_state.factor_verified_at, Some(now + 2));
        assert!(
            db.auth_password_set(&edited, &hash, &random_token().unwrap(), now + 303)
                .await
                .unwrap()
                .is_none()
        );
        // Enable TOTP using a known RFC secret; all older full/pending email sessions disappear.
        let pending_email = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &pending_email, None, now + 61)
            .await
            .unwrap()
            .unwrap();
        db.auth_factor_start(&key, &pending_email, "33333333", now + 61)
            .await
            .unwrap();
        let sealed = seal_totp(&key, &id, b"12345678901234567890").unwrap();
        assert!(
            db.auth_totp_start(&edited, &sealed, now + 62)
                .await
                .unwrap()
        );
        let totp_session = random_token().unwrap();
        let codes = vec!["a".repeat(32), "b".repeat(32)];
        assert!(
            db.auth_totp_confirm(&key, &edited, "687638", &totp_session, &codes, now + 62)
                .await
                .unwrap()
                .is_some()
        );
        assert!(db.auth_session(&full, now + 63).await.unwrap().is_none());
        assert!(
            db.auth_factor_verify(
                &key,
                &pending_email,
                "33333333",
                &random_token().unwrap(),
                &ip,
                now + 63
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(!db.auth_enrollment_allowed(&email).await.unwrap());
        let t1 = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &t1, None, now + 90)
            .await
            .unwrap()
            .unwrap();
        assert!(
            !db.auth_factor_start(&key, &t1, "44444444", now + 90)
                .await
                .unwrap()
        );
        assert!(
            db.auth_factor_verify(
                &key,
                &t1,
                "44444444",
                &random_token().unwrap(),
                &ip,
                now + 90
            )
            .await
            .unwrap()
            .is_none()
        );
        let done = random_token().unwrap();
        assert!(
            db.auth_factor_verify(&key, &t1, "945226", &done, &ip, now + 90)
                .await
                .unwrap()
                .is_some()
        );
        let t2 = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &t2, None, now + 91)
            .await
            .unwrap()
            .unwrap();
        assert!(
            db.auth_factor_verify(&key, &t2, "945226", &random_token().unwrap(), &ip, now + 91)
                .await
                .unwrap()
                .is_none()
        );
        // Recovery codes replace only TOTP after a valid primary, and concurrent replay succeeds once.
        let t3 = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &t3, None, now + 91)
            .await
            .unwrap()
            .unwrap();
        let a = random_token().unwrap();
        let b = random_token().unwrap();
        let (a, b) = tokio::join!(
            db.auth_factor_verify(&key, &t2, &codes[0], &a, &ip, now + 92),
            db.auth_factor_verify(&key, &t3, &codes[0], &b, &ip, now + 92)
        );
        assert_eq!(
            usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
            1
        );
        let metadata = db.auth_security(&done, now + 93).await.unwrap().unwrap();
        assert_eq!(metadata["recovery_codes_remaining"], 1);
        assert_eq!(metadata["totp_enabled"], true);
        assert!(metadata.get("totp_secret").is_none());
        assert!(metadata.get("password_hash").is_none());
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(&id)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    #[tokio::test]
    #[ignore = "requires dedicated AUTH_TEST_DATABASE_URL PostgreSQL database"]
    async fn factor_and_password_budgets_survive_new_sessions_and_ip_changes() {
        let db =
            Storage::connect(&std::env::var("AUTH_TEST_DATABASE_URL").expect("dedicated test DB"))
                .await
                .unwrap();
        let now = 1_800_001_000;
        let id = uuid::Uuid::new_v4().to_string();
        let email = format!("{id}@example.invalid");
        let key = AuthKey::from_hex(&"ab".repeat(32)).unwrap();
        let hash = hash_password("another test password").unwrap();
        sqlx::query("INSERT INTO users(id,email,created_at,password_hash) VALUES($1,$2,$3,$4)")
            .bind(&id)
            .bind(&email)
            .bind(now)
            .bind(&hash)
            .execute(&db.pool)
            .await
            .unwrap();
        let full = random_token().unwrap();
        sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at,stage,primary_verified_at,factor_verified_at) VALUES($1,$2,$3,$3,$4,'authenticated',$3,$3)").bind(hash_token(&full).as_slice()).bind(&id).bind(now).bind(now+86400).execute(&db.pool).await.unwrap();
        for _ in 0..5 {
            assert!(db.auth_password_admit(&full, now).await.unwrap());
        }
        assert!(!db.auth_password_admit(&full, now).await.unwrap());
        for browser in 0..4 {
            let pending = random_token().unwrap();
            db.auth_primary(&email, Some(&hash), &pending, None, now)
                .await
                .unwrap()
                .unwrap();
            db.auth_factor_start(&key, &pending, "12345678", now)
                .await
                .unwrap();
            for attempt in 0..5 {
                let ip = hash_token(&format!("factor-ip-{browser}-{attempt}"));
                assert!(
                    db.auth_factor_verify(
                        &key,
                        &pending,
                        "00000000",
                        &random_token().unwrap(),
                        &ip,
                        now
                    )
                    .await
                    .unwrap()
                    .is_none()
                );
            }
        }
        // A new pending session and new IP cannot use even a valid code after 20 guesses.
        let pending = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &pending, None, now)
            .await
            .unwrap()
            .unwrap();
        db.auth_factor_start(&key, &pending, "12345678", now)
            .await
            .unwrap();
        assert!(
            db.auth_factor_verify(
                &key,
                &pending,
                "12345678",
                &random_token().unwrap(),
                &hash_token("new-ip"),
                now
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(!db.auth_password_admit(&pending, now).await.unwrap());
        // Unproven passkey/primary attempts may prune other pending attempts, not full sessions.
        for i in 0..25 {
            db.auth_primary(&email, Some(&hash), &random_token().unwrap(), None, now + i)
                .await
                .unwrap()
                .unwrap();
        }
        assert!(db.auth_session(&full, now + 25).await.unwrap().is_some());
        // Budgets have a finite window, while a genuinely fresh login is still required.
        let later = now + 901;
        let pending = random_token().unwrap();
        db.auth_primary(&email, Some(&hash), &pending, None, later)
            .await
            .unwrap()
            .unwrap();
        db.auth_factor_start(&key, &pending, "12345678", later)
            .await
            .unwrap();
        let new = random_token().unwrap();
        assert!(
            db.auth_factor_verify(
                &key,
                &pending,
                "12345678",
                &new,
                &hash_token("new-ip"),
                later
            )
            .await
            .unwrap()
            .is_some()
        );
        assert!(db.auth_password_admit(&new, later).await.unwrap());
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
