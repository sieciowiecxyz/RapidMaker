use super::{Storage, StorageError};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};

pub(crate) const QUEUE_LOCK: i64 = 72617069647;

#[derive(Debug)]
pub struct SlicerJob {
    pub id: String,
    pub owner_hash: String,
    pub model_id: String,
    pub payload: Value,
    pub attempts: i32,
    pub lease_token: Option<String>,
    pub status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub created_at: i64,
    pub expires_at: i64,
}

fn job(row: PgRow) -> Result<SlicerJob, sqlx::Error> {
    Ok(SlicerJob {
        id: row.try_get("id")?,
        owner_hash: row.try_get("owner_hash")?,
        model_id: row.try_get("model_id")?,
        payload: row.try_get("payload")?,
        attempts: row.try_get("attempts")?,
        lease_token: row.try_get("lease_token")?,
        status: row.try_get("status")?,
        result: row.try_get("result")?,
        error: row.try_get("error")?,
        created_at: row.try_get("created_at")?,
        expires_at: row.try_get("expires_at")?,
    })
}

impl Storage {
    pub(crate) async fn queue_transaction(
        &self,
    ) -> Result<Transaction<'_, Postgres>, StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(QUEUE_LOCK)
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    pub async fn enqueue_slice(
        &self,
        owner: &str,
        model_id: &str,
        fingerprint: &str,
        payload: &Value,
        now: i64,
    ) -> Result<Option<SlicerJob>, StorageError> {
        if fingerprint.is_empty()
            || fingerprint.len() > 256
            || !payload.is_object()
            || serde_json::to_vec(payload)?.len() > 65536
        {
            return Err("invalid slicer job".into());
        }
        let mut tx = self.queue_transaction().await?;
        if sqlx::query("SELECT id FROM private_models WHERE id=$1 AND owner_hash=$2 AND expires_at>$3 AND NOT deletion_started FOR UPDATE")
            .bind(model_id).bind(owner).bind(now).fetch_optional(&mut *tx).await?.is_none() { return Ok(None); }
        sqlx::query("DELETE FROM slicer_jobs WHERE expires_at<=$1 AND (status<>'running' OR lease_until<=$1)")
            .bind(now).execute(&mut *tx).await?;
        if let Some(row) = sqlx::query("SELECT * FROM slicer_jobs WHERE owner_hash=$1 AND fingerprint=$2 AND model_id=$3 AND expires_at>$4 AND (status IN ('queued','running') OR (status='succeeded' AND expires_at>$4+30)) ORDER BY created_at DESC LIMIT 1")
            .bind(owner).bind(fingerprint).bind(model_id).bind(now).fetch_optional(&mut *tx).await? { return Ok(Some(job(row)?)); }
        let counts = sqlx::query("SELECT count(*) AS retained,count(*) FILTER (WHERE status IN ('queued','running')) AS active,count(*) FILTER (WHERE owner_hash=$1 AND status IN ('queued','running')) AS owned FROM slicer_jobs")
            .bind(owner).fetch_one(&mut *tx).await?;
        if counts.try_get::<i64, _>("retained")? >= 1000
            || counts.try_get::<i64, _>("active")? >= 64
            || counts.try_get::<i64, _>("owned")? >= 4
        {
            return Ok(None);
        }
        sqlx::query(
            "UPDATE private_models SET expires_at=GREATEST(expires_at,$2+1800) WHERE id=$1",
        )
        .bind(model_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query("INSERT INTO slicer_jobs(id,owner_hash,model_id,fingerprint,payload,created_at,available_at,expires_at) VALUES($1,$2,$3,$4,$5,$6,$6,$6+1800) RETURNING *")
            .bind(uuid::Uuid::new_v4().to_string()).bind(owner).bind(model_id).bind(fingerprint).bind(payload).bind(now).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(job(row)?))
    }

    pub async fn claim_slice(&self, now: i64) -> Result<Option<SlicerJob>, StorageError> {
        let mut tx = self.queue_transaction().await?;
        sqlx::query("UPDATE slicer_jobs SET status=CASE WHEN attempts>=3 OR expires_at<=$1 THEN 'failed' ELSE 'queued' END,lease_token=NULL,lease_until=NULL,available_at=$1+5,error='worker_interrupted' WHERE status='running' AND lease_until<=$1")
            .bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE slicer_jobs SET status='failed',error='job_expired' WHERE status='queued' AND expires_at<=$1")
            .bind(now).execute(&mut *tx).await?;
        let row = sqlx::query("UPDATE slicer_jobs SET status='running',attempts=attempts+1,lease_token=$2,lease_until=$1+180 WHERE id=(SELECT id FROM slicer_jobs WHERE status='queued' AND available_at<=$1 AND expires_at>$1 AND attempts<3 AND NOT EXISTS(SELECT 1 FROM slicer_jobs WHERE status='running' AND lease_until>$1) ORDER BY created_at,id FOR UPDATE SKIP LOCKED LIMIT 1) RETURNING *")
            .bind(now).bind(rapidmaker_auth::random_token().map_err(|_| "random source unavailable")?).fetch_optional(&mut *tx).await?;
        tx.commit().await?;
        row.map(job).transpose().map_err(Into::into)
    }

    /// Fetch the current model owner, so claiming a guest account cannot invalidate a worker.
    pub async fn slice_model(
        &self,
        id: &str,
        lease_token: &str,
        now: i64,
    ) -> Result<Option<crate::StoredModel>, StorageError> {
        sqlx::query("SELECT m.* FROM slicer_jobs j JOIN private_models m ON m.id=j.model_id AND m.owner_hash=j.owner_hash WHERE j.id=$1 AND j.lease_token=$2 AND j.status='running' AND j.lease_until>$3 AND j.expires_at>$3 AND m.expires_at>$3 AND NOT m.deletion_started")
            .bind(id).bind(lease_token).bind(now).fetch_optional(&self.pool).await?.map(crate::models::stored_model).transpose()
    }

    pub async fn finish_slice(
        &self,
        id: &str,
        token: &str,
        request: &Value,
        result: &Value,
        now: i64,
    ) -> Result<bool, StorageError> {
        if !request.is_object()
            || !result.is_object()
            || serde_json::to_vec(request)?.len() > 1_048_576
            || serde_json::to_vec(result)?.len() > 1_048_576
        {
            return Err("invalid quote snapshot".into());
        }
        let mut tx = self.queue_transaction().await?;
        let row = sqlx::query("SELECT j.*,m.user_id FROM slicer_jobs j JOIN private_models m ON m.id=j.model_id WHERE j.id=$1 AND j.status='running' AND j.lease_token=$2 AND j.lease_until>$3 AND j.expires_at>$3 AND NOT m.deletion_started FOR UPDATE OF j")
            .bind(id).bind(token).bind(now).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let mut request = request.clone();
        request["owner_hash"] = Value::String(row.try_get("owner_hash")?);
        request["model_id"] = Value::String(row.try_get("model_id")?);
        request["user_id"] = row
            .try_get::<Option<String>, _>("user_id")?
            .map(Value::String)
            .unwrap_or(Value::Null);
        let expiry = request
            .get("expires_at")
            .and_then(Value::as_i64)
            .unwrap_or(now + 600)
            .min(now + 600);
        if expiry <= now {
            return Err("expired quote snapshot".into());
        }
        request["expires_at"] = Value::from(expiry);
        let mut result = result.clone();
        result["expires_at"] = Value::from(expiry);
        sqlx::query("UPDATE private_models SET expires_at=GREATEST(expires_at,$2) WHERE id=$1")
            .bind(request.get("model_id").and_then(Value::as_str))
            .bind(expiry)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO quotes(id,request,result,user_id) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(&request)
            .bind(&result)
            .bind(request.get("user_id").and_then(Value::as_str))
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE slicer_jobs SET status='succeeded',lease_token=NULL,lease_until=NULL,result=$2,error=NULL,expires_at=$3 WHERE id=$1")
            .bind(id).bind(&result).bind(expiry).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }

    pub async fn fail_slice(
        &self,
        id: &str,
        token: &str,
        retry: bool,
        safe_error: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if safe_error.len() > 512 {
            return Err("slicer error too long".into());
        }
        let mut tx = self.queue_transaction().await?;
        let updated = sqlx::query("UPDATE slicer_jobs SET status=CASE WHEN $3 AND attempts<3 AND expires_at>$5 THEN 'queued' ELSE 'failed' END,lease_token=NULL,lease_until=NULL,available_at=$5+5*attempts,error=$4 WHERE id=$1 AND lease_token=$2 AND status='running' AND lease_until>$5")
            .bind(id).bind(token).bind(retry).bind(safe_error).bind(now).execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(updated == 1)
    }

    pub async fn slice_job(
        &self,
        id: &str,
        owner: &str,
        now: i64,
    ) -> Result<Option<SlicerJob>, StorageError> {
        sqlx::query("SELECT * FROM slicer_jobs WHERE id=$1 AND owner_hash=$2 AND expires_at>$3")
            .bind(id)
            .bind(owner)
            .bind(now)
            .fetch_optional(&self.pool)
            .await?
            .map(job)
            .transpose()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StoredModel, account_owner};
    use serde_json::json;

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn queue_recovers_fences_deduplicates_and_pins_models() {
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
        let owner = "e".repeat(64);
        let model_id = "00000000-0000-4000-8000-000000000009";
        assert!(
            db.insert_model(&StoredModel {
                id: model_id.into(),
                owner_hash: owner.clone(),
                user_id: None,
                filename: "job.stl".into(),
                extension: "stl".into(),
                source_bytes: 100,
                mesh_bytes: 100,
                sha256: "f".repeat(64),
                dimensions_mm: [10.; 3],
                expires_at: now + 1,
            })
            .await
            .unwrap()
        );
        let enqueue = |fingerprint: &'static str| {
            let db = &db;
            let owner = &owner;
            async move {
                db.enqueue_slice(
                    owner,
                    model_id,
                    fingerprint,
                    &json!({"material_id":"test"}),
                    now,
                )
                .await
                .unwrap()
            }
        };
        assert!(
            db.enqueue_slice(&"d".repeat(64), model_id, "wrong", &json!({}), now)
                .await
                .unwrap()
                .is_none()
        );
        let first = enqueue("one").await.unwrap();
        assert_eq!(first.id, enqueue("one").await.unwrap().id);
        assert!(
            db.expired_model_ids(now + 2)
                .await
                .unwrap()
                .iter()
                .all(|id| id != model_id)
        );
        let (a, b) = tokio::join!(db.claim_slice(now), db.claim_slice(now));
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!(usize::from(a.is_some()) + usize::from(b.is_some()), 1);
        let first = a.or(b).unwrap();
        assert_eq!(first.attempts, 1);
        assert!(db.claim_slice(now).await.unwrap().is_none());
        assert!(
            db.slice_job(&first.id, &"d".repeat(64), now)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !db.fail_slice(&first.id, "wrong", false, "failed", now)
                .await
                .unwrap()
        );
        assert!(db.claim_slice(now + 181).await.unwrap().is_none());
        let retried = db.claim_slice(now + 187).await.unwrap().unwrap();
        assert_eq!(retried.id, first.id);
        assert_eq!(retried.attempts, 2);
        assert!(
            !db.finish_slice(
                &first.id,
                first.lease_token.as_deref().unwrap(),
                &json!({}),
                &json!({}),
                now + 188
            )
            .await
            .unwrap()
        );
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES('queue-user','queue-user@example.com',$1)").bind(now).execute(&db.pool).await.unwrap();
        db.claim_guest("queue-user", &owner, now + 188)
            .await
            .unwrap();
        let account = account_owner("queue-user");
        let model = db
            .slice_model(
                &retried.id,
                retried.lease_token.as_deref().unwrap(),
                now + 188,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(model.owner_hash, account);
        assert_eq!(model.user_id.as_deref(), Some("queue-user"));
        assert!(
            db.slice_model(&retried.id, "wrong", now + 188)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.slice_model(
                &retried.id,
                retried.lease_token.as_deref().unwrap(),
                now + 400
            )
            .await
            .unwrap()
            .is_none()
        );
        assert!(
            db.slice_job(&first.id, &owner, now + 188)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.finish_slice(
                &retried.id,
                retried.lease_token.as_deref().unwrap(),
                &json!({"owner_hash":owner,"model_id":"bad","expires_at":now+1000}),
                &json!({"price":123}),
                now + 189
            )
            .await
            .unwrap()
        );
        let (request, result) = db
            .quote(&first.id, &account, now + 190)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(request["owner_hash"], account);
        assert_eq!(request["model_id"], model_id);
        assert_eq!(request["user_id"], "queue-user");
        assert_eq!(result["expires_at"], now + 789);
        assert!(
            !db.finish_slice(
                &retried.id,
                retried.lease_token.as_deref().unwrap(),
                &json!({}),
                &json!({}),
                now + 190
            )
            .await
            .unwrap()
        );
        assert_eq!(
            db.enqueue_slice(&account, model_id, "one", &json!({}), now + 190)
                .await
                .unwrap()
                .unwrap()
                .id,
            first.id
        );
        for fingerprint in ["two", "three", "four", "five"] {
            assert!(
                db.enqueue_slice(&account, model_id, fingerprint, &json!({}), now + 190)
                    .await
                    .unwrap()
                    .is_some()
            );
        }
        assert!(
            db.enqueue_slice(&account, model_id, "six", &json!({}), now + 190)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("INSERT INTO slicer_jobs(id,owner_hash,model_id,fingerprint,payload,created_at,available_at,expires_at) SELECT 'bound-'||n,$1,$2,'bound-'||n,'{}'::jsonb,$3,$3,$3+1800 FROM generate_series(1,60) n")
            .bind("b".repeat(64)).bind(model_id).bind(now).execute(&db.pool).await.unwrap();
        sqlx::query("UPDATE private_models SET owner_hash=$2 WHERE id=$1")
            .bind(model_id)
            .bind("d".repeat(64))
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            db.enqueue_slice(&"d".repeat(64), model_id, "global-bound", &json!({}), now)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("UPDATE private_models SET owner_hash=$2 WHERE id=$1")
            .bind(model_id)
            .bind(&account)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM slicer_jobs WHERE id LIKE 'bound-%'")
            .execute(&db.pool)
            .await
            .unwrap();
        let failed = db.claim_slice(now + 190).await.unwrap().unwrap();
        assert!(
            db.fail_slice(
                &failed.id,
                failed.lease_token.as_deref().unwrap(),
                true,
                "temporary",
                now + 191
            )
            .await
            .unwrap()
        );
        assert!(db.claim_slice(now + 191).await.unwrap().is_some());
        sqlx::query("UPDATE slicer_jobs SET status='failed',lease_token=NULL,lease_until=NULL WHERE model_id=$1").bind(model_id).execute(&db.pool).await.unwrap();
        assert!(
            db.expired_model_ids(now + 3000)
                .await
                .unwrap()
                .contains(&model_id.to_string())
        );
        // A failed filesystem delete leaves the row counted and unavailable to new work.
        assert!(db.model(model_id, &account, now).await.unwrap().is_none());
        assert!(
            db.enqueue_slice(&account, model_id, "deleted", &json!({}), now)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.expired_model_ids(now + 3000)
                .await
                .unwrap()
                .contains(&model_id.to_string())
        );
        db.forget_expired_model(model_id, now + 3000).await.unwrap();
        sqlx::query("DELETE FROM slicer_jobs WHERE model_id=$1")
            .bind(model_id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM quotes WHERE id=$1")
            .bind(&first.id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id='queue-user'")
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
