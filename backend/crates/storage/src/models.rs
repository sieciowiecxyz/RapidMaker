use super::{Storage, StorageError};
use sqlx::Row;

/// Paths are derived from server UUIDs, never stored or supplied by clients.
pub struct StoredModel {
    pub id: String,
    pub owner_hash: String,
    pub user_id: Option<String>,
    pub filename: String,
    pub extension: String,
    pub source_bytes: i64,
    pub mesh_bytes: i64,
    pub sha256: String,
    pub dimensions_mm: [f64; 3],
    pub expires_at: i64,
}

/// Resource namespace only: callers must authenticate the session before using it.
pub fn account_owner(user: &str) -> String {
    rapidmaker_auth::hash_token(&format!("account-owner-v1:{user}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Storage {
    /// All writers take one database lock; concurrent processes cannot exceed quotas.
    /// Return false for quota refusal, while database failures remain errors.
    pub async fn insert_model(&self, model: &StoredModel) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(72617069646)")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query("SELECT COUNT(*) AS total_count, COALESCE(SUM(source_bytes+mesh_bytes),0)::bigint AS total_bytes,
            COUNT(*) FILTER (WHERE owner_hash=$1) AS owner_count,
            COALESCE(SUM(source_bytes+mesh_bytes) FILTER (WHERE owner_hash=$1),0)::bigint AS owner_bytes FROM private_models")
            .bind(&model.owner_hash).fetch_one(&mut *tx).await?;
        let bytes = model
            .source_bytes
            .checked_add(model.mesh_bytes)
            .ok_or("model size overflow")?;
        if row.try_get::<i64, _>("total_count")? >= 1000
            || row.try_get::<i64, _>("owner_count")? >= 20
            || row.try_get::<i64, _>("total_bytes")? + bytes > 8 * 1024 * 1024 * 1024
            || row.try_get::<i64, _>("owner_bytes")? + bytes > 256 * 1024 * 1024
        {
            return Ok(false);
        }
        sqlx::query("INSERT INTO private_models (id,owner_hash,filename,extension,source_bytes,mesh_bytes,sha256,dimensions_mm,expires_at,user_id)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(&model.id).bind(&model.owner_hash).bind(&model.filename).bind(&model.extension)
            .bind(model.source_bytes).bind(model.mesh_bytes).bind(&model.sha256)
            .bind(model.dimensions_mm.as_slice()).bind(model.expires_at).bind(&model.user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn model(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<Option<StoredModel>, StorageError> {
        let row = sqlx::query(
            "SELECT * FROM private_models WHERE id=$1 AND owner_hash=$2 AND expires_at>$3",
        )
        .bind(id)
        .bind(owner)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(StoredModel {
                id: row.try_get("id")?,
                owner_hash: row.try_get("owner_hash")?,
                user_id: row.try_get("user_id")?,
                filename: row.try_get("filename")?,
                extension: row.try_get("extension")?,
                source_bytes: row.try_get("source_bytes")?,
                mesh_bytes: row.try_get("mesh_bytes")?,
                sha256: row.try_get("sha256")?,
                dimensions_mm: row
                    .try_get::<Vec<f64>, _>("dimensions_mm")?
                    .try_into()
                    .map_err(|_| "invalid stored dimensions")?,
                expires_at: row.try_get("expires_at")?,
            })
        })
        .transpose()
    }

    pub async fn expired_model_ids(&self, now: i64) -> Result<Vec<String>, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT id FROM private_models WHERE expires_at<=$1 LIMIT 1000")
                .bind(now)
                .fetch_all(&self.pool)
                .await?,
        )
    }

    /// Delete metadata after file deletion succeeds, so failed cleanup still counts toward quota.
    pub async fn forget_expired_model(&self, id: &str, now: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM private_models WHERE id=$1 AND expires_at<=$2")
            .bind(id)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

impl Storage {
    /// Both the authenticated account and original guest cookie must be checked by the caller.
    /// A contact email never grants ownership; already claimed resources are excluded.
    pub async fn claim_guest(
        &self,
        user: &str,
        guest: &str,
        now: i64,
    ) -> Result<u64, StorageError> {
        if guest.len() != 64 || !guest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid guest owner hash".into());
        }
        let account = account_owner(user);
        let mut tx = self.pool.begin().await?;
        if sqlx::query("SELECT id FROM users WHERE id=$1 AND disabled_at IS NULL FOR SHARE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Err("account unavailable".into());
        }
        let models = sqlx::query("UPDATE private_models SET user_id=$1,owner_hash=$3 WHERE user_id IS NULL AND owner_hash=$2 AND expires_at>$4")
            .bind(user).bind(guest).bind(&account).bind(now).execute(&mut *tx).await?.rows_affected();
        let quotes = sqlx::query("UPDATE quotes q SET user_id=$1,request=jsonb_set(jsonb_set(request,'{owner_hash}',to_jsonb($3::text)),'{user_id}',to_jsonb($1::text)) WHERE user_id IS NULL AND request->>'owner_hash'=$2 AND (request->>'expires_at')::bigint>$4 AND EXISTS(SELECT 1 FROM private_models m WHERE m.id=q.request->>'model_id' AND m.user_id=$1 AND m.owner_hash=$3)")
            .bind(user).bind(guest).bind(&account).bind(now).execute(&mut *tx).await?.rows_affected();
        let orders: Vec<String> = sqlx::query_scalar("UPDATE orders SET user_id=$1,owner_hash=$3 WHERE user_id IS NULL AND owner_hash=$2 AND created_at>$4-86400 RETURNING id")
            .bind(user).bind(guest).bind(&account).bind(now).fetch_all(&mut *tx).await?;
        for id in &orders {
            sqlx::query("UPDATE email_outbox SET user_id=$1 WHERE left(event_key,length($2))=$2 AND user_id IS NULL AND status IN ('pending','sending')")
                .bind(user).bind(format!("order:{id}:")).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(models + quotes + orders.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn account_owner_has_a_distinct_stable_namespace() {
        let owner = account_owner("alice");
        assert_eq!(owner.len(), 64);
        assert_eq!(owner, account_owner("alice"));
        assert_ne!(owner, account_owner("bob"));
        let raw_guest = rapidmaker_auth::hash_token("alice")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_ne!(owner, raw_guest);
    }

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn claim_moves_only_owned_live_resources_and_cannot_override_an_account() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        let db = Storage::connect(&url).await.unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        for user in ["claim-alice", "claim-bob"] {
            sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,1000)")
                .bind(user)
                .bind(format!("{user}@example.com"))
                .execute(&db.pool)
                .await
                .unwrap();
        }
        let guest = "c".repeat(64);
        let stranger = "d".repeat(64);
        let ids = [
            "00000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-000000000002",
            "00000000-0000-4000-8000-000000000003",
        ];
        for (id, owner, expiry) in [
            (ids[0], &guest, 2000),
            (ids[1], &stranger, 2000),
            (ids[2], &guest, 999),
        ] {
            assert!(
                db.insert_model(&StoredModel {
                    id: id.into(),
                    owner_hash: owner.into(),
                    user_id: None,
                    filename: "cube.stl".into(),
                    extension: "stl".into(),
                    source_bytes: 100,
                    mesh_bytes: 100,
                    sha256: "a".repeat(64),
                    dimensions_mm: [20.0; 3],
                    expires_at: expiry
                })
                .await
                .unwrap()
            );
        }
        db.save_quote(
            "claim-quote",
            &json!({"model_id":ids[0],"owner_hash":guest,"expires_at":2000}),
            &json!({}),
        )
        .await
        .unwrap();
        assert_eq!(
            db.claim_guest("claim-alice", &guest, 1000).await.unwrap(),
            2
        );
        let alice = account_owner("claim-alice");
        let bob = account_owner("claim-bob");
        let model = db.model(ids[0], &alice, 1000).await.unwrap().unwrap();
        assert_eq!(model.user_id.as_deref(), Some("claim-alice"));
        assert!(db.model(ids[0], &guest, 1000).await.unwrap().is_none());
        assert!(db.model(ids[0], &bob, 1000).await.unwrap().is_none());
        assert!(
            db.quote("claim-quote", &guest, 1000)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.quote("claim-quote", &alice, 1000)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(db.claim_guest("claim-bob", &guest, 1000).await.unwrap(), 0);
        assert!(db.model(ids[1], &stranger, 1000).await.unwrap().is_some());
        assert!(db.model(ids[2], &alice, 1000).await.unwrap().is_none());
        let expired = sqlx::query("SELECT owner_hash,user_id FROM private_models WHERE id=$1")
            .bind(ids[2])
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(expired.get::<String, _>("owner_hash"), guest);
        assert!(expired.get::<Option<String>, _>("user_id").is_none());
        sqlx::query("DELETE FROM quotes WHERE id='claim-quote'")
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM private_models WHERE id=ANY($1)")
            .bind(ids.to_vec())
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id IN ('claim-alice','claim-bob')")
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
