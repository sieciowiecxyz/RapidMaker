use super::{Storage, StorageError};
use sqlx::Row;

/// Paths are derived from server UUIDs, never stored or supplied by clients.
pub struct StoredModel {
    pub id: String,
    pub owner_hash: String,
    pub filename: String,
    pub extension: String,
    pub source_bytes: i64,
    pub mesh_bytes: i64,
    pub sha256: String,
    pub dimensions_mm: [f64; 3],
    pub expires_at: i64,
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
        sqlx::query("INSERT INTO private_models (id,owner_hash,filename,extension,source_bytes,mesh_bytes,sha256,dimensions_mm,expires_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(&model.id).bind(&model.owner_hash).bind(&model.filename).bind(&model.extension)
            .bind(model.source_bytes).bind(model.mesh_bytes).bind(&model.sha256)
            .bind(model.dimensions_mm.as_slice()).bind(model.expires_at).execute(&mut *tx).await?;
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
