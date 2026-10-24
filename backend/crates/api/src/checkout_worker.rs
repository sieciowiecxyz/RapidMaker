//! A small PostgreSQL-backed reconciliation loop, shared by webhooks and HTTP retries.
use super::checkout::{App, Error, Result, error, now, unavailable};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use rapidmaker_payments::{Error as PaymentError, ExpectedPayment, verify_webhook};
use rapidmaker_storage::{PaymentAttempt, Storage, StripeEvent};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

pub struct CheckoutWorker {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl CheckoutWorker {
    pub fn start(app: Arc<App>) -> Self {
        let (stop, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                if *receiver.borrow() {
                    break;
                }
                if let Some(db) = &app.db {
                    match db.stripe_event_lease(now()).await {
                        Ok(Some(event)) => process_event(&app, event).await,
                        Err(_) => eprintln!("event=stripe_event_claim_failed"),
                        _ => {}
                    }
                    match db.checkout_lease(None, now()).await {
                        Ok(Some(attempt)) => reconcile(&app, attempt).await,
                        Err(_) => eprintln!("event=payment_claim_failed"),
                        _ => {}
                    }
                }
                tokio::select! {_=tokio::time::sleep(Duration::from_secs(3))=>{},_=receiver.changed()=>break}
            }
        });
        Self { stop, task }
    }
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(25), &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => eprintln!("event=checkout_worker_failed"),
            Err(_) => {
                task.abort();
                eprintln!("event=checkout_worker_shutdown_timeout");
            }
        }
    }
}
pub(super) async fn process_attempt(app: &Arc<App>, id: &str) {
    if let Some(db) = &app.db {
        match db.checkout_lease(Some(id), now()).await {
            Ok(Some(attempt)) => reconcile(app, attempt).await,
            Err(_) => eprintln!("event=payment_claim_failed"),
            _ => {}
        }
    }
}
pub(super) async fn retrieve(
    app: &App,
    attempt: &PaymentAttempt,
) -> std::result::Result<Value, PaymentError> {
    let stripe = app.stripe.as_ref().ok_or(PaymentError::InvalidInput)?;
    let expected = ExpectedPayment {
        correlation_id: attempt.id.clone(),
        amount_grosze: attempt.amount_grosze,
    };
    let id = attempt
        .provider_id
        .as_deref()
        .ok_or(PaymentError::InvalidInput)?;
    let _permit = app.permit().map_err(|_| PaymentError::Unknown)?;
    if attempt.provider_kind == "session" {
        Ok(stripe.retrieve_checkout(id, &expected).await?.object)
    } else {
        Ok(stripe.retrieve_intent(id, &expected).await?.object)
    }
}
async fn reconcile(app: &App, attempt: PaymentAttempt) {
    let Some(db) = &app.db else { return };
    let Some(lease) = attempt.lease_token.as_deref() else {
        return;
    };
    let result = if attempt.provider_id.is_some() {
        retrieve(app, &attempt).await
    } else {
        async {
            let stripe = app.stripe.as_ref().ok_or(PaymentError::InvalidInput)?;
            let form: Vec<(String, String)> =
                serde_json::from_value(attempt.provider_request.clone())
                    .map_err(|_| PaymentError::InvalidInput)?;
            let expected = ExpectedPayment {
                correlation_id: attempt.id.clone(),
                amount_grosze: attempt.amount_grosze,
            };
            let _permit = app.permit().map_err(|_| PaymentError::Unknown)?;
            if attempt.provider_kind == "session" {
                Ok(stripe
                    .create_checkout(&form, &attempt.idempotency_key, &expected)
                    .await?
                    .object)
            } else {
                Ok(stripe
                    .create_intent(&form, &attempt.idempotency_key, &expected)
                    .await?
                    .object)
            }
        }
        .await
    };
    let saved = match result {
        Ok(object) => {
            db.checkout_resolve(&attempt.id, &attempt.provider_kind, &object, now())
                .await
        }
        Err(e) => {
            // Only a definite first creation rejection can release the protected files.
            let rejected = matches!(
                e,
                PaymentError::Rejected { .. } | PaymentError::InvalidInput
            );
            eprintln!(
                "event=payment_reconcile_retry attempt={} rejected={rejected} reason={e:?}",
                attempt.id
            );
            db.checkout_retry(&attempt.id, lease, rejected, now()).await
        }
    };
    if !matches!(saved, Ok(true)) {
        eprintln!("event=payment_reconcile_not_saved attempt={}", attempt.id);
    }
}
fn event_kind(event_type: &str) -> Option<&'static str> {
    match event_type {
        "checkout.session.completed"
        | "checkout.session.async_payment_succeeded"
        | "checkout.session.async_payment_failed"
        | "checkout.session.expired" => Some("session"),
        "payment_intent.succeeded"
        | "payment_intent.processing"
        | "payment_intent.payment_failed"
        | "payment_intent.canceled" => Some("intent"),
        _ => None,
    }
}
#[utoipa::path(post,path="/api/webhooks/stripe",tag="Stripe",operation_id="stripe_webhook",
    params(("Stripe-Signature"=String,Header,description="Podpis Stripe; zwykłe wywołanie Swaggerem bez podpisu jest odrzucane")),
    request_body(content=Value,content_type="application/json"),responses((status=204,description="Zdarzenie przyjęte trwale lub bezpiecznie pominięte"),(status=400,description="Nieprawidłowy podpis"),(status=503,description="Brak trwałego zapisu")))]
pub(super) async fn webhook(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode> {
    let secret = app.webhook_secret.as_deref().ok_or_else(unavailable)?;
    if headers.get_all("Stripe-Signature").iter().count() != 1 {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_signature",
            "Nieprawidłowy podpis webhooka.",
        ));
    }
    let signature = headers
        .get("Stripe-Signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let event = verify_webhook(secret, signature, &body, now()).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_signature",
            "Nieprawidłowy podpis webhooka.",
        )
    })?;
    let Some(kind) = event_kind(&event.event_type) else {
        return Ok(StatusCode::NO_CONTENT);
    };
    let raw: Value = serde_json::from_slice(&body).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_event",
            "Nieprawidłowe zdarzenie.",
        )
    })?;
    // Keep only routing evidence. Addresses, card metadata and client secrets are unnecessary here.
    let correlation = raw["data"]["object"]["metadata"]["quote_id"]
        .as_str()
        .filter(|s| uuid::Uuid::parse_str(s).is_ok());
    let payload = json!({"livemode":false,"data":{"object":{"id":event.object_id,"metadata":{"quote_id":correlation}}}});
    app.db()?
        .stripe_event_accept(
            &event.id,
            &event.event_type,
            kind,
            &event.object_id,
            &payload,
            now(),
        )
        .await
        .map_err(|_| unavailable())?;
    Ok(StatusCode::NO_CONTENT)
}
async fn process_event(app: &App, event: StripeEvent) {
    let Some(db) = &app.db else { return };
    let result: std::result::Result<bool, Box<dyn std::error::Error + Send + Sync>> = async {
        let mut attempt = db
            .checkout_attempt_provider(&event.provider_kind, &event.object_id)
            .await?;
        if attempt.is_none()
            && let Some(id) = event.payload["data"]["object"]["metadata"]["quote_id"].as_str()
        {
            attempt = db.checkout_attempt_id(id).await?;
        }
        let Some(mut attempt) = attempt else {
            if let Some(order) = db
                .checkout_legacy_order(&event.provider_kind, &event.object_id)
                .await?
            {
                let id = order["id"].as_str().ok_or("invalid legacy order")?;
                let expected = ExpectedPayment {
                    correlation_id: id.into(),
                    amount_grosze: order["amount_grosze"]
                        .as_i64()
                        .ok_or("invalid legacy amount")?,
                };
                let stripe = app.stripe.as_ref().ok_or("Stripe unavailable")?;
                let _permit = app.permit().map_err(|_| "provider busy")?;
                let paid = if event.provider_kind == "session" {
                    stripe
                        .retrieve_checkout(&event.object_id, &expected)
                        .await?
                        .paid()
                } else {
                    stripe
                        .retrieve_intent(&event.object_id, &expected)
                        .await?
                        .paid()
                };
                if paid {
                    db.order_paid(id, &event.object_id, expected.amount_grosze, now())
                        .await?;
                }
            }
            return Ok(true);
        };
        if attempt.provider_kind != event.provider_kind
            || attempt
                .provider_id
                .as_deref()
                .is_some_and(|id| id != event.object_id)
        {
            return Ok(false);
        }
        attempt.provider_id = Some(event.object_id.clone());
        let object = retrieve(app, &attempt).await?;
        let resolved = db
            .checkout_resolve(&attempt.id, &attempt.provider_kind, &object, now())
            .await?;
        if resolved
            && event.event_type == "checkout.session.async_payment_failed"
            && object["status"] == "complete"
            && object["payment_status"] != "paid"
        {
            db.checkout_async_failed(&attempt.id, &event.object_id, now())
                .await?;
        }
        Ok(resolved)
    }
    .await;
    let success = matches!(result, Ok(true));
    if !success {
        eprintln!("event=stripe_event_retry id={}", event.id);
    }
    if db
        .stripe_event_finish(&event.id, &event.lease_token, success, now())
        .await
        .is_err()
    {
        eprintln!("event=stripe_event_finish_failed");
    }
}
pub(super) async fn refresh_order(
    db: &Storage,
    id: &str,
    order: &Value,
) -> std::result::Result<(), Error> {
    if order["status"] != "awaiting_payment"
        || !db
            .poll_order_payment(id, now())
            .await
            .map_err(|_| unavailable())?
    {
        return Ok(());
    }
    let app = App::new(Some(db.clone())).map_err(|_| unavailable())?;
    if let Some(attempt) = db
        .checkout_attempt_id(id)
        .await
        .map_err(|_| unavailable())?
    {
        if attempt.provider_id.is_some() {
            let object = retrieve(&app, &attempt).await.map_err(|_| unavailable())?;
            if !db
                .checkout_resolve(id, &attempt.provider_kind, &object, now())
                .await
                .map_err(|_| unavailable())?
            {
                return Err(unavailable());
            }
        } else {
            process_attempt(&app, id).await;
        }
        return Ok(());
    }
    // Pre-migration orders remain readable/payable with their original Stripe reference.
    let provider = order["provider_id"].as_str().ok_or_else(unavailable)?;
    let expected = ExpectedPayment {
        correlation_id: id.into(),
        amount_grosze: order["amount_grosze"].as_i64().ok_or_else(unavailable)?,
    };
    let stripe = app.stripe()?;
    let paid = if order["provider_kind"] == "session" {
        stripe
            .retrieve_checkout(provider, &expected)
            .await
            .map_err(|_| unavailable())?
            .paid()
    } else {
        stripe
            .retrieve_intent(provider, &expected)
            .await
            .map_err(|_| unavailable())?
            .paid()
    };
    if paid {
        db.order_paid(id, provider, expected.amount_grosze, now())
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(())
}
/// Local operator recovery: a provider ID is accepted only after Stripe proves the stored correlation.
pub async fn operator_status(
    id: &str,
    provider: Option<&str>,
    close: bool,
) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    uuid::Uuid::parse_str(id)?;
    let db = Storage::connect(&std::env::var("DATABASE_URL")?).await?;
    let app = App::new(Some(db.clone()))?;
    let mut attempt = db
        .checkout_attempt_id(id)
        .await?
        .ok_or("Unknown payment attempt")?;
    if let Some(provider) = provider {
        if attempt
            .provider_id
            .as_deref()
            .is_some_and(|old| old != provider)
        {
            return Err("Provider reference already bound".into());
        }
        attempt.provider_id = Some(provider.into());
    }
    if attempt.provider_id.is_some() {
        let mut object = retrieve(&app, &attempt).await?;
        if close && attempt.provider_kind == "session" && object["status"] == "open" {
            let expected = ExpectedPayment {
                correlation_id: attempt.id.clone(),
                amount_grosze: attempt.amount_grosze,
            };
            let provider = attempt
                .provider_id
                .as_deref()
                .ok_or("Missing provider reference")?;
            object = app
                .stripe
                .as_ref()
                .ok_or("Stripe unavailable")?
                .expire_checkout(provider, &format!("expire:{}", attempt.id), &expected)
                .await?
                .object;
        } else if close && attempt.provider_kind != "session" {
            return Err("Legacy PaymentIntent must be canceled in Stripe and reconciled".into());
        }
        if !db
            .checkout_resolve(id, &attempt.provider_kind, &object, now())
            .await?
        {
            return Err("Provider evidence does not match attempt".into());
        }
    } else if close {
        return Err("Unknown Stripe outcome: identify original session before closing".into());
    }
    let a = db.checkout_attempt_id(id).await?.ok_or("Missing attempt")?;
    Ok(
        json!({"id":a.id,"state":a.state,"provider_kind":a.provider_kind,"provider_id":a.provider_id,"amount_grosze":a.amount_grosze,"currency":"pln"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_payment_events_are_reconciled() {
        assert_eq!(
            event_kind("checkout.session.async_payment_succeeded"),
            Some("session")
        );
        assert_eq!(event_kind("payment_intent.payment_failed"), Some("intent"));
        assert_eq!(event_kind("customer.updated"), None);
    }
}
