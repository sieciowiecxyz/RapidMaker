//! Manual offers have their own immutable files and price, never a fabricated slicer result.
use crate::{
    PaymentAttempt, Storage, StorageError, account_owner, order_files::hold_files, orders::enqueue,
};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashSet;

fn digest(value: &str) -> String {
    rapidmaker_auth::hash_token(value)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
impl Storage {
    /// Idempotent per account. Each part is snapshotted from a live owned model under a row lock.
    pub async fn manual_request(
        &self,
        user: &str,
        request: &str,
        input: &Value,
        now: i64,
    ) -> Result<Option<String>, StorageError> {
        let Ok(request) = uuid::Uuid::parse_str(request).map(|v| v.to_string()) else {
            return Ok(None);
        };
        let Some(parts) = input["parts"]
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= 10)
        else {
            return Ok(None);
        };
        if input.to_string().len() > 65536 || input["contact"].to_string().len() > 8192 {
            return Ok(None);
        }
        let mut tx = self.queue_transaction().await?;
        if let Some(old) = sqlx::query(
            "SELECT order_id,input FROM manual_quotes WHERE user_id=$1 AND request_id=$2",
        )
        .bind(user)
        .bind(&request)
        .fetch_optional(&mut *tx)
        .await?
        {
            return Ok((old.try_get::<Value, _>("input")? == *input).then(|| old.get("order_id")));
        }
        let Some(email): Option<String> = sqlx::query_scalar(
            "SELECT email FROM users WHERE id=$1 AND disabled_at IS NULL FOR SHARE",
        )
        .bind(user)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(None);
        };
        if input["contact"]["email"] != email {
            return Ok(None);
        }
        let recent:i64=sqlx::query_scalar("SELECT count(*) FROM manual_quotes m JOIN orders o ON o.id=m.order_id WHERE m.user_id=$1 AND o.created_at>$2-86400").bind(user).bind(now).fetch_one(&mut *tx).await?;
        let capacity:bool=sqlx::query_scalar("SELECT count(*)<10000 FROM orders o JOIN manual_quotes m ON m.order_id=o.id WHERE o.payment_attempt_id IS NULL AND o.status IN ('awaiting_quote','awaiting_payment')").fetch_one(&mut *tx).await?;
        if recent >= 30 || !capacity {
            return Ok(None);
        }
        let owner = account_owner(user);
        let id = uuid::Uuid::new_v4().to_string();
        let until = now + 14 * 86400;
        let mut ids = HashSet::new();
        let mut quotes = Vec::new();
        let mut lines = Vec::new();
        for part in parts {
            let Some(model_id) = part["model_id"]
                .as_str()
                .filter(|v| uuid::Uuid::parse_str(v).is_ok())
            else {
                return Ok(None);
            };
            let Some(quantity) = part["quantity"].as_u64().filter(|v| (1..=1000).contains(v))
            else {
                return Ok(None);
            };
            if !ids.insert(model_id) {
                return Ok(None);
            }
            let Some(model)=sqlx::query("SELECT * FROM private_models WHERE id=$1 AND user_id=$2 AND owner_hash=$3 AND expires_at>$4 AND NOT deletion_started FOR UPDATE").bind(model_id).bind(user).bind(&owner).bind(now).fetch_optional(&mut *tx).await? else{return Ok(None);};
            let q = uuid::Uuid::new_v4().to_string();
            let filename: String = model.try_get("filename")?;
            let dimensions: Vec<f64> = model.try_get("dimensions_mm")?;
            let snapshot = json!({"kind":"manual","owner_hash":owner,"user_id":user,"model_id":model_id,"model_sha256":model.try_get::<String,_>("sha256")?,"expires_at":until,"dimensions_mm":dimensions,"quantity":quantity,"material_id":part["material_id"],"color_code":part["color_code"],"options":part["options"]});
            sqlx::query("INSERT INTO quotes(id,request,result,user_id) VALUES($1,$2,$3,$4)")
                .bind(&q)
                .bind(snapshot)
                .bind(json!({"kind":"manual","payable":false}))
                .bind(user)
                .execute(&mut *tx)
                .await?;
            lines.push(
                json!({"model_id":model_id,"quote_id":q,"filename":filename,"quantity":quantity}),
            );
            quotes.push(q);
        }
        if !hold_files(&mut tx, &id, &owner, &quotes, now).await? {
            return Ok(None);
        }
        sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
            .bind(&id)
            .bind(until)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO orders(id,owner_hash,user_id,contact,lines,amount_grosze,provider_kind,status,guest_updates,created_at,fulfillment) VALUES($1,$2,$3,$4,$5,NULL,'intent','awaiting_quote',true,$6,$7)")
            .bind(&id).bind(&owner).bind(user).bind(&input["contact"]).bind(json!(lines)).bind(now).bind(json!({"kind":"manual","delivery":input["delivery"],"notes":input["notes"]})).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO manual_quotes(order_id,user_id,request_id,input,expires_at) VALUES($1,$2,$3,$4,$5)").bind(&id).bind(user).bind(request).bind(input).bind(until).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(id))
    }
    /// Publishing and the notification are one transaction. No edits after any payment attempt.
    #[allow(clippy::too_many_arguments)]
    pub async fn manual_publish(
        &self,
        id: &str,
        actor: &str,
        version: i64,
        amount: i64,
        shipping: i64,
        scope: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        if version < 1
            || version == i64::MAX
            || !(1..=99999999).contains(&amount)
            || !(0..=amount).contains(&shipping)
            || scope.trim().is_empty()
            || scope.len() > 4000
        {
            return Ok(false);
        }
        let mut tx = self.queue_transaction().await?;
        let admin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND role='admin' AND disabled_at IS NULL)").bind(actor).fetch_one(&mut *tx).await?;
        if !admin {
            return Ok(false);
        }
        let Some(row)=sqlx::query("SELECT m.*,o.status,o.payment_attempt_id,o.contact,o.fulfillment FROM manual_quotes m JOIN orders o ON o.id=m.order_id WHERE m.order_id=$1 FOR UPDATE OF m,o").bind(id).fetch_optional(&mut *tx).await? else{return Ok(false);};
        if row
            .try_get::<Option<String>, _>("payment_attempt_id")?
            .is_some()
            || row.try_get::<i64, _>("expires_at")? <= now
            || !matches!(
                row.try_get::<String, _>("status")?.as_str(),
                "awaiting_quote" | "awaiting_payment"
            )
        {
            return Ok(false);
        }
        let offer = json!({"amount_grosze":amount,"shipping_grosze":shipping,"scope":scope});
        let current: i64 = row.try_get("version")?;
        if current == version + 1
            && row.try_get::<Option<Value>, _>("offer")?.as_ref() == Some(&offer)
        {
            tx.commit().await?;
            return Ok(true);
        }
        if current != version {
            return Ok(false);
        }
        let contact: Value = row.try_get("contact")?;
        let mut fulfillment: Value = row.try_get("fulfillment")?;
        if fulfillment["delivery"]["method"] == "pickup" && shipping != 0 {
            return Ok(false);
        }
        fulfillment["manual_scope"] = json!(scope);
        fulfillment["manual_shipping_grosze"] = json!(shipping);
        let until = now + 7 * 86400;
        sqlx::query(
            "UPDATE manual_quotes SET version=version+1,offer=$2,expires_at=$3 WHERE order_id=$1",
        )
        .bind(id)
        .bind(offer)
        .bind(until)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE orders SET amount_grosze=$2,status='awaiting_payment',fulfillment=$3 WHERE id=$1").bind(id).bind(amount).bind(fulfillment).execute(&mut *tx).await?;
        sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
            .bind(id)
            .bind(until)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE email_outbox SET status='skipped',finished_at=$2 WHERE kind='manual_offer' AND payload->'notification'->>'reference'=$1 AND status='pending'").bind(id).bind(now).execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            id,
            &format!("manual-offer:{}", current + 1),
            Some(&row.try_get::<String, _>("user_id")?),
            contact["email"].as_str().ok_or("invalid manual contact")?,
            "manual_offer",
            json!({"reference":id,"amount_grosze":amount,"valid_until":until}),
            true,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
    /// One attempt for this order's lifetime. Closed/uncertain attempts cannot be replaced or repriced.
    pub async fn manual_payment(
        &self,
        id: &str,
        user: &str,
        version: i64,
        request: &str,
        now: i64,
    ) -> Result<Option<PaymentAttempt>, StorageError> {
        if uuid::Uuid::parse_str(request).is_err() {
            return Ok(None);
        }
        let owner = account_owner(user);
        let mut tx = self.queue_transaction().await?;
        let Some(row)=sqlx::query("SELECT m.*,o.payment_attempt_id,o.status,o.amount_grosze FROM manual_quotes m JOIN orders o ON o.id=m.order_id WHERE m.order_id=$1 AND m.user_id=$2 FOR UPDATE OF m,o").bind(id).bind(user).fetch_optional(&mut *tx).await? else{return Ok(None);};
        if row.try_get::<i64, _>("version")? != version {
            return Ok(None);
        }
        if row
            .try_get::<Option<String>, _>("payment_attempt_id")?
            .is_some()
        {
            let a = sqlx::query("SELECT * FROM payment_attempts WHERE id=$1 AND owner_hash=$2")
                .bind(id)
                .bind(&owner)
                .fetch_one(&mut *tx)
                .await?;
            return Ok(Some(crate::checkout::attempt(a)?));
        }
        if row.try_get::<i64, _>("expires_at")? <= now
            || row.try_get::<Option<Value>, _>("offer")?.is_none()
            || row.try_get::<String, _>("status")? != "awaiting_payment"
        {
            return Ok(None);
        }
        let key = digest(&format!("manual:{request}"));
        let reused:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM payment_attempts WHERE owner_hash=$1 AND client_key_hash=$2)").bind(&owner).bind(&key).fetch_one(&mut *tx).await?;
        let capacity:bool=sqlx::query_scalar("SELECT count(*)<10000 FROM payment_attempts WHERE state IN ('creating','unknown','open','processing','manual_review')").fetch_one(&mut *tx).await?;
        if reused || !capacity {
            return Ok(None);
        }
        let files=sqlx::query("SELECT m.id FROM order_files f JOIN private_models m ON m.id=f.model_id WHERE f.order_id=$1 AND (f.retain_until IS NULL OR f.retain_until>$2) AND NOT m.deletion_started FOR UPDATE OF m").bind(id).bind(now).fetch_all(&mut *tx).await?;
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM order_files WHERE order_id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if count == 0 || files.len() != count as usize {
            return Ok(None);
        }
        let amount: i64 = row.try_get("amount_grosze")?;
        let form = json!([
            ["amount", amount.to_string()],
            ["currency", "pln"],
            ["metadata[quote_id]", id]
        ]);
        sqlx::query("INSERT INTO checkout_drafts(id,owner_hash,user_id,input,expires_at,locked,created_at) VALUES($1,$2,$3,$4,$5,true,$6)").bind(id).bind(&owner).bind(user).bind(json!({"kind":"manual","order_id":id})).bind(now+86400).bind(now).execute(&mut *tx).await?;
        let attempt=sqlx::query("INSERT INTO payment_attempts(id,draft_id,owner_hash,client_key_hash,fingerprint,state,provider_kind,amount_grosze,idempotency_key,provider_request,next_attempt,created_at,updated_at) VALUES($1,$1,$2,$3,$4,'creating','intent',$5,$6,$7,$8,$8,$8) RETURNING *")
            .bind(id).bind(&owner).bind(key).bind(digest(&format!("manual:{id}:{version}:{amount}"))).bind(amount).bind(format!("rapidmaker-manual:{id}")).bind(form).bind(now).fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE orders SET payment_attempt_id=$1 WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE order_files SET retain_until=NULL WHERE order_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(crate::checkout::attempt(attempt)?))
    }
    /// Internal reader; HTTP callers must enforce account ownership or administrator access.
    pub async fn manual_offer(&self, id: &str) -> Result<Option<Value>, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT offer FROM manual_quote_views WHERE order_id=$1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
    pub async fn manual_expire(&self, now: i64) -> Result<u64, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let ids:Vec<String>=sqlx::query_scalar("UPDATE orders SET status='quote_expired' WHERE id IN (SELECT o.id FROM orders o JOIN manual_quotes m ON m.order_id=o.id WHERE m.expires_at<=$1 AND o.payment_attempt_id IS NULL AND o.status IN ('awaiting_quote','awaiting_payment') ORDER BY m.expires_at LIMIT 100) RETURNING id").bind(now).fetch_all(&mut *tx).await?;
        if !ids.is_empty() {
            sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=ANY($1)")
                .bind(&ids)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE email_outbox SET status='skipped',finished_at=$2 WHERE kind='manual_offer' AND payload->'notification'->>'reference'=ANY($1) AND status='pending'").bind(&ids).bind(now).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(ids.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoredModel;
    #[tokio::test]
    #[ignore = "requires isolated TEST_DATABASE_URL"]
    async fn manual_offers_are_owned_atomic_versioned_and_keep_files_until_resolved() {
        let url = std::env::var("TEST_DATABASE_URL").expect("isolated test DB");
        let db = Storage::connect(&url).await.unwrap();
        assert!(
            sqlx::query_scalar::<_, bool>("SELECT current_database() LIKE 'rapidmaker_test%'")
                .fetch_one(&db.pool)
                .await
                .unwrap()
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let user = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let admin = uuid::Uuid::new_v4().to_string();
        let email = format!("{user}@example.invalid");
        for (id, role) in [(&user, "client"), (&other, "client"), (&admin, "admin")] {
            sqlx::query("INSERT INTO users(id,email,role,created_at) VALUES($1,$2,$3,$4)")
                .bind(id)
                .bind(format!("{id}@example.invalid"))
                .bind(role)
                .bind(now)
                .execute(&db.pool)
                .await
                .unwrap();
        }
        let model = StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: account_owner(&user),
            user_id: Some(user.clone()),
            filename: "large.stl".into(),
            extension: "stl".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 100,
            sha256: "a".repeat(64),
            dimensions_mm: [1000.0, 20.0, 10.0],
            expires_at: now + 86400,
        };
        assert!(db.insert_model(&model).await.unwrap());
        let input = json!({"parts":[{"model_id":model.id,"quantity":5,"material_id":"GFA00","color_code":"10100","options":{"quality":"standard","wall_loops":2,"infill_percent":15}}],"contact":{"email":email,"name":"Test Client"},"delivery":{"method":"courier"},"notes":"Cut model into smaller parts"});
        let request = uuid::Uuid::new_v4().to_string();
        let id = db
            .manual_request(&user, &request, &input, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            db.manual_request(&user, &request, &input, now + 1)
                .await
                .unwrap()
                .as_deref(),
            Some(id.as_str())
        );
        let mut changed = input.clone();
        changed["notes"] = json!("Different request");
        assert!(
            db.manual_request(&user, &request, &changed, now + 1)
                .await
                .unwrap()
                .is_none()
        );
        changed = input.clone();
        changed["contact"]["email"] = json!(format!("{other}@example.invalid"));
        assert!(
            db.manual_request(&other, &request, &changed, now + 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(db.order(&id, Some(&other), None).await.unwrap().is_none());
        let original = db.order(&id, Some(&user), None).await.unwrap().unwrap();
        assert!(original["amount_grosze"].is_null());
        assert_eq!(original["status"], "awaiting_quote");
        assert_eq!(original["manual_offer"]["version"], 1);
        assert_eq!(original["manual_offer"]["state"], "awaiting_quote");
        assert_eq!(original["manual_offer"]["payment_started"], false);
        let held: Option<i64> =
            sqlx::query_scalar("SELECT retain_until FROM order_files WHERE order_id=$1")
                .bind(&id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(held, Some(now + 14 * 86400));
        assert!(
            !db.manual_publish(&id, &user, 1, 12345, 1000, "5 parts", now + 2)
                .await
                .unwrap()
        );
        assert!(
            !db.manual_publish(&id, &admin, 1, 12345, 20000, "5 parts", now + 2)
                .await
                .unwrap()
        );
        assert!(
            db.manual_publish(&id, &admin, 1, 12345, 1000, "5 parts", now + 2)
                .await
                .unwrap()
        );
        assert!(
            db.manual_publish(&id, &admin, 1, 12345, 1000, "5 parts", now + 3)
                .await
                .unwrap()
        );
        let one: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_outbox WHERE kind='manual_offer' AND user_id=$1",
        )
        .bind(&user)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(one, 1);
        assert!(
            !db.manual_publish(&id, &admin, 1, 23456, 1000, "Updated", now + 3)
                .await
                .unwrap()
        );
        assert!(
            db.manual_publish(&id, &admin, 2, 23456, 1000, "Updated", now + 3)
                .await
                .unwrap()
        );
        let offered = db.manual_offer(&id).await.unwrap().unwrap();
        assert_eq!(offered["version"], 3);
        assert_eq!(offered["amount_grosze"], 23456);
        assert_eq!(offered["valid_until"], now + 3 + 7 * 86400);
        assert!(
            db.manual_payment(&id, &other, 3, &request, now + 4)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.manual_payment(&id, &user, 2, &request, now + 4)
                .await
                .unwrap()
                .is_none()
        );
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        let (a, b) = tokio::join!(
            db.manual_payment(&id, &user, 3, &a, now + 4),
            db.manual_payment(&id, &user, 3, &b, now + 4)
        );
        assert_eq!(a.unwrap().unwrap().id, id);
        assert_eq!(b.unwrap().unwrap().id, id);
        let attempts: i64 = sqlx::query_scalar("SELECT count(*) FROM payment_attempts WHERE id=$1")
            .bind(&id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(attempts, 1);
        assert!(
            !db.manual_publish(&id, &admin, 3, 1, 0, "Cannot reprice", now + 5)
                .await
                .unwrap()
        );
        db.manual_expire(now + 30 * 86400).await.unwrap();
        let hold: Option<i64> =
            sqlx::query_scalar("SELECT retain_until FROM order_files WHERE order_id=$1")
                .bind(&id)
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(hold, None);
        let pending = db.manual_offer(&id).await.unwrap().unwrap();
        assert_eq!(pending["state"], "payment_started");
        assert_eq!(pending["payment_started"], true);
        let mut paid = json!({"id":"pi_manual_test","metadata":{"quote_id":id},"amount":23456,"amount_received":23456,"currency":"pln","livemode":false,"status":"succeeded"});
        for field in ["amount", "currency", "livemode", "metadata"] {
            let mut bad = paid.clone();
            bad[field] = match field {
                "amount" => json!(23455),
                "currency" => json!("eur"),
                "livemode" => json!(true),
                _ => json!({"quote_id":"other"}),
            };
            assert!(
                !db.checkout_resolve(&id, "intent", &bad, now + 6)
                    .await
                    .unwrap()
            );
        }
        assert!(
            db.checkout_resolve(&id, "intent", &paid, now + 7)
                .await
                .unwrap()
        );
        assert!(
            db.checkout_resolve(&id, "intent", &paid, now + 8)
                .await
                .unwrap()
        );
        assert_eq!(
            db.manual_offer(&id).await.unwrap().unwrap()["state"],
            "paid"
        );
        let receipts: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM email_outbox WHERE kind='payment_received' AND user_id=$1",
        )
        .bind(&user)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(receipts, 1);
        paid["status"] = json!("canceled");
        assert!(
            db.checkout_resolve(&id, "intent", &paid, now + 9)
                .await
                .unwrap()
        );
        assert_eq!(
            db.manual_offer(&id).await.unwrap().unwrap()["state"],
            "paid"
        );
        // A separate abandoned request expires in 14 days; it cannot release shared paid files.
        let abandoned = db
            .manual_request(&user, &uuid::Uuid::new_v4().to_string(), &input, now + 10)
            .await
            .unwrap()
            .unwrap();
        db.manual_expire(now + 15 * 86400).await.unwrap();
        assert_eq!(
            db.manual_offer(&abandoned).await.unwrap().unwrap()["state"],
            "expired"
        );
        assert!(
            db.expired_model_ids(now + 15 * 86400)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            !db.manual_publish(&abandoned, &admin, 1, 10000, 0, "Expired", now + 15 * 86400)
                .await
                .unwrap()
        );
        // Published but unpaid offers release their own holds after exactly seven days.
        let expiry = db
            .manual_request(&user, &uuid::Uuid::new_v4().to_string(), &input, now + 11)
            .await
            .unwrap()
            .unwrap();
        assert!(
            db.manual_publish(&expiry, &admin, 1, 10000, 0, "Unpaid", now + 11)
                .await
                .unwrap()
        );
        db.manual_expire(now + 11 + 7 * 86400).await.unwrap();
        assert_eq!(
            db.manual_offer(&expiry).await.unwrap().unwrap()["state"],
            "expired"
        );
        assert!(
            db.manual_payment(&expiry, &user, 2, &request, now + 11 + 7 * 86400)
                .await
                .unwrap()
                .is_none()
        );
        // Explicit terminal failure keeps the old immutable attempt; it never creates another.
        let failed = db
            .manual_request(&user, &uuid::Uuid::new_v4().to_string(), &input, now + 12)
            .await
            .unwrap()
            .unwrap();
        db.manual_publish(&failed, &admin, 1, 2000, 0, "Will be cancelled", now + 12)
            .await
            .unwrap();
        db.manual_payment(&failed, &user, 2, &request, now + 13)
            .await
            .unwrap()
            .unwrap();
        let closed = json!({"id":"pi_manual_cancelled","metadata":{"quote_id":failed},"amount":2000,"currency":"pln","livemode":false,"status":"canceled"});
        assert!(
            db.checkout_resolve(&failed, "intent", &closed, now + 14)
                .await
                .unwrap()
        );
        let replay = db
            .manual_payment(
                &failed,
                &user,
                2,
                &uuid::Uuid::new_v4().to_string(),
                now + 15,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(replay.state, "expired");
        assert_eq!(
            db.manual_offer(&failed).await.unwrap().unwrap()["payment_started"],
            true
        );
        assert!(
            !db.manual_publish(
                &failed,
                &admin,
                2,
                3000,
                0,
                "Cannot retry with new amount",
                now + 15
            )
            .await
            .unwrap()
        );
        // Cleanup only this test's records, leaving no first-admin pollution for other suites.
        let order_ids: Vec<String> = sqlx::query_scalar("SELECT id FROM orders WHERE user_id=$1")
            .bind(&user)
            .fetch_all(&db.pool)
            .await
            .unwrap();
        let quote_ids: Vec<String> =
            sqlx::query_scalar("SELECT quote_id FROM order_files WHERE order_id=ANY($1)")
                .bind(&order_ids)
                .fetch_all(&db.pool)
                .await
                .unwrap();
        sqlx::query("DELETE FROM email_outbox WHERE user_id=$1")
            .bind(&user)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM manual_quotes WHERE user_id=$1")
            .bind(&user)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM orders WHERE user_id=$1")
            .bind(&user)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM order_files WHERE order_id=ANY($1)")
            .bind(&order_ids)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM order_file_holds WHERE order_id=ANY($1)")
            .bind(&order_ids)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM payment_attempts WHERE id=ANY($1)")
            .bind(&order_ids)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM checkout_drafts WHERE id=ANY($1)")
            .bind(&order_ids)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM quotes WHERE id=ANY($1)")
            .bind(&quote_ids)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM private_models WHERE id=$1")
            .bind(&model.id)
            .execute(&db.pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM users WHERE id=ANY($1)")
            .bind(vec![user, other, admin])
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
