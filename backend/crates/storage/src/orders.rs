//! Business writes and email enqueue share a PostgreSQL transaction.
use crate::email::EmailInput;
use crate::{Storage, StorageError};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};

pub struct OrderInput<'a> {
    pub id: &'a str,
    pub owner_hash: &'a str,
    pub user_id: Option<&'a str>,
    pub contact: &'a Value,
    pub lines: &'a Value,
    pub amount_grosze: i64,
    pub provider_kind: &'a str,
    pub provider_id: &'a str,
    pub guest_updates: bool,
}
impl Storage {
    pub async fn poll_order_payment(&self, id: &str, now: i64) -> Result<bool, StorageError> {
        Ok(sqlx::query("UPDATE orders SET payment_checked_at=$2 WHERE id=$1 AND status='awaiting_payment' AND payment_checked_at<=$2-15")
            .bind(id).bind(now).execute(&self.pool).await?.rows_affected()==1)
    }
    pub async fn create_order(&self, o: &OrderInput<'_>, now: i64) -> Result<(), StorageError> {
        if uuid::Uuid::parse_str(o.id).is_err()
            || o.provider_id.len() > 128
            || !o
                .provider_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid order identifier".into());
        }
        let recipient = o.contact["email"]
            .as_str()
            .filter(|s| s.len() <= 254 && s.contains('@') && !s.chars().any(char::is_control))
            .ok_or("invalid order email")?;
        let mut tx = self.queue_transaction().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(72617069801)")
            .execute(&mut *tx)
            .await?;
        let held_owner: Option<String> =
            sqlx::query_scalar("SELECT owner_hash FROM order_file_holds WHERE order_id=$1")
                .bind(o.id)
                .fetch_optional(&mut *tx)
                .await?;
        if held_owner.as_deref() != Some(o.owner_hash) {
            return Err("order files unavailable or owner changed".into());
        }
        let existing = sqlx::query(
            "SELECT owner_hash,amount_grosze,provider_id FROM orders WHERE id=$1 FOR UPDATE",
        )
        .bind(o.id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = existing {
            if row.get::<String, _>("owner_hash") != o.owner_hash
                || row.get::<i64, _>("amount_grosze") != o.amount_grosze
                || row.get::<Option<String>, _>("provider_id").as_deref() != Some(o.provider_id)
            {
                return Err("order changed".into());
            }
            tx.commit().await?;
            return Ok(());
        }
        let recent: i64 =
            sqlx::query_scalar("SELECT count(*) FROM orders WHERE owner_hash=$1 AND created_at>$2")
                .bind(o.owner_hash)
                .bind(now - 86400)
                .fetch_one(&mut *tx)
                .await?;
        if recent >= 30 {
            return Err("order limit".into());
        }
        sqlx::query("INSERT INTO orders(id,owner_hash,user_id,contact,lines,amount_grosze,provider_kind,provider_id,guest_updates,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(o.id).bind(o.owner_hash).bind(o.user_id).bind(o.contact).bind(o.lines).bind(o.amount_grosze).bind(o.provider_kind).bind(o.provider_id).bind(o.guest_updates).bind(now).execute(&mut *tx).await?;
        enqueue(
            &mut tx,
            o.id,
            "confirmed",
            o.user_id,
            recipient,
            "order_confirmed",
            json!({"reference":o.id}),
            true,
            now,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn orders(
        &self,
        user: Option<&str>,
        owner: Option<&str>,
    ) -> Result<Vec<Value>, StorageError> {
        self.manual_expire(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
        )
        .await?;
        let rows=sqlx::query_scalar("SELECT jsonb_build_object('id',o.id,'status',o.status,'payment_state',p.state,'amount_grosze',o.amount_grosze,'created_at',o.created_at,'paid_at',o.paid_at,'manual_offer',mq.offer) FROM orders o LEFT JOIN payment_attempts p ON p.id=o.payment_attempt_id LEFT JOIN manual_quote_views mq ON mq.order_id=o.id WHERE o.user_id=$1 OR (o.user_id IS NULL AND o.owner_hash=$2 AND o.created_at>extract(epoch from now())::bigint-86400) ORDER BY o.created_at DESC LIMIT 100")
            .bind(user).bind(owner).fetch_all(&self.pool).await?;
        Ok(rows)
    }
    pub async fn order(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
    ) -> Result<Option<Value>, StorageError> {
        self.manual_expire(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
        )
        .await?;
        Ok(sqlx::query_scalar("SELECT (to_jsonb(o)-'owner_hash'-'user_id')||jsonb_build_object('payment_state',p.state,'manual_offer',mq.offer) FROM orders o LEFT JOIN payment_attempts p ON p.id=o.payment_attempt_id LEFT JOIN manual_quote_views mq ON mq.order_id=o.id WHERE o.id=$1 AND (o.user_id=$2 OR (o.user_id IS NULL AND o.owner_hash=$3 AND o.created_at>extract(epoch from now())::bigint-86400))")
            .bind(id).bind(user).bind(owner).fetch_optional(&self.pool).await?)
    }
    pub async fn admin_orders(&self) -> Result<Vec<Value>, StorageError> {
        self.manual_expire(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
        )
        .await?;
        Ok(sqlx::query_scalar(
            "SELECT (to_jsonb(o)-'owner_hash')||jsonb_build_object('payment_state',p.state,'manual_offer',mq.offer) FROM orders o LEFT JOIN payment_attempts p ON p.id=o.payment_attempt_id LEFT JOIN manual_quote_views mq ON mq.order_id=o.id ORDER BY o.created_at DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?)
    }
    pub async fn order_paid(
        &self,
        id: &str,
        provider: &str,
        amount: i64,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.pool.begin().await?;
        let paid = mark_paid(&mut tx, id, provider, amount, now).await?;
        tx.commit().await?;
        if paid {
            tracing::info!(event = "order_paid", order_id = id);
        }
        Ok(paid)
    }

    pub async fn order_status(
        &self,
        id: &str,
        status: &str,
        now: i64,
    ) -> Result<bool, StorageError> {
        let mut tx = self.queue_transaction().await?;
        let row = sqlx::query(
            "SELECT status,user_id,contact,guest_updates,fulfillment FROM orders WHERE id=$1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else { return Ok(false) };
        let old: String = row.try_get("status")?;
        let allowed = matches!(
            (old.as_str(), status),
            ("paid", "in_production") | ("in_production", "ready") | ("ready", "completed")
        );
        if !allowed
            || (status == "completed" && {
                let f = row
                    .try_get::<Option<Value>, _>("fulfillment")?
                    .unwrap_or(Value::Null);
                f["delivery"]["method"]
                    .as_str()
                    .or_else(|| f["delivery"].as_str())
                    != Some("pickup")
            })
        {
            return Ok(false);
        }
        let user: Option<String> = row.try_get("user_id")?;
        let contact: Value = row.try_get("contact")?;
        sqlx::query("UPDATE orders SET status=$2 WHERE id=$1")
            .bind(id)
            .bind(status)
            .execute(&mut *tx)
            .await?;
        if status == "completed" {
            let until = now
                .checked_add(crate::order_files::COMPLETED_FILE_RETENTION)
                .ok_or("retention overflow")?;
            sqlx::query("UPDATE order_files SET retain_until=$2 WHERE order_id=$1")
                .bind(id)
                .bind(until)
                .execute(&mut *tx)
                .await?;
        }
        enqueue(
            &mut tx,
            id,
            status,
            user.as_deref(),
            contact["email"].as_str().ok_or("invalid order contact")?,
            "status_changed",
            json!({"reference":id,"status":status}),
            row.try_get("guest_updates")?,
            now,
        )
        .await?;
        tx.commit().await?;
        tracing::info!(event="order_status_changed", order_id=id, from=%old, to=status);
        Ok(true)
    }
    pub async fn order_messages(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
    ) -> Result<Option<Vec<Value>>, StorageError> {
        Ok(self
            .chat_messages(id, user, owner, operator, 0, 100)
            .await?
            .map(|p| p.messages))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn order_message(
        &self,
        id: &str,
        user: Option<&str>,
        owner: Option<&str>,
        operator: bool,
        body: &str,
        admin_to: Option<&str>,
        now: i64,
    ) -> Result<bool, StorageError> {
        self.chat_web_message(id, user, owner, operator, body, admin_to, now)
            .await
    }
}

pub(crate) async fn mark_paid(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    provider: &str,
    amount: i64,
    now: i64,
) -> Result<bool, StorageError> {
    let row=sqlx::query("SELECT user_id,contact,lines,fulfillment,legal_version,production_days FROM orders WHERE id=$1 AND provider_id=$2 AND amount_grosze=$3 AND status='awaiting_payment' FOR UPDATE")
            .bind(id).bind(provider).bind(amount).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let user: Option<String> = row.try_get("user_id")?;
    let contact: Value = row.try_get("contact")?;
    sqlx::query("UPDATE orders SET status='paid',paid_at=$2 WHERE id=$1")
        .bind(id)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    enqueue(
        tx,
        id,
        "paid",
        user.as_deref(),
        contact["email"].as_str().ok_or("invalid order contact")?,
        "payment_received",
        json!({"reference":id,"amount_grosze":amount}),
        true,
        now,
    )
    .await?;
    if let Some(version) = row.try_get::<Option<String>, _>("legal_version")? {
        let parts: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('plik',f.filename,'liczba_sztuk',q.request->'quantity','material',q.request->'material_id','kolor',q.request->'color_code','parametry_druku',q.request->'options','wymiary_mm',q.request->'dimensions_mm') FROM order_files f JOIN quotes q ON q.id=f.quote_id WHERE f.order_id=$1 ORDER BY f.quote_id")
            .bind(id).fetch_all(&mut **tx).await?;
        let fulfillment = row
            .try_get::<Option<Value>, _>("fulfillment")?
            .unwrap_or(Value::Null);
        // Confirm the customer-facing terms, not the potentially large packing/placement plan.
        let delivery = json!({"metoda":fulfillment["delivery"],"przewoznik":fulfillment["offer"]["name"],"koszt_grosze":fulfillment["offer"]["shipping_grosze"],"opakowania_grosze":fulfillment["packaging_grosze"],"punkt_odbioru":fulfillment["point"],"uzgodnienia":fulfillment["manual_scope"],"dostawa_reczna_grosze":fulfillment["manual_shipping_grosze"]});
        let snapshot = json!({"odbiorca":contact,"czesci":parts,"pozycje_cenowe":row.try_get::<Value,_>("lines")?,"dostawa_i_uzgodnienia":delivery});
        let details = format!(
            "Potwierdzamy zawarcie umowy i płatność {}.{:02} PLN brutto za zamówienie {id}.\nWykonanie i nadanie / gotowość do odbioru: do {} dni kalendarzowych od potwierdzenia płatności, chyba że w ofercie uzgodniono inny termin. Czas przewozu osobno.\nSpecyfikacja i rozliczenie (kwoty pól *_grosze podano w groszach):\n{}",
            amount / 100,
            amount % 100,
            row.try_get::<Option<i32>, _>("production_days")?
                .ok_or("missing production term")?,
            serde_json::to_string_pretty(&snapshot)?
        );
        enqueue(
            tx,
            id,
            "contract",
            user.as_deref(),
            contact["email"].as_str().ok_or("invalid order contact")?,
            "legal_documents",
            json!({"reference":id,"legal_version":version,"details":details}),
            true,
            now,
        )
        .await?;
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn enqueue(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    event: &str,
    user: Option<&str>,
    recipient: &str,
    kind: &str,
    fields: Value,
    enabled: bool,
    now: i64,
) -> Result<(), StorageError> {
    let mut notification = fields;
    notification["kind"] = json!(kind);
    super::email::enqueue(
        tx,
        &EmailInput {
            id: uuid::Uuid::new_v4().to_string(),
            event_key: format!("order:{id}:{event}"),
            user_id: user.map(str::to_owned),
            kind: kind.into(),
            recipient: recipient.into(),
            payload: json!({"version":1,"notification":notification}),
            optional_enabled: enabled,
        },
        now,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires an isolated TEST_DATABASE_URL"]
    async fn order_events_are_owned_atomic_and_respect_preferences() {
        let url = std::env::var("TEST_DATABASE_URL").unwrap();
        assert!(
            url.rsplit('/')
                .next()
                .unwrap()
                .starts_with("rapidmaker_test")
        );
        let db = Storage::connect(&url).await.unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let user = uuid::Uuid::new_v4().to_string();
        let owner = "ab".repeat(32);
        let contact = json!({"email":"order@example.invalid"});
        sqlx::query("INSERT INTO users(id,email,created_at) VALUES($1,$2,1000)")
            .bind(&user)
            .bind(format!("{user}@example.invalid"))
            .execute(&db.pool)
            .await
            .unwrap();
        let o = OrderInput {
            id: &id,
            owner_hash: &owner,
            user_id: Some(&user),
            contact: &contact,
            lines: &json!([]),
            amount_grosze: 1234,
            provider_kind: "intent",
            provider_id: &format!("pi_{}", id.replace('-', "")),
            guest_updates: true,
        };
        assert!(db.create_order(&o, 1000).await.is_err());
        let model = crate::StoredModel {
            id: uuid::Uuid::new_v4().to_string(),
            owner_hash: owner.clone(),
            user_id: Some(user.clone()),
            filename: "order.stl".into(),
            extension: "stl".into(),
            source_bytes: 100,
            mesh_bytes: 100,
            preview_bytes: 0,
            sha256: "a".repeat(64),
            dimensions_mm: [20.0; 3],
            expires_at: 2000,
        };
        assert!(db.insert_model(&model).await.unwrap());
        let quote = uuid::Uuid::new_v4().to_string();
        db.save_quote(&quote, &json!({"model_id":model.id,"model_sha256":model.sha256,"owner_hash":owner,"user_id":user,"expires_at":2000}), &json!({})).await.unwrap();
        assert!(
            db.hold_order_files(&id, &owner, &[quote], 1000)
                .await
                .unwrap()
        );
        db.create_order(&o, 1000).await.unwrap();
        db.create_order(&o, 1000).await.unwrap();
        assert!(db.poll_order_payment(&id, 1000).await.unwrap());
        assert!(!db.poll_order_payment(&id, 1001).await.unwrap());
        assert!(db.order(&id, Some("other"), None).await.unwrap().is_none());
        assert!(
            !db.order_message(&id, Some("other"), None, false, "forged", None, 1001)
                .await
                .unwrap()
        );
        assert!(
            db.order(&id, None, Some(&owner)).await.unwrap().is_none(),
            "account order must not fall back to guest cookie"
        );
        assert!(!db.order_paid(&id, o.provider_id, 1235, 1002).await.unwrap());
        assert!(db.order_paid(&id, o.provider_id, 1234, 1002).await.unwrap());
        assert!(!db.order_paid(&id, o.provider_id, 1234, 1002).await.unwrap());
        db.auth_preferences(&user, false, false).await.unwrap();
        assert!(db.order_status(&id, "in_production", 1003).await.unwrap());
        assert!(!db.order_status(&id, "paid", 1004).await.unwrap());
        assert!(
            db.order_message(&id, None, None, true, "Private content", None, 1005)
                .await
                .unwrap()
        );
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM email_outbox WHERE event_key LIKE $1")
                .bind(format!("order:{id}:%"))
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            count, 2,
            "only mandatory confirmation and payment, neither optional mail"
        );
        // A failed queue write must roll back the matching business transition.
        sqlx::query("ALTER TABLE email_outbox ADD CONSTRAINT reject_test_event CHECK(kind!='status_changed') NOT VALID").execute(&db.pool).await.unwrap();
        db.auth_preferences(&user, true, true).await.unwrap();
        assert!(db.order_status(&id, "ready", 1006).await.is_err());
        assert_eq!(
            db.order(&id, Some(&user), None).await.unwrap().unwrap()["status"],
            "in_production"
        );
        sqlx::query("ALTER TABLE email_outbox DROP CONSTRAINT reject_test_event")
            .execute(&db.pool)
            .await
            .unwrap();
    }
}
