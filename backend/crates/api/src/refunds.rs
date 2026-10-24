//! Admin-only refund requests; the shared payment worker reconciles durable reservations.
use super::{
    auth::Auth,
    checkout::{App, Result, error, now, unavailable},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_payments::{Error as PaymentError, ExpectedPayment, ExpectedRefund};
use rapidmaker_storage::{Refund, RefundPrepareError};
use serde::Deserialize;
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct RefundRequest {
    /// A new UUID for a new refund; reuse the same UUID when retrying this request.
    request_id: String,
    /// Positive amount in grosze, including VAT; full refund = remaining paid amount.
    amount_grosze: i64,
}
impl RefundRequest {
    fn validate(&self) -> Result<()> {
        if uuid::Uuid::parse_str(&self.request_id).is_err()
            || !(1..=99_999_999).contains(&self.amount_grosze)
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "invalid_refund",
                "Podaj request_id UUID i dodatnią kwotę zwrotu w groszach.",
            ));
        }
        Ok(())
    }
}
pub(super) fn router() -> OpenApiRouter<Arc<App>> {
    OpenApiRouter::new().routes(routes!(create, list))
}

#[utoipa::path(post,path="/api/admin/orders/{id}/refunds",operation_id="admin_refund_create",tag="Admin",
    summary="Zleć pełny lub częściowy zwrot płatności Stripe test.",
    description="Wymaga admina, passkey i CSRF. request_id identyfikuje jedną operację; ponowienie nie tworzy kolejnego zwrotu. 202 oznacza zapisanie zlecenia, nie potwierdzenie zwrotu. Status realizacji zamówienia pozostaje osobny.",
    params(("id"=String,Path)),request_body=RefundRequest,
    responses((status=202,body=Refund),(status=400,body=super::checkout::Problem),(status=401,body=super::checkout::Problem),(status=403,body=super::checkout::Problem),(status=404,body=super::checkout::Problem),(status=409,body=super::checkout::Problem),(status=429,body=super::checkout::Problem),(status=503,body=super::checkout::Problem)),
    security(("session"=[],"csrf"=[])))]
async fn create(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<RefundRequest>,
) -> Result<(StatusCode, Json<Refund>)> {
    let admin = auth.administrator(&headers, true).await?;
    input.validate()?;
    app.stripe()?;
    let refund = app
        .db()?
        .refund_prepare(
            &id,
            &input.request_id,
            input.amount_grosze,
            &admin.user.id,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .map_err(prepare_error)?;
    Ok((StatusCode::ACCEPTED, Json(refund)))
}
fn prepare_error(reason: RefundPrepareError) -> super::checkout::Error {
    let (status, code, message) = match reason {
        RefundPrepareError::ShipmentActive => (
            StatusCode::CONFLICT,
            "shipment_active",
            "Nadanie przesyłki jest w toku. Najpierw sprawdź jego wynik przed zleceniem zwrotu.",
        ),
        RefundPrepareError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "Nie znaleziono zamówienia.",
        ),
        RefundPrepareError::NotPaid => (
            StatusCode::CONFLICT,
            "payment_not_paid",
            "Zwrot wymaga potwierdzonej płatności.",
        ),
        RefundPrepareError::Amount => (
            StatusCode::CONFLICT,
            "refund_amount",
            "Kwota przekracza środki dostępne do zwrotu; uwzględniono też oczekujące zwroty.",
        ),
        RefundPrepareError::Conflict => (
            StatusCode::CONFLICT,
            "refund_changed",
            "Ten request_id został użyty z inną kwotą.",
        ),
        RefundPrepareError::Limit => (
            StatusCode::TOO_MANY_REQUESTS,
            "refund_limit",
            "Przekroczono limit zleceń zwrotu.",
        ),
        RefundPrepareError::Forbidden => (
            StatusCode::FORBIDDEN,
            "forbidden",
            "Brak uprawnień do zwrotu.",
        ),
    };
    error(status, code, message)
}
#[utoipa::path(get,path="/api/admin/orders/{id}/refunds",operation_id="admin_refund_list",tag="Admin",
    summary="Pobierz zlecone zwroty i ich aktualne statusy.",params(("id"=String,Path)),
    responses((status=200,body=Vec<Refund>),(status=401,body=super::checkout::Problem),(status=403,body=super::checkout::Problem),(status=503,body=super::checkout::Problem)),security(("session"=[])))]
async fn list(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<Refund>>> {
    auth.administrator(&headers, false).await?;
    Ok(Json(
        app.db()?
            .refund_list(&id)
            .await
            .map_err(|_| unavailable())?,
    ))
}

pub(super) async fn process_next(app: &App) {
    let Some(db) = &app.db else { return };
    match db.refund_lease(None, now()).await {
        Ok(Some(refund)) if reconcile(app, &refund).await.is_err() => {
            eprintln!("event=refund_reconcile_failed");
        }
        Err(_) => eprintln!("event=refund_claim_failed"),
        _ => {}
    }
}
async fn reconcile(
    app: &App,
    refund: &Refund,
) -> std::result::Result<(), rapidmaker_storage::StorageError> {
    let db = app.db.as_ref().ok_or("missing refund database")?;
    let lease = refund
        .lease_token
        .as_deref()
        .ok_or("missing refund lease")?;
    let result = execute(app, refund, lease).await;
    match result {
        Ok(Some(value)) => {
            if !db
                .refund_resolve(&refund.id, lease, &value.id, &value.status, now())
                .await?
            {
                return Err("refund lease lost before save".into());
            }
        }
        Ok(None) => {} // Lease was lost before the external mutation; the next owner resumes.
        Err(PaymentError::Mismatch | PaymentError::InvalidInput) => {
            db.refund_review(&refund.id, lease, now()).await?;
        }
        Err(PaymentError::Rejected { .. }) if refund.provider_id.is_some() => {
            db.refund_review(&refund.id, lease, now()).await?;
        }
        Err(error) => {
            // A later rejection cannot disprove a prior timed-out creation.
            db.refund_retry(
                &refund.id,
                lease,
                !matches!(error, PaymentError::Rejected { .. }),
                now(),
            )
            .await?;
        }
    }
    Ok(())
}
async fn execute(
    app: &App,
    refund: &Refund,
    lease: &str,
) -> std::result::Result<Option<rapidmaker_payments::Refund>, PaymentError> {
    let stripe = app.stripe.as_ref().ok_or(PaymentError::InvalidInput)?;
    let _permit = app.permit().map_err(|_| PaymentError::Unknown)?;
    if let Some(id) = &refund.provider_id {
        let expected = expected_refund(
            refund,
            refund
                .payment_intent
                .clone()
                .ok_or(PaymentError::Mismatch)?,
        );
        return stripe.retrieve_refund(id, &expected).await.map(Some);
    }
    if now() - refund.created_at >= 23 * 3600 {
        return Err(PaymentError::InvalidInput);
    }
    let expected = ExpectedPayment {
        correlation_id: refund.correlation_id.clone(),
        amount_grosze: refund.payment_amount_grosze,
    };
    let intent = if refund.provider_kind == "session" {
        let session = stripe
            .retrieve_checkout(&refund.original_provider_id, &expected)
            .await?;
        if !session.paid() {
            return Err(PaymentError::Mismatch);
        }
        session.payment_intent.ok_or(PaymentError::Mismatch)?
    } else {
        let intent = stripe
            .retrieve_intent(&refund.original_provider_id, &expected)
            .await?;
        if !intent.paid() {
            return Err(PaymentError::Mismatch);
        }
        intent.id
    };
    // Pin the provider mapping durably before creating a refund; never call Stripe after losing the lease.
    let pinned = app
        .db
        .as_ref()
        .ok_or(PaymentError::InvalidInput)?
        .refund_set_intent(&refund.id, lease, &intent, now())
        .await
        .map_err(|_| PaymentError::Unknown)?;
    if !pinned {
        return Ok(None);
    }
    if now() - refund.created_at >= 23 * 3600 {
        return Err(PaymentError::InvalidInput);
    }
    stripe
        .create_refund(&expected_refund(refund, intent), &refund.idempotency_key)
        .await
        .map(Some)
}
fn expected_refund(refund: &Refund, payment_intent: String) -> ExpectedRefund {
    ExpectedRefund {
        refund_id: refund.id.clone(),
        order_id: refund.order_id.clone(),
        payment_intent,
        amount_grosze: refund.amount_grosze,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refund_request_requires_uuid_positive_money_and_no_extra_fields() {
        let mut value =
            serde_json::json!({"request_id":uuid::Uuid::new_v4().to_string(),"amount_grosze":100});
        assert!(
            serde_json::from_value::<RefundRequest>(value.clone())
                .unwrap()
                .validate()
                .is_ok()
        );
        for amount in [0, -1, 100_000_000] {
            value["amount_grosze"] = amount.into();
            assert!(
                serde_json::from_value::<RefundRequest>(value.clone())
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        value["amount_grosze"] = 100.into();
        value["payment_intent"] = "pi_untrusted".into();
        assert!(serde_json::from_value::<RefundRequest>(value).is_err());
    }
}
