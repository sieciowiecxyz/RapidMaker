use crate::{Storage, StorageError, StoredModel, models::stored_model};
use serde_json::Value;

pub(crate) const COMPLETED_FILE_RETENTION: i64 = 90 * 86400;

impl Storage {
    /// Protect trusted quote models before contacting an external payment provider.
    pub async fn hold_order_files(
        &self,
        order: &str,
        owner: &str,
        quotes: &[String],
        now: i64,
    ) -> Result<bool, StorageError> {
        if uuid::Uuid::parse_str(order).is_err()
            || owner.len() != 64
            || !owner.bytes().all(|b| b.is_ascii_hexdigit())
            || quotes.is_empty()
            || quotes.len() > 20
        {
            return Ok(false);
        }
        let mut ids = quotes.to_vec();
        ids.sort();
        ids.dedup();
        if ids.len() != quotes.len() {
            return Ok(false);
        }
        let mut tx = self.queue_transaction().await?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT owner_hash FROM order_file_holds WHERE order_id=$1")
                .bind(order)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(existing) = existing {
            let held: Vec<String> = sqlx::query_scalar(
                "SELECT quote_id FROM order_files WHERE order_id=$1 ORDER BY quote_id",
            )
            .bind(order)
            .fetch_all(&mut *tx)
            .await?;
            return Ok(existing == owner && held == ids);
        }
        let recent: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM order_file_holds WHERE owner_hash=$1 AND created_at>$2-86400",
        )
        .bind(owner)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        if recent >= 30 {
            return Ok(false);
        }
        // Snapshot expiry and hash are server-produced; no client line metadata is trusted.
        let valid: Vec<(String, String, String)> = sqlx::query_as("SELECT q.id,m.id,m.filename FROM quotes q JOIN private_models m ON m.id=q.request->>'model_id' WHERE q.id=ANY($1) AND q.request->>'owner_hash'=$2 AND (q.request->>'expires_at')::bigint>$3 AND m.owner_hash=$2 AND m.expires_at>$3 AND NOT m.deletion_started AND q.request->>'model_sha256'=m.sha256")
            .bind(&ids).bind(owner).bind(now).fetch_all(&mut *tx).await?;
        if valid.len() != ids.len() {
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO order_file_holds(order_id,owner_hash,created_at) VALUES($1,$2,$3)",
        )
        .bind(order)
        .bind(owner)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        for (quote, model, filename) in valid {
            sqlx::query(
                "INSERT INTO order_files(order_id,quote_id,model_id,filename) VALUES($1,$2,$3,$4)",
            )
            .bind(order)
            .bind(quote)
            .bind(model)
            .bind(filename)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    /// Caller must authorize access to the order before exposing these records.
    pub async fn order_files(&self, order: &str, now: i64) -> Result<Vec<Value>, StorageError> {
        Ok(sqlx::query_scalar("SELECT jsonb_build_object('quote_id',f.quote_id,'model_id',f.model_id,'filename',f.filename,'preview_bytes',COALESCE(m.preview_bytes,0),'options',q.request->'options','material_id',q.request->'material_id','color_code',q.request->'color_code','quantity',q.request->'quantity','dimensions_mm',q.request->'dimensions_mm','profile_version',q.request->'slicer_profile_version','available',m.id IS NOT NULL AND NOT m.deletion_started AND (f.retain_until IS NULL OR f.retain_until>$2),'retain_until',f.retain_until) FROM order_files f JOIN quotes q ON q.id=f.quote_id LEFT JOIN private_models m ON m.id=f.model_id WHERE f.order_id=$1 ORDER BY f.quote_id")
            .bind(order).bind(now).fetch_all(&self.pool).await?)
    }

    /// Caller must authorize the order. This deliberately bypasses working-model expiry.
    pub async fn order_model(
        &self,
        order: &str,
        model: &str,
        now: i64,
    ) -> Result<Option<StoredModel>, StorageError> {
        let row = sqlx::query("SELECT m.* FROM private_models m WHERE m.id=$2 AND NOT m.deletion_started AND EXISTS(SELECT 1 FROM order_files f WHERE f.order_id=$1 AND f.model_id=m.id AND (f.retain_until IS NULL OR f.retain_until>$3))")
            .bind(order).bind(model).bind(now).fetch_optional(&self.pool).await?;
        row.map(stored_model).transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderInput, account_owner};
    use serde_json::json;

    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn order_file_holds_survive_expiry_and_release_only_after_all_orders_complete() {
        let db = Storage::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let owner = "e".repeat(64);
        let model = StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: owner.clone(),
            user_id: None,
            filename: "original.step".into(),
            extension: "step".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 100,
            sha256: "a".repeat(64),
            dimensions_mm: [20.0; 3],
            expires_at: 2000,
        };
        assert!(db.insert_model(&model).await.unwrap());
        for (id, expiry, hash) in [
            ("held", 2000, "a".repeat(64)),
            ("other", 2000, "a".repeat(64)),
            ("expired", 999, "a".repeat(64)),
            ("wrong-hash", 2000, "b".repeat(64)),
        ] {
            db.save_quote(id,&json!({"model_id":model.id,"owner_hash":owner,"expires_at":expiry,"model_sha256":hash,"options":{"quantity":2}}),&json!({})).await.unwrap();
        }
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        let held = vec!["held".into()];
        assert!(
            !db.hold_order_files(&first, &"f".repeat(64), &held, 1000)
                .await
                .unwrap()
        );
        assert!(
            !db.hold_order_files(&first, &owner, &["expired".into()], 1000)
                .await
                .unwrap()
        );
        assert!(
            !db.hold_order_files(&first, &owner, &["wrong-hash".into()], 1000)
                .await
                .unwrap()
        );
        sqlx::query("UPDATE private_models SET deletion_started=true WHERE id=$1")
            .bind(&model.id)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            !db.hold_order_files(&first, &owner, &held, 1000)
                .await
                .unwrap()
        );
        sqlx::query("UPDATE private_models SET deletion_started=false WHERE id=$1")
            .bind(&model.id)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            db.hold_order_files(&first, &owner, &held, 1000)
                .await
                .unwrap()
        );
        assert!(
            db.hold_order_files(&first, &owner, &held, 1000)
                .await
                .unwrap()
        );
        assert!(
            !db.hold_order_files(&first, &owner, &["other".into()], 1000)
                .await
                .unwrap()
        );
        assert!(
            !db.hold_order_files(&first, &owner, &["held".into(), "held".into()], 1000)
                .await
                .unwrap()
        );
        assert!(
            db.hold_order_files(&second, &owner, &held, 1000)
                .await
                .unwrap()
        );
        assert!(db.expired_model_ids(90000).await.unwrap().is_empty());
        assert!(
            db.order_model(&first, &model.id, 90000)
                .await
                .unwrap()
                .is_some()
        );
        assert!(db.model(&model.id, &owner, 90000).await.unwrap().is_none());
        // Claim both live models and their previously established payment holds.
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES('retention-user','retention@example.com',1000)").execute(&db.pool).await.unwrap();
        db.claim_guest("retention-user", &owner, 1001)
            .await
            .unwrap();
        let account = account_owner("retention-user");
        assert!(
            !db.hold_order_files(&first, &owner, &held, 1001)
                .await
                .unwrap()
        );
        assert!(
            db.hold_order_files(&first, &account, &held, 1001)
                .await
                .unwrap()
        );
        let contact = json!({"email":"retention@example.com"});
        for id in [&first, &second] {
            let provider = format!("pi_{}", id.replace('-', ""));
            let input = OrderInput {
                id,
                owner_hash: &account,
                user_id: Some("retention-user"),
                contact: &contact,
                lines: &json!([]),
                amount_grosze: 100,
                provider_kind: "intent",
                provider_id: &provider,
                guest_updates: true,
            };
            db.create_order(&input, 1002).await.unwrap();
            assert!(db.order_paid(id, &provider, 100, 1003).await.unwrap());
            assert!(db.order_status(id, "in_production", 1004).await.unwrap());
            assert!(db.order_status(id, "ready", 1005).await.unwrap());
        }
        assert!(db.order_status(&first, "completed", 1006).await.unwrap());
        let end = 1006 + COMPLETED_FILE_RETENTION;
        assert!(db.expired_model_ids(end).await.unwrap().is_empty());
        assert!(
            db.order_model(&first, &model.id, end)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.order_model(&second, &model.id, end)
                .await
                .unwrap()
                .is_some()
        );
        assert!(db.order_status(&second, "completed", 1007).await.unwrap());
        assert!(db.expired_model_ids(end).await.unwrap().is_empty());
        assert_eq!(
            db.expired_model_ids(end + 1).await.unwrap(),
            vec![model.id.clone()]
        );
        db.forget_expired_model(&model.id, end + 1).await.unwrap();
        assert!(
            db.order_model(&second, &model.id, end + 1)
                .await
                .unwrap()
                .is_none()
        );
        let metadata = db.order_files(&first, end + 1).await.unwrap();
        assert_eq!(metadata[0]["filename"], "original.step");
        assert_eq!(metadata[0]["options"]["quantity"], 2);
        assert_eq!(metadata[0]["available"], false);
        assert_eq!(metadata[0]["retain_until"], end);
    }
}
