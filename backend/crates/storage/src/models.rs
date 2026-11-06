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
    pub preview_bytes: i64,
    pub sha256: String,
    pub dimensions_mm: [f64; 3],
    pub expires_at: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ModelRemoval {
    Missing,
    Protected,
    Ready,
}

/// Resource namespace only: callers must authenticate the session before using it.
pub fn account_owner(user: &str) -> String {
    rapidmaker_auth::hash_token(&format!("account-owner-v1:{user}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn stored_model(row: sqlx::postgres::PgRow) -> Result<StoredModel, StorageError> {
    Ok(StoredModel {
        id: row.try_get("id")?,
        owner_hash: row.try_get("owner_hash")?,
        user_id: row.try_get("user_id")?,
        filename: row.try_get("filename")?,
        extension: row.try_get("extension")?,
        source_bytes: row.try_get("source_bytes")?,
        mesh_bytes: row.try_get("mesh_bytes")?,
        preview_bytes: row.try_get("preview_bytes")?,
        sha256: row.try_get("sha256")?,
        dimensions_mm: row
            .try_get::<Vec<f64>, _>("dimensions_mm")?
            .try_into()
            .map_err(|_| "invalid stored dimensions")?,
        expires_at: row.try_get("expires_at")?,
    })
}

impl Storage {
    /// All writers take one database lock; concurrent processes cannot exceed quotas.
    /// Return false for quota refusal, while database failures remain errors.
    pub async fn insert_model(&self, model: &StoredModel) -> Result<bool, StorageError> {
        self.insert_model_with_limits(model, 32 * 1024 * 1024, 50 * 1024 * 1024)
            .await
    }

    /// Explicit per-file allowances; aggregate quotas and preview cap stay fixed.
    pub async fn insert_model_with_limits(
        &self,
        model: &StoredModel,
        max_source_bytes: usize,
        max_mesh_bytes: usize,
    ) -> Result<bool, StorageError> {
        if !(1..=96 * 1024 * 1024).contains(&max_source_bytes)
            || !(84..=200 * 1024 * 1024).contains(&max_mesh_bytes)
        {
            return Err("invalid model byte limits".into());
        }
        if !(1..=max_source_bytes as i64).contains(&model.source_bytes)
            || !(84..=max_mesh_bytes as i64).contains(&model.mesh_bytes)
            || !(0..=25 * 1024 * 1024).contains(&model.preview_bytes)
        {
            return Err("invalid model byte lengths".into());
        }
        let bytes = model
            .source_bytes
            .checked_add(model.mesh_bytes)
            .and_then(|bytes| bytes.checked_add(model.preview_bytes))
            .ok_or("model size overflow")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(72617069646)")
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query("SELECT COUNT(*) AS total_count, COALESCE(SUM(source_bytes+mesh_bytes+preview_bytes),0)::bigint AS total_bytes,
            COUNT(*) FILTER (WHERE owner_hash=$1) AS owner_count,
            COALESCE(SUM(source_bytes+mesh_bytes+preview_bytes) FILTER (WHERE owner_hash=$1),0)::bigint AS owner_bytes FROM private_models")
            .bind(&model.owner_hash).fetch_one(&mut *tx).await?;
        if row.try_get::<i64, _>("total_count")? >= 1000
            || row.try_get::<i64, _>("owner_count")? >= 20
            || row
                .try_get::<i64, _>("total_bytes")?
                .checked_add(bytes)
                .ok_or("quota size overflow")?
                > 8 * 1024 * 1024 * 1024
            || row
                .try_get::<i64, _>("owner_bytes")?
                .checked_add(bytes)
                .ok_or("quota size overflow")?
                > 256 * 1024 * 1024
        {
            return Ok(false);
        }
        sqlx::query("INSERT INTO private_models (id,owner_hash,filename,extension,source_bytes,mesh_bytes,sha256,dimensions_mm,expires_at,user_id,preview_bytes)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(&model.id).bind(&model.owner_hash).bind(&model.filename).bind(&model.extension)
            .bind(model.source_bytes).bind(model.mesh_bytes).bind(&model.sha256)
            .bind(model.dimensions_mm.as_slice()).bind(model.expires_at).bind(&model.user_id).bind(model.preview_bytes).execute(&mut *tx).await?;
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
            "SELECT * FROM private_models WHERE id=$1 AND owner_hash=$2 AND expires_at>$3 AND NOT deletion_started",
        )
        .bind(id)
        .bind(owner)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        row.map(stored_model).transpose()
    }

    pub async fn expired_model_ids(&self, now: i64) -> Result<Vec<String>, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let ids = sqlx::query_scalar("UPDATE private_models m SET deletion_started=true WHERE expires_at<=$1 AND NOT EXISTS(SELECT 1 FROM slicer_jobs j WHERE j.model_id=m.id AND j.status IN ('queued','running') AND (j.expires_at>$1 OR j.lease_until>$1)) AND NOT EXISTS(SELECT 1 FROM order_files f WHERE f.model_id=m.id AND (f.retain_until IS NULL OR f.retain_until>$1)) RETURNING id")
            .bind(now).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        Ok(ids)
    }

    /// Fence new quotes/holds before filesystem deletion; marked rows stay owned and retryable.
    pub async fn begin_model_removal(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<ModelRemoval, StorageError> {
        self.begin_model_removal_with_guest(id, owner, None, now)
            .await
    }

    /// The caller must authenticate the account and original guest cookie separately.
    /// The alternate namespace can authorize only unclaimed guest records.
    pub async fn begin_model_removal_with_guest(
        &self,
        id: &str,
        owner: &str,
        authorized_guest: Option<&str>,
        now: i64,
    ) -> Result<ModelRemoval, StorageError> {
        if authorized_guest
            .is_some_and(|guest| guest.len() != 64 || !guest.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("invalid guest owner hash".into());
        }
        let mut tx = self.queue_transaction().await?;
        if sqlx::query("SELECT id FROM private_models WHERE id=$1 AND (owner_hash=$2 OR (user_id IS NULL AND owner_hash=$3)) FOR UPDATE")
            .bind(id)
            .bind(owner)
            .bind(authorized_guest)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Ok(ModelRemoval::Missing);
        }
        let protected: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM slicer_jobs WHERE model_id=$1 AND status='running' AND lease_until>$2) OR EXISTS(SELECT 1 FROM order_files WHERE model_id=$1 AND (retain_until IS NULL OR retain_until>$2))")
            .bind(id).bind(now).fetch_one(&mut *tx).await?;
        if protected {
            return Ok(ModelRemoval::Protected);
        }
        sqlx::query("UPDATE slicer_jobs SET status='failed',error='model_removed',lease_token=NULL,lease_until=NULL WHERE model_id=$1 AND status IN ('queued','running')")
            .bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE private_models SET expires_at=LEAST(expires_at,$2),deletion_started=true WHERE id=$1")
            .bind(id).bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ModelRemoval::Ready)
    }

    /// Delete metadata after file deletion succeeds, so failed cleanup still counts toward quota.
    pub async fn forget_expired_model(&self, id: &str, now: i64) -> Result<(), StorageError> {
        sqlx::query(
            "DELETE FROM private_models WHERE id=$1 AND expires_at<=$2 AND deletion_started",
        )
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
        let mut tx = self.queue_transaction().await?;
        if sqlx::query("SELECT id FROM users WHERE id=$1 AND disabled_at IS NULL FOR SHARE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?
            .is_none()
        {
            return Err("account unavailable".into());
        }
        let models = sqlx::query("UPDATE private_models SET user_id=$1,owner_hash=$3 WHERE user_id IS NULL AND owner_hash=$2 AND expires_at>$4 AND NOT deletion_started")
            .bind(user).bind(guest).bind(&account).bind(now).execute(&mut *tx).await?.rows_affected();
        sqlx::query("UPDATE slicer_jobs j SET owner_hash=$3 WHERE owner_hash=$2 AND EXISTS(SELECT 1 FROM private_models m WHERE m.id=j.model_id AND m.user_id=$1 AND m.owner_hash=$3)")
            .bind(user).bind(guest).bind(&account).execute(&mut *tx).await?;
        sqlx::query("UPDATE order_file_holds h SET owner_hash=$3 WHERE owner_hash=$2 AND EXISTS(SELECT 1 FROM order_files f JOIN private_models m ON m.id=f.model_id WHERE f.order_id=h.order_id AND m.user_id=$1 AND m.owner_hash=$3) AND NOT EXISTS(SELECT 1 FROM order_files f LEFT JOIN private_models m ON m.id=f.model_id WHERE f.order_id=h.order_id AND (m.id IS NULL OR m.owner_hash<>$3))")
            .bind(user).bind(guest).bind(&account).execute(&mut *tx).await?;
        let quotes = sqlx::query("UPDATE quotes q SET user_id=$1,request=jsonb_set(jsonb_set(request,'{owner_hash}',to_jsonb($3::text)),'{user_id}',to_jsonb($1::text)) WHERE user_id IS NULL AND request->>'owner_hash'=$2 AND (request->>'expires_at')::bigint>$4 AND EXISTS(SELECT 1 FROM private_models m WHERE m.id=q.request->>'model_id' AND m.user_id=$1 AND m.owner_hash=$3)")
            .bind(user).bind(guest).bind(&account).bind(now).execute(&mut *tx).await?.rows_affected();
        let orders: Vec<String> = sqlx::query_scalar("UPDATE orders SET user_id=$1,owner_hash=$3 WHERE user_id IS NULL AND owner_hash=$2 AND (created_at>$4-86400 OR EXISTS(SELECT 1 FROM payment_attempts p JOIN checkout_drafts d ON d.id=p.draft_id WHERE p.id=orders.payment_attempt_id AND d.owner_hash=$2 AND d.locked)) RETURNING id")
            .bind(user).bind(guest).bind(&account).bind(now).fetch_all(&mut *tx).await?;
        for id in &orders {
            sqlx::query("UPDATE email_outbox SET user_id=$1 WHERE left(event_key,length($2))=$2 AND user_id IS NULL AND status IN ('pending','sending')")
                .bind(user).bind(format!("order:{id}:")).execute(&mut *tx).await?;
        }
        let drafts = sqlx::query("UPDATE checkout_drafts SET user_id=$1,owner_hash=$3 WHERE user_id IS NULL AND owner_hash=$2 AND (expires_at>$4 OR locked)")
            .bind(user).bind(guest).bind(&account).bind(now).execute(&mut *tx).await?.rows_affected();
        sqlx::query("UPDATE payment_attempts p SET owner_hash=$3 WHERE owner_hash=$2 AND EXISTS(SELECT 1 FROM checkout_drafts d WHERE d.id=p.draft_id AND d.user_id=$1 AND d.owner_hash=$3)")
            .bind(user).bind(guest).bind(&account).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(models + quotes + orders.len() as u64 + drafts)
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
    async fn model_removal_guest_fallback_is_targeted_and_cannot_override_account_ownership() {
        let db = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let now = 2_000_000_000;
        let users = [
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        ];
        for user in &users {
            sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,$3)")
                .bind(user)
                .bind(format!("{user}@example.com"))
                .bind(now)
                .execute(&db.pool)
                .await
                .unwrap();
        }
        let account = account_owner(&users[0]);
        let other_account = account_owner(&users[1]);
        let guest = account_owner(&uuid::Uuid::new_v4().to_string());
        let wrong_guest = "b".repeat(64);
        let mut model = StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: guest.clone(),
            user_id: None,
            filename: "inflight.stl".into(),
            extension: "stl".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 0,
            sha256: "a".repeat(64),
            dimensions_mm: [20.; 3],
            expires_at: now + 3600,
        };
        assert!(db.insert_model(&model).await.unwrap());
        let target = model.id.clone();
        model.id = uuid::Uuid::new_v4().to_string();
        assert!(db.insert_model(&model).await.unwrap());
        let unrelated = model.id.clone();
        assert_eq!(
            db.begin_model_removal_with_guest(&target, &account, Some(&wrong_guest), now)
                .await
                .unwrap(),
            ModelRemoval::Missing
        );
        assert_eq!(
            db.begin_model_removal(&target, &other_account, now)
                .await
                .unwrap(),
            ModelRemoval::Missing
        );
        assert_eq!(
            db.begin_model_removal_with_guest(&target, &account, Some(&guest), now)
                .await
                .unwrap(),
            ModelRemoval::Ready
        );
        let (owner, user): (String, Option<String>) =
            sqlx::query_as("SELECT owner_hash,user_id FROM private_models WHERE id=$1")
                .bind(&target)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(owner, guest);
        assert!(user.is_none());
        assert!(db.model(&unrelated, &guest, now).await.unwrap().is_some());
        assert_eq!(
            db.begin_model_removal_with_guest(&target, &account, Some(&guest), now)
                .await
                .unwrap(),
            ModelRemoval::Ready
        );

        model.id = uuid::Uuid::new_v4().to_string();
        model.owner_hash = other_account.clone();
        model.user_id = Some(users[1].clone());
        assert!(db.insert_model(&model).await.unwrap());
        for fallback in [&guest, &other_account] {
            assert_eq!(
                db.begin_model_removal_with_guest(&model.id, &account, Some(fallback), now)
                    .await
                    .unwrap(),
                ModelRemoval::Missing
            );
        }
        assert_eq!(
            db.begin_model_removal(&model.id, &guest, now)
                .await
                .unwrap(),
            ModelRemoval::Missing
        );
        assert!(
            db.model(&model.id, &other_account, now)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            db.begin_model_removal(&model.id, &other_account, now)
                .await
                .unwrap(),
            ModelRemoval::Ready
        );
        sqlx::query("DELETE FROM private_models WHERE id=ANY($1)")
            .bind(vec![target, unrelated, model.id])
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id=ANY($1)")
            .bind(users.to_vec())
            .execute(&db.pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn working_model_removal_fences_jobs_preserves_holds_and_releases_quota() {
        use serde_json::json;
        let db = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let now = 2_000_000_000;
        let owner = account_owner(&uuid::Uuid::new_v4().to_string());
        let mut model = StoredModel {
            id: String::new(),
            owner_hash: owner.clone(),
            user_id: None,
            filename: "removal.stl".into(),
            extension: "stl".into(),
            source_bytes: 32 * 1024 * 1024,
            mesh_bytes: 50 * 1024 * 1024,
            preview_bytes: 0,
            sha256: "a".repeat(64),
            dimensions_mm: [20.; 3],
            expires_at: now + 3600,
        };
        let mut ids = Vec::new();
        for _ in 0..3 {
            model.id = uuid::Uuid::new_v4().to_string();
            assert!(db.insert_model(&model).await.unwrap());
            ids.push(model.id.clone());
        }
        model.id = uuid::Uuid::new_v4().to_string();
        assert!(!db.insert_model(&model).await.unwrap());
        let queued = db
            .enqueue_slice(&owner, &ids[0], "remove-queued", &json!({}), now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[0], &"b".repeat(64), now)
                .await
                .unwrap(),
            ModelRemoval::Missing
        );
        assert_eq!(
            db.slice_job(&queued.id, &owner, now)
                .await
                .unwrap()
                .unwrap()
                .status,
            "queued"
        );
        let quote = uuid::Uuid::new_v4().to_string();
        db.save_quote(&quote, &json!({"model_id":ids[0],"owner_hash":owner,"expires_at":now+3600,"model_sha256":model.sha256}), &json!({})).await.unwrap();
        let payload = json!({});
        let (removed, raced) = tokio::join!(
            db.begin_model_removal(&ids[0], &owner, now),
            db.enqueue_slice(&owner, &ids[0], "remove-race", &payload, now)
        );
        assert_eq!(removed.unwrap(), ModelRemoval::Ready);
        let queued = db
            .slice_job(&queued.id, &owner, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(queued.status, "failed");
        assert_eq!(queued.error.as_deref(), Some("model_removed"));
        if let Some(job) = raced.unwrap() {
            assert_eq!(
                db.slice_job(&job.id, &owner, now)
                    .await
                    .unwrap()
                    .unwrap()
                    .status,
                "failed"
            );
        }
        assert_eq!(
            db.begin_model_removal(&ids[0], &owner, now).await.unwrap(),
            ModelRemoval::Ready
        );
        assert!(
            db.enqueue_slice(&owner, &ids[0], "after-remove", &json!({}), now)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !db.hold_order_files(
                &uuid::Uuid::new_v4().to_string(),
                &owner,
                std::slice::from_ref(&quote),
                now
            )
            .await
            .unwrap()
        );
        assert!(!db.insert_model(&model).await.unwrap()); // Mark alone cannot release quota.
        db.forget_expired_model(&ids[0], now).await.unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[0], &owner, now).await.unwrap(),
            ModelRemoval::Missing
        );
        assert!(db.insert_model(&model).await.unwrap());

        let running = db
            .enqueue_slice(&owner, &ids[1], "remove-running", &json!({}), now)
            .await
            .unwrap()
            .unwrap();
        sqlx::query("UPDATE slicer_jobs SET status='running',attempts=1,lease_token='removal-test',lease_until=$2 WHERE id=$1")
            .bind(&running.id).bind(now+180).execute(&db.pool).await.unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[1], &owner, now).await.unwrap(),
            ModelRemoval::Protected
        );
        assert!(db.model(&ids[1], &owner, now).await.unwrap().is_some());
        assert_eq!(
            db.slice_job(&running.id, &owner, now)
                .await
                .unwrap()
                .unwrap()
                .status,
            "running"
        );
        sqlx::query("UPDATE slicer_jobs SET lease_until=$2 WHERE id=$1")
            .bind(&running.id)
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[1], &owner, now).await.unwrap(),
            ModelRemoval::Ready
        );
        let stale = db
            .slice_job(&running.id, &owner, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stale.status, "failed");
        assert!(stale.lease_token.is_none());

        let held_quote = uuid::Uuid::new_v4().to_string();
        db.save_quote(&held_quote, &json!({"model_id":ids[2],"owner_hash":owner,"expires_at":now+3600,"model_sha256":model.sha256}), &json!({})).await.unwrap();
        let order = uuid::Uuid::new_v4().to_string();
        assert!(
            db.hold_order_files(&order, &owner, std::slice::from_ref(&held_quote), now)
                .await
                .unwrap()
        );
        assert_eq!(
            db.begin_model_removal(&ids[2], &owner, now).await.unwrap(),
            ModelRemoval::Protected
        );
        sqlx::query("UPDATE private_models SET expires_at=$2 WHERE id=$1")
            .bind(&ids[2])
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[2], &owner, now).await.unwrap(),
            ModelRemoval::Protected
        );
        assert!(
            db.order_model(&order, &ids[2], now)
                .await
                .unwrap()
                .is_some()
        );
        sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
            .bind(&order)
            .bind(now + 1)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[2], &owner, now).await.unwrap(),
            ModelRemoval::Protected
        );
        sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
            .bind(&order)
            .bind(now)
            .execute(&db.pool)
            .await
            .unwrap();
        assert_eq!(
            db.begin_model_removal(&ids[2], &owner, now).await.unwrap(),
            ModelRemoval::Ready
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT retain_until FROM order_files WHERE order_id=$1")
                .bind(&order)
                .fetch_one(&db.pool)
                .await
                .unwrap(),
            now
        );
        db.forget_expired_model(&ids[1], now).await.unwrap();
        db.forget_expired_model(&ids[2], now).await.unwrap();
        sqlx::query("DELETE FROM order_files WHERE order_id=$1")
            .bind(&order)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM order_file_holds WHERE order_id=$1")
            .bind(&order)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM quotes WHERE id=ANY($1)")
            .bind(vec![quote, held_quote])
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM slicer_jobs WHERE owner_hash=$1")
            .bind(&owner)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM private_models WHERE owner_hash=$1")
            .bind(&owner)
            .execute(&db.pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn explicit_model_limits_fail_closed_before_database_access() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused/unused")
            .unwrap();
        pool.close().await;
        let db = Storage { pool };
        let mut model = StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: "a".repeat(64),
            user_id: None,
            filename: "large.stl".into(),
            extension: "stl".into(),
            source_bytes: 33 * 1024 * 1024,
            mesh_bytes: 51 * 1024 * 1024,
            preview_bytes: 25 * 1024 * 1024,
            sha256: "a".repeat(64),
            dimensions_mm: [20.; 3],
            expires_at: 2_000_000_000,
        };
        assert_eq!(
            db.insert_model(&model).await.unwrap_err().to_string(),
            "invalid model byte lengths"
        );
        let source_limit = 96 * 1024 * 1024;
        let mesh_limit = 200 * 1024 * 1024;
        for (source, mesh) in [
            (0, mesh_limit),
            (source_limit + 1, mesh_limit),
            (source_limit, 83),
            (source_limit, mesh_limit + 1),
        ] {
            assert_eq!(
                db.insert_model_with_limits(&model, source, mesh)
                    .await
                    .unwrap_err()
                    .to_string(),
                "invalid model byte limits"
            );
        }
        model.source_bytes = source_limit as i64;
        model.mesh_bytes = mesh_limit as i64;
        let error = db
            .insert_model_with_limits(&model, source_limit, mesh_limit)
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::PoolClosed)
        ));
        model.source_bytes += 1;
        assert_eq!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .unwrap_err()
                .to_string(),
            "invalid model byte lengths"
        );
        model.source_bytes -= 1;
        model.mesh_bytes += 1;
        assert_eq!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .unwrap_err()
                .to_string(),
            "invalid model byte lengths"
        );
        model.mesh_bytes -= 1;
        model.preview_bytes += 1;
        assert_eq!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .unwrap_err()
                .to_string(),
            "invalid model byte lengths"
        );
    }

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn preview_bytes_count_toward_quota_and_validate_boundaries() {
        let db = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let owner = account_owner(&uuid::Uuid::new_v4().to_string());
        let mut model = StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: owner.clone(),
            user_id: None,
            filename: "quota.stl".into(),
            extension: "stl".into(),
            source_bytes: 1024 * 1024,
            mesh_bytes: 38 * 1024 * 1024,
            preview_bytes: 25 * 1024 * 1024,
            sha256: "a".repeat(64),
            dimensions_mm: [20.0; 3],
            expires_at: 2_000_000_000,
        };
        for invalid in [-1, 25 * 1024 * 1024 + 1] {
            model.preview_bytes = invalid;
            assert!(db.insert_model(&model).await.is_err());
        }
        model.preview_bytes = 25 * 1024 * 1024;
        for _ in 0..4 {
            model.id = uuid::Uuid::new_v4().to_string();
            assert!(db.insert_model(&model).await.unwrap());
        }
        assert_eq!(
            db.model(&model.id, &owner, 1000)
                .await
                .unwrap()
                .unwrap()
                .preview_bytes,
            25 * 1024 * 1024
        );
        // Four 64 MiB records reach exactly 256 MiB; previews are 100 MiB of it.
        model.id = uuid::Uuid::new_v4().to_string();
        model.source_bytes = 1;
        model.mesh_bytes = 84;
        model.preview_bytes = 0;
        assert!(!db.insert_model(&model).await.unwrap());
        sqlx::query("DELETE FROM private_models WHERE owner_hash=$1")
            .bind(&owner)
            .execute(&db.pool)
            .await
            .unwrap();
        // The same small record fits once the quota is released; legacy zero is valid.
        assert!(db.insert_model(&model).await.unwrap());
        // Larger per-file allowances are explicit and retain hard upper bounds.
        model.id = uuid::Uuid::new_v4().to_string();
        model.source_bytes = 33 * 1024 * 1024;
        model.mesh_bytes = 51 * 1024 * 1024;
        assert!(db.insert_model(&model).await.is_err());
        let source_limit = 96 * 1024 * 1024;
        let mesh_limit = 200 * 1024 * 1024;
        assert!(
            db.insert_model_with_limits(&model, source_limit + 1, mesh_limit)
                .await
                .is_err()
        );
        assert!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit + 1)
                .await
                .is_err()
        );
        assert!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .unwrap()
        );
        model.id = uuid::Uuid::new_v4().to_string();
        model.source_bytes = source_limit as i64 + 1;
        assert!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .is_err()
        );
        model.source_bytes = 33 * 1024 * 1024;
        model.mesh_bytes = mesh_limit as i64 + 1;
        assert!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .is_err()
        );
        model.mesh_bytes = 51 * 1024 * 1024;
        model.preview_bytes = 25 * 1024 * 1024 + 1;
        assert!(
            db.insert_model_with_limits(&model, source_limit, mesh_limit)
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM private_models WHERE owner_hash=$1")
            .bind(&owner)
            .execute(&db.pool)
            .await
            .unwrap();
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
                    preview_bytes: 0,
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
