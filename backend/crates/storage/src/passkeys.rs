//! Server-only, one-use WebAuthn challenges bound to a live PostgreSQL session.
use crate::{AuthSession, Storage, StorageError, auth::limit};
use rapidmaker_auth::{constant_eq, hash_token, passkeys::*, random_token, valid_token};
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

#[derive(Serialize)]
pub struct PasskeyStart {
    pub id: String,
    pub options: Value,
}
#[derive(Serialize)]
pub struct PasskeyInfo {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

impl Storage {
    /// Trusted local CLI only. A verified active client session is required; no role is granted yet.
    pub async fn grant_admin(&self, email: &str, now: i64) -> Result<Option<String>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE role='admin')")
                .fetch_one(&mut *tx)
                .await?;
        if exists {
            return Ok(None);
        }
        let row=sqlx::query("SELECT u.id,s.token_hash FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.email=$1 AND u.role='client' AND u.disabled_at IS NULL AND s.expires_at>$2 AND s.last_seen_at>$2-604800 AND s.created_at>$2-600 AND s.stage='authenticated' AND s.factor_verified_at>$2-300 ORDER BY s.created_at DESC LIMIT 1 FOR UPDATE OF u,s")
            .bind(email).bind(now).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let secret = random_token().map_err(|_| "bootstrap entropy unavailable")?;
        sqlx::query("INSERT INTO admin_bootstrap(singleton,user_id,session_hash,secret_hash,expires_at) VALUES(true,$1,$2,$3,$4) ON CONFLICT(singleton) DO UPDATE SET user_id=EXCLUDED.user_id,session_hash=EXCLUDED.session_hash,secret_hash=EXCLUDED.secret_hash,expires_at=EXCLUDED.expires_at,recovery=false")
            .bind(row.try_get::<String,_>("id")?).bind(row.try_get::<Vec<u8>,_>("token_hash")?).bind(hash_token(&secret).as_slice()).bind(now+600).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(secret))
    }

    /// Trusted local key replacement requires an already fully authenticated, fresh admin session.
    /// This is not password/TOTP recovery: email-only sessions and TOTP users are refused.
    pub async fn recover_admin(
        &self,
        email: &str,
        now: i64,
    ) -> Result<Option<String>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let row=sqlx::query("SELECT u.id,s.token_hash FROM users u JOIN sessions s ON s.user_id=u.id WHERE u.email=$1 AND u.role='admin' AND u.disabled_at IS NULL AND s.expires_at>$2 AND s.created_at>$2-600 AND s.stage='authenticated' AND s.primary_verified_at>$2-300 AND s.factor_verified_at>$2-300 AND u.totp_secret IS NULL AND u.password_hash IS NOT NULL ORDER BY s.created_at DESC LIMIT 1 FOR UPDATE OF u,s")
            .bind(email).bind(now).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let user_id: String = row.try_get("id")?;
        let session_hash: Vec<u8> = row.try_get("token_hash")?;
        sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND token_hash<>$2")
            .bind(&user_id)
            .bind(&session_hash)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM passkey_challenges WHERE user_id=$1")
            .bind(&user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM passkeys WHERE user_id=$1")
            .bind(&user_id)
            .execute(&mut *tx)
            .await?;
        let secret = random_token().map_err(|_| "bootstrap entropy unavailable")?;
        sqlx::query("INSERT INTO admin_bootstrap(singleton,user_id,session_hash,secret_hash,expires_at,recovery) VALUES(true,$1,$2,$3,$4,true) ON CONFLICT(singleton) DO UPDATE SET user_id=EXCLUDED.user_id,session_hash=EXCLUDED.session_hash,secret_hash=EXCLUDED.secret_hash,expires_at=EXCLUDED.expires_at,recovery=true")
            .bind(user_id).bind(session_hash).bind(hash_token(&secret).as_slice()).bind(now+600).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(secret))
    }

    pub async fn passkey_registration_start(
        &self,
        webauthn: &Webauthn,
        token: &str,
        bootstrap: Option<&str>,
        now: i64,
    ) -> Result<Option<PasskeyStart>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(None);
        };
        let user_id: String = row.try_get("id")?;
        let bootstrap_valid = bootstrap_allowed(&mut tx, token, &user_id, bootstrap, now).await?;
        let enrollment = enrollment_allowed(&mut tx, &row, now).await?;
        if !fresh_admin(&row, now)? && !enrollment {
            return Ok(None);
        }
        let keys = keys(&mut tx, &user_id).await?;
        if keys.len() >= 5 {
            return Ok(None);
        }
        let email: String = row.try_get("email")?;
        let excluded = keys
            .iter()
            .map(|(_, key, _)| key.cred_id().clone())
            .collect();
        let (options, state) = webauthn.start_passkey_registration(
            Uuid::parse_str(&user_id)?,
            &email,
            &email,
            Some(excluded),
        )?;
        let result = save_challenge(
            &mut tx,
            token,
            &user_id,
            "register",
            serde_json::to_value(state)?,
            bootstrap_valid,
            serde_json::to_value(options)?,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn passkey_registration_finish(
        &self,
        webauthn: &Webauthn,
        token: &str,
        id: &str,
        response: &RegisterPublicKeyCredential,
        name: &str,
        new_token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if name.trim().is_empty()
            || name.chars().count() > 64
            || name.chars().any(char::is_control)
            || !valid_token(new_token)
        {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(None);
        };
        let Some(challenge) = consume(&mut tx, token, id, "register", now).await? else {
            return Ok(None);
        };
        let user_id: String = row.try_get("id")?;
        let bootstrap: bool = challenge.try_get("bootstrap")?;
        let allowed = (fresh_admin(&row, now)? || enrollment_allowed(&mut tx, &row, now).await?)
            && (!bootstrap || bootstrap_active(&mut tx, token, &user_id, now).await?);
        if !allowed || keys(&mut tx, &user_id).await?.len() >= 5 {
            tx.commit().await?;
            return Ok(None);
        }
        let state: PasskeyRegistration = serde_json::from_value(challenge.try_get("state")?)?;
        let key = match webauthn.finish_passkey_registration(response, &state) {
            Ok(key) => key,
            Err(_) => {
                tx.commit().await?;
                return Ok(None);
            }
        };
        let cred_id: &[u8] = key.cred_id().as_ref();
        let inserted=sqlx::query("INSERT INTO passkeys(id,credential_id,user_id,credential,name,created_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(credential_id) DO NOTHING")
            .bind(Uuid::new_v4().to_string()).bind(cred_id).bind(&user_id).bind(serde_json::to_value(&key)?).bind(name.trim()).bind(now).execute(&mut *tx).await?.rows_affected();
        if inserted != 1 {
            tx.commit().await?;
            return Ok(None);
        }
        if bootstrap {
            // Trigger revokes all old sessions and their pending challenges/bootstrap grants.
            sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
                .bind(&user_id)
                .execute(&mut *tx)
                .await?;
        }
        crate::credentials::revoke_others(&mut tx, &user_id, token).await?;
        let result = rotate(&mut tx, token, &user_id, new_token, now).await?;
        tx.commit().await?;
        Ok(Some(result))
    }

    pub async fn passkey_login_start(
        &self,
        webauthn: &Webauthn,
        token: &str,
        now: i64,
    ) -> Result<Option<PasskeyStart>, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(None);
        };
        if row.try_get::<String, _>("stage")? != "primary" {
            return Ok(None);
        }
        let id: String = row.try_get("id")?;
        let keys = keys(&mut tx, &id)
            .await?
            .into_iter()
            .map(|(_, key, _)| key)
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return Ok(None);
        }
        let (options, state) = webauthn.start_passkey_authentication(&keys)?;
        let result = save_challenge(
            &mut tx,
            token,
            &id,
            "login",
            serde_json::to_value(state)?,
            false,
            serde_json::to_value(options)?,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn passkey_login_finish(
        &self,
        webauthn: &Webauthn,
        token: &str,
        id: &str,
        response: &PublicKeyCredential,
        new_token: &str,
        now: i64,
    ) -> Result<Option<AuthSession>, StorageError> {
        if !valid_token(new_token) {
            return Ok(None);
        }
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(None);
        };
        if row.try_get::<String, _>("stage")? != "primary" {
            return Ok(None);
        }
        let Some(challenge) = consume(&mut tx, token, id, "login", now).await? else {
            return Ok(None);
        };
        let state: PasskeyAuthentication = serde_json::from_value(challenge.try_get("state")?)?;
        let result = match webauthn.finish_passkey_authentication(response, &state) {
            Ok(result) if result.user_verified() => result,
            _ => {
                tx.commit().await?;
                return Ok(None);
            }
        };
        let user_id: String = row.try_get("id")?;
        let mut matched = false;
        for (id, mut key, counter) in keys(&mut tx, &user_id).await? {
            if key.cred_id() != result.cred_id() {
                continue;
            }
            if result.counter() > 0 && counter > 0 && i64::from(result.counter()) <= counter {
                tx.commit().await?;
                return Ok(None);
            }
            key.update_credential(&result);
            sqlx::query(
                "UPDATE passkeys SET credential=$2,counter=GREATEST(counter,$3) WHERE id=$1",
            )
            .bind(id)
            .bind(serde_json::to_value(&key)?)
            .bind(i64::from(result.counter()))
            .execute(&mut *tx)
            .await?;
            matched = true;
        }
        if !matched {
            tx.commit().await?;
            return Ok(None);
        }
        let session =
            crate::credentials::rotate(&mut tx, token, &user_id, new_token, "second_factor", now)
                .await?;
        tx.commit().await?;
        Ok(Some(session))
    }

    pub async fn passkeys_list(
        &self,
        token: &str,
        now: i64,
    ) -> Result<Option<Vec<PasskeyInfo>>, StorageError> {
        let mut tx = self.pool.begin().await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(None);
        };
        if !fresh_admin(&row, now)? {
            return Ok(None);
        }
        let rows = sqlx::query(
            "SELECT id,name,created_at FROM passkeys WHERE user_id=$1 ORDER BY created_at,id",
        )
        .bind(row.try_get::<String, _>("id")?)
        .fetch_all(&mut *tx)
        .await?;
        let result = rows
            .into_iter()
            .map(|row| {
                Ok(PasskeyInfo {
                    id: row.try_get("id")?,
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<_, StorageError>>()?;
        tx.commit().await?;
        Ok(Some(result))
    }

    pub async fn passkey_remove(
        &self,
        token: &str,
        id: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx).await?;
        let Some(row) = session(&mut tx, token, now).await? else {
            return Ok(false);
        };
        if !fresh_admin(&row, now)? {
            return Ok(false);
        }
        let user_id: String = row.try_get("id")?;
        if keys(&mut tx, &user_id).await?.len() <= 1
            && row.try_get::<Option<String>, _>("password_hash")?.is_none()
        {
            return Ok(false);
        }
        let deleted = sqlx::query("DELETE FROM passkeys WHERE id=$1 AND user_id=$2")
            .bind(id)
            .bind(&user_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
        if deleted {
            crate::credentials::revoke_others(&mut tx, &user_id, token).await?;
        }
        tx.commit().await?;
        Ok(deleted)
    }
}

async fn lock(tx: &mut Transaction<'_, Postgres>) -> Result<(), StorageError> {
    sqlx::query("SELECT pg_advisory_xact_lock(1380794709)")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
async fn session(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    now: i64,
) -> Result<Option<PgRow>, StorageError> {
    crate::credentials::row(tx, token, now).await
}
fn fresh_admin(row: &PgRow, now: i64) -> Result<bool, StorageError> {
    crate::credentials::fresh(row, now)
}
async fn enrollment_allowed(
    tx: &mut Transaction<'_, Postgres>,
    row: &PgRow,
    now: i64,
) -> Result<bool, StorageError> {
    Ok(row.try_get::<String, _>("stage")? == "setup"
        && row.try_get::<i64, _>("session_created_at")? > now - 300
        && crate::credentials::credentialless(tx, &row.try_get::<String, _>("id")?).await?)
}
async fn bootstrap_active(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    user_id: &str,
    now: i64,
) -> Result<bool, StorageError> {
    Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM admin_bootstrap b JOIN users u ON u.id=b.user_id WHERE b.session_hash=$1 AND b.user_id=$2 AND b.expires_at>$3 AND ((u.role='client' AND NOT b.recovery AND NOT EXISTS(SELECT 1 FROM users WHERE role='admin')) OR (u.role='admin' AND b.recovery)))")
        .bind(hash_token(token).as_slice()).bind(user_id).bind(now).fetch_one(&mut **tx).await?)
}
async fn bootstrap_allowed(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    user_id: &str,
    secret: Option<&str>,
    now: i64,
) -> Result<bool, StorageError> {
    let Some(secret) = secret.filter(|secret| valid_token(secret)) else {
        return Ok(false);
    };
    if !bootstrap_active(tx, token, user_id, now).await? {
        return Ok(false);
    }
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT secret_hash FROM admin_bootstrap WHERE singleton=true")
            .fetch_one(&mut **tx)
            .await?;
    Ok(constant_eq(&stored, &hash_token(secret)))
}
async fn keys(
    tx: &mut Transaction<'_, Postgres>,
    user_id: &str,
) -> Result<Vec<(String, Passkey, i64)>, StorageError> {
    sqlx::query(
        "SELECT id,credential,counter FROM passkeys WHERE user_id=$1 ORDER BY id FOR UPDATE",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|row| {
        Ok((
            row.try_get("id")?,
            serde_json::from_value(row.try_get("credential")?)?,
            row.try_get("counter")?,
        ))
    })
    .collect()
}
#[allow(clippy::too_many_arguments)]
async fn save_challenge(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    user_id: &str,
    purpose: &str,
    state: Value,
    bootstrap: bool,
    options: Value,
    now: i64,
) -> Result<Option<PasskeyStart>, StorageError> {
    sqlx::query("DELETE FROM passkey_challenges WHERE id IN (SELECT id FROM passkey_challenges WHERE expires_at<$1 LIMIT 100)").bind(now).execute(&mut **tx).await?;
    let capacity: bool = sqlx::query_scalar("SELECT count(*)<10000 FROM passkey_challenges")
        .fetch_one(&mut **tx)
        .await?;
    if !capacity || !limit(tx, "passkey-start", user_id.as_bytes(), 10, now, 300).await? {
        return Ok(None);
    }
    sqlx::query("UPDATE passkey_challenges SET consumed_at=$3 WHERE session_hash=$1 AND purpose=$2 AND consumed_at IS NULL").bind(hash_token(token).as_slice()).bind(purpose).bind(now).execute(&mut **tx).await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO passkey_challenges(id,session_hash,user_id,purpose,state,bootstrap,created_at,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(&id).bind(hash_token(token).as_slice()).bind(user_id).bind(purpose).bind(state).bind(bootstrap).bind(now).bind(now+300).execute(&mut **tx).await?;
    Ok(Some(PasskeyStart { id, options }))
}
async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    id: &str,
    purpose: &str,
    now: i64,
) -> Result<Option<PgRow>, StorageError> {
    Ok(sqlx::query("UPDATE passkey_challenges SET consumed_at=$4 WHERE id=$1 AND session_hash=$2 AND purpose=$3 AND expires_at>$4 AND consumed_at IS NULL RETURNING state,bootstrap")
        .bind(id).bind(hash_token(token).as_slice()).bind(purpose).bind(now).fetch_optional(&mut **tx).await?)
}
async fn rotate(
    tx: &mut Transaction<'_, Postgres>,
    token: &str,
    user_id: &str,
    new_token: &str,
    now: i64,
) -> Result<AuthSession, StorageError> {
    crate::credentials::rotate(tx, token, user_id, new_token, "authenticated", now).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires dedicated AUTH_TEST_DATABASE_URL PostgreSQL database"]
    async fn enrollment_challenges_are_bound_and_bootstrap_requires_full_mfa() {
        let db =
            Storage::connect(&std::env::var("AUTH_TEST_DATABASE_URL").expect("dedicated test DB"))
                .await
                .unwrap();
        let now = 1_800_000_000;
        let id = Uuid::new_v4().to_string();
        let email = format!("{id}@example.invalid");
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,$3)")
            .bind(&id)
            .bind(&email)
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        let token = random_token().unwrap();
        let other = random_token().unwrap();
        for token in [&token, &other] {
            sqlx::query("INSERT INTO sessions(token_hash,user_id,created_at,last_seen_at,expires_at) VALUES($1,$2,$3,$3,$4)").bind(hash_token(token).as_slice()).bind(&id).bind(now).bind(now+600).execute(&db.pool).await.unwrap();
        }
        let webauthn =
            WebauthnBuilder::new("localhost", &Url::parse("http://localhost:18430").unwrap())
                .unwrap()
                .build()
                .unwrap();
        // Email enrollment may add the very first credential but has no business/admin rights.
        assert!(db.auth_session(&token, now).await.unwrap().is_none());
        assert!(db.grant_admin(&email, now).await.unwrap().is_none());
        let start = db
            .passkey_registration_start(&webauthn, &token, None, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            start.options["publicKey"]["authenticatorSelection"]["userVerification"],
            "required"
        );
        assert!(start.options.get("state").is_none());
        let mut tx = db.pool.begin().await.unwrap();
        assert!(
            consume(&mut tx, &other, &start.id, "register", now + 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            consume(&mut tx, &token, &start.id, "login", now + 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            consume(&mut tx, &token, &start.id, "register", now + 301)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            consume(&mut tx, &token, &start.id, "register", now + 1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            consume(&mut tx, &token, &start.id, "register", now + 1)
                .await
                .unwrap()
                .is_none()
        );
        tx.commit().await.unwrap();
        // A pre-existing primary credential closes email-enrollment grants, even pending ones.
        sqlx::query("UPDATE users SET password_hash='fixture' WHERE id=$1")
            .bind(&id)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            db.passkey_registration_start(&webauthn, &token, None, now + 2)
                .await
                .unwrap()
                .is_none()
        );
        assert!(db.passkeys_list(&token, now + 2).await.unwrap().is_none());
        sqlx::query("UPDATE sessions SET stage='authenticated',primary_verified_at=$2,factor_verified_at=$2 WHERE token_hash=$1").bind(hash_token(&token).as_slice()).bind(now).execute(&db.pool).await.unwrap();
        let secret = db.grant_admin(&email, now + 3).await.unwrap().unwrap();
        assert!(
            db.passkey_registration_start(&webauthn, &token, Some(&secret), now + 3)
                .await
                .unwrap()
                .is_some()
        );
        // Role changes revoke old sessions; local recovery cannot start from email-only state.
        sqlx::query("UPDATE users SET role='admin' WHERE id=$1")
            .bind(&id)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(db.auth_session(&token, now + 4).await.unwrap().is_none());
        assert!(db.recover_admin(&email, now + 4).await.unwrap().is_none());
        sqlx::query("DELETE FROM users WHERE id=$1")
            .bind(id)
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
