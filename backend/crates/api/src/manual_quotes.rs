//! Authenticated manual requests/offers use the same durable Stripe worker, not slicer pricing.
use super::{
    auth::Auth,
    checkout::{App, Problem, Result, error, now, unavailable},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_shipping::{DeliveryChoice, Recipient};
use rapidmaker_slicer::Options;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc};
use utoipa_axum::{router::OpenApiRouter, routes};
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ManualQuotePart)]
struct Part {
    model_id: String,
    quantity: u32,
    material_id: String,
    color_code: String,
    options: Options,
}
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ManualQuoteRequest)]
struct Request {
    request_id: String,
    parts: Vec<Part>,
    contact: Recipient,
    delivery: DeliveryChoice,
    #[serde(default)]
    notes: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ManualQuoteOffer)]
struct Offer {
    version: i64,
    amount_grosze: i64,
    shipping_grosze: i64,
    scope: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ManualQuotePayment)]
struct Payment {
    #[serde(default)]
    #[schema(required = true, example = "2026-10-25")]
    legal_version: String,
    version: i64,
    request_id: String,
}
fn conflict() -> super::checkout::Error {
    error(
        StatusCode::CONFLICT,
        "manual_quote_changed",
        "Zgłoszenie lub oferta wygasły, zmieniły się albo płatność została już rozpoczęta. Odśwież zamówienie; po zakończonej nieudanej próbie wyślij nowe zgłoszenie.",
    )
}
pub(super) fn router() -> OpenApiRouter<Arc<App>> {
    OpenApiRouter::new()
        .routes(routes!(request))
        .routes(routes!(publish))
        .routes(routes!(payment))
}
#[utoipa::path(post,path="/api/account/manual-quotes",operation_id="manual_quote_request",tag="Zamówienia",request_body=Request,responses((status=200,body=Value),(status=400,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn request(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Json(mut input): Json<Request>,
) -> Result<Json<Value>> {
    let session = auth.mutation(&headers).await?;
    if uuid::Uuid::parse_str(&input.request_id).is_err()
        || input.parts.is_empty()
        || input.parts.len() > 10
        || input.notes.len() > 4000
        || input
            .notes
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_manual_request",
            "Sprawdź części, identyfikator zgłoszenia i opis (do 4000 znaków).",
        ));
    }
    input.notes = input.notes.trim().to_owned();
    input.contact.email = session.user.email;
    input.contact.normalize(&input.delivery).map_err(|v|(StatusCode::UNPROCESSABLE_ENTITY,Json(json!({"code":"invalid_recipient","message":"Popraw dane odbiorcy.","field_errors":v.field_errors}))))?;
    let catalog = app.db()?.materials().await.map_err(|_| unavailable())?;
    let mut models = HashSet::new();
    for part in &mut input.parts {
        part.options.quantity = part.quantity;
        if part.options.validate().is_err()
            || uuid::Uuid::parse_str(&part.model_id).is_err()
            || !models.insert(&part.model_id)
        {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "invalid_manual_parts",
                "Dodaj różne części i poprawne ilości/opcje druku.",
            ));
        }
        if !catalog.iter().any(|m| {
            m.id == part.material_id
                && super::quotes::customer_material(&m.id)
                && super::quotes::known_color(m, &part.color_code)
        }) {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "invalid_material",
                "Wybierz PLA, ABS albo TPU oraz kolor należący do tego profilu.",
            ));
        }
    }
    let input_json = json!({"parts":input.parts,"contact":input.contact,"delivery":input.delivery,"notes":input.notes});
    app.db()?
        .manual_expire(now())
        .await
        .map_err(|_| unavailable())?;
    let id = app
        .db()?
        .manual_request(&session.user.id, &input.request_id, &input_json, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(conflict)?;
    Ok(Json(
        json!({"order_id":id,"manual_offer":app.db()?.manual_offer(&id).await.map_err(|_|unavailable())?}),
    ))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/quote",operation_id="manual_quote_publish",tag="Admin",params(("id"=String,Path)),request_body=Offer,responses((status=200,body=Value),(status=400,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn publish(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Offer>,
) -> Result<Json<Value>> {
    let session = auth.administrator(&headers, true).await?;
    if input.version < 1
        || input.version == i64::MAX
        || !(1..=99999999).contains(&input.amount_grosze)
        || !(0..=input.amount_grosze).contains(&input.shipping_grosze)
        || input.scope.trim().is_empty()
        || input.scope.len() > 4000
        || input
            .scope
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_manual_offer",
            "Podaj cenę brutto w groszach, dostawę nie większą niż suma i zakres realizacji (do 4000 znaków).",
        ));
    }
    app.db()?
        .manual_expire(now())
        .await
        .map_err(|_| unavailable())?;
    if !app
        .db()?
        .manual_publish(
            &id,
            &session.user.id,
            input.version,
            input.amount_grosze,
            input.shipping_grosze,
            input.scope.trim(),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
    {
        return Err(conflict());
    }
    Ok(Json(
        json!({"order_id":id,"manual_offer":app.db()?.manual_offer(&id).await.map_err(|_|unavailable())?}),
    ))
}
#[utoipa::path(post,path="/api/account/orders/{id}/payment",operation_id="manual_quote_payment",tag="Zamówienia",params(("id"=String,Path)),request_body=Payment,responses((status=200,body=Value),(status=202,body=Value),(status=409,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn payment(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Payment>,
) -> Result<(StatusCode, Json<Value>)> {
    let session = auth.mutation(&headers).await?;
    super::legal::validate(&input.legal_version)?;
    app.stripe()?;
    if input.version < 1 || uuid::Uuid::parse_str(&input.request_id).is_err() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_payment_request",
            "Sprawdź wersję oferty i identyfikator próby.",
        ));
    }
    app.db()?
        .manual_expire(now())
        .await
        .map_err(|_| unavailable())?;
    let a = app
        .db()?
        .manual_payment(
            &id,
            &session.user.id,
            input.version,
            &input.request_id,
            now(),
            &input.legal_version,
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(conflict)?;
    if a.attempts == 0 {
        app.provider_limit(&rapidmaker_storage::account_owner(&session.user.id))
            .await?;
    }
    super::checkout_worker::process_attempt(&app, &a.id).await;
    let a = app
        .db()?
        .checkout_attempt(&a.id, &rapidmaker_storage::account_owner(&session.user.id))
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(conflict)?;
    let secret = a
        .provider_response
        .as_ref()
        .filter(|o| {
            a.state == "open"
                && matches!(
                    o["status"].as_str(),
                    Some("requires_payment_method" | "requires_confirmation" | "requires_action")
                )
        })
        .and_then(|o| o["client_secret"].as_str());
    let status = if matches!(a.state.as_str(), "creating" | "unknown" | "manual_review") {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(
            json!({"attempt_id":a.id,"order_id":id,"state":a.state,"client_secret":secret,"payment_intent_id":a.provider_id,"total_grosze":a.amount_grosze}),
        ),
    ))
}
