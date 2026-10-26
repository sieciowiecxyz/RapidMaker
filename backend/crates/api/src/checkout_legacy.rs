//! Compatibility endpoints. All money and payment ownership use the durable checkout engine.
use super::checkout::{
    self as c, App, DraftInput, PaymentInput, Result, conflict, error, identity, mutation, now,
    owned, unavailable,
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_shipping::{DeliveryChoice, Invoice, Method, Recipient};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(default, deny_unknown_fields)]
struct Contact {
    name: String,
    email: String,
    phone: String,
    street: String,
    city: String,
    #[serde(rename = "postalCode")]
    postal_code: String,
    #[serde(rename = "pickupPoint")]
    pickup_point: String,
    company: String,
    nip: String,
    #[serde(rename = "emailOrderUpdates")]
    email_order_updates: bool,
}
impl Default for Contact {
    fn default() -> Self {
        Self {
            name: String::new(),
            email: String::new(),
            phone: String::new(),
            street: String::new(),
            city: String::new(),
            postal_code: String::new(),
            pickup_point: String::new(),
            company: String::new(),
            nip: String::new(),
            email_order_updates: true,
        }
    }
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct QuoteInput {
    parts: Vec<super::cart::CartPart>,
    delivery: rapidmaker_packing::Delivery,
    contact: Option<Contact>,
    #[serde(default)]
    manual: bool,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Start {
    #[serde(default)]
    #[schema(required = true, example = "2026-10-25")]
    legal_version: String,
    quote_id: String,
    offer_id: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Search {
    search: String,
}
#[utoipa::path(post,path="/api/shipping/points",tag="Dostawa",operation_id="legacy_shipping_points",request_body=Search,responses((status=200,body=Value),(status=503,body=Value)),security(("session"=[]),("guest"=[])))]
async fn points(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<Search>,
) -> Result<Json<Value>> {
    let owner = identity(&app, &headers).await?.0;
    app.provider_limit(&owner).await?;
    let _permit = app.permit()?;
    let points = app
        .shipping
        .as_ref()
        .ok_or_else(unavailable)?
        .points(input.search.trim())
        .await
        .map_err(c::shipping_error)?;
    Ok(Json(json!({"points":points})))
}
#[utoipa::path(post,path="/api/checkout/quote",tag="Checkout",operation_id="legacy_checkout_quote",request_body=QuoteInput,
 responses((status=200,body=Value),(status=409,body=Value),(status=422,body=Value),(status=503,body=Value)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn quote(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Json(input): Json<QuoteInput>,
) -> Result<Json<Value>> {
    mutation(&auth, &headers).await?;
    let contact = input.contact.unwrap_or_default();
    let method = match input.delivery {
        rapidmaker_packing::Delivery::Courier => Method::Courier,
        rapidmaker_packing::Delivery::Locker => Method::Locker,
        rapidmaker_packing::Delivery::Pickup => Method::Pickup,
    };
    let invoice = if !contact.nip.is_empty() || !contact.company.is_empty() {
        Some(Invoice {
            company: contact.company,
            nip: contact.nip,
            street: contact.street.clone(),
            postal_code: contact.postal_code.clone(),
            city: contact.city.clone(),
            country: "PL".into(),
        })
    } else {
        None
    };
    let draft = c::create_draft(
        &app,
        &headers,
        DraftInput {
            parts: input.parts,
            recipient: Recipient {
                name: contact.name,
                email: contact.email,
                phone: contact.phone,
                street: contact.street,
                postal_code: contact.postal_code,
                city: contact.city,
                invoice,
                country: "PL".into(),
                apartment: String::new(),
            },
            delivery: DeliveryChoice {
                method,
                point_id: (!contact.pickup_point.is_empty()).then_some(contact.pickup_point),
            },
            manual: input.manual,
            email_order_updates: contact.email_order_updates,
        },
    )
    .await?;
    let draft = c::quote_draft(&app, &headers, &draft.id, draft.version, true).await?;
    let mut result = draft.quote.ok_or_else(unavailable)?;
    if let Some(o) = result.as_object_mut() {
        for key in [
            "contact",
            "quote_ids",
            "lines",
            "guest_updates",
            "_source_version",
        ] {
            o.remove(key);
        }
    }
    result["id"] = json!(draft.id);
    result["manual"] = json!(input.manual);
    result["production_demo"] = json!(true);
    result["stripe_configured"] = json!(app.stripe.is_some());
    result["furgonetka_configured"] = json!(app.shipping.is_some());
    Ok(Json(result))
}
async fn start(
    app: &Arc<App>,
    headers: &HeaderMap,
    input: Start,
    kind: &str,
) -> Result<rapidmaker_storage::PaymentAttempt> {
    let draft = owned(app, &input.quote_id, headers).await?;
    let key = format!("legacy:{kind}:{}", input.quote_id);
    c::start_payment(
        app,
        headers,
        &input.quote_id,
        PaymentInput {
            legal_version: input.legal_version,
            version: draft.version,
            offer_id: input.offer_id,
        },
        &key,
        kind,
    )
    .await
}
#[utoipa::path(post,path="/api/checkout/session",tag="Checkout",operation_id="legacy_checkout_session",request_body=Start,
 responses((status=200,body=Value),(status=409,body=Value),(status=503,body=Value)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn session(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Json(input): Json<Start>,
) -> Result<Json<Value>> {
    mutation(&auth, &headers).await?;
    let a = start(&app, &headers, input, "session").await?;
    let response = a.provider_response.ok_or_else(unavailable)?;
    let url = response["url"]
        .as_str()
        .filter(|_| a.state == "open")
        .ok_or_else(conflict)?;
    Ok(Json(
        json!({"url":url,"session_id":a.provider_id,"order_id":a.id}),
    ))
}
#[utoipa::path(post,path="/api/checkout/intent",tag="Checkout",operation_id="legacy_checkout_intent",request_body=Start,
 responses((status=200,body=Value),(status=409,body=Value),(status=503,body=Value)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn intent(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Json(input): Json<Start>,
) -> Result<Json<Value>> {
    mutation(&auth, &headers).await?;
    let a = start(&app, &headers, input, "intent").await?;
    intent_secret(&app, &a).await
}
async fn by_provider(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    kind: &str,
) -> Result<rapidmaker_storage::PaymentAttempt> {
    let (owner, user) = identity(app, headers).await?;
    if let Some(a) = app
        .db()?
        .checkout_attempt_provider(kind, id)
        .await
        .map_err(|_| unavailable())?
    {
        return app
            .db()?
            .checkout_attempt(&a.id, &owner)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| {
                error(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "Nie znaleziono płatności.",
                )
            });
    }
    let legacy = app
        .db()?
        .checkout_legacy_order(kind, id)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "not_found",
                "Nie znaleziono płatności.",
            )
        })?;
    let order_id = legacy["id"].as_str().ok_or_else(unavailable)?;
    if app
        .db()?
        .order(order_id, user.as_deref(), Some(&owner))
        .await
        .map_err(|_| unavailable())?
        .is_none()
    {
        return Err(error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Nie znaleziono płatności.",
        ));
    }
    Ok(rapidmaker_storage::PaymentAttempt {
        id: order_id.into(),
        draft_id: order_id.into(),
        state: legacy["status"].as_str().unwrap_or("unknown").into(),
        provider_kind: kind.into(),
        provider_id: Some(id.into()),
        offer_id: None,
        amount_grosze: legacy["amount_grosze"].as_i64().ok_or_else(unavailable)?,
        provider_request: Value::Null,
        idempotency_key: String::new(),
        provider_response: None,
        lease_token: None,
        attempts: 0,
        created_at: 0,
    })
}
async fn payment_status(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    kind: &str,
) -> Result<Json<Value>> {
    let a = by_provider(app, headers, id, kind).await?;
    let object = super::checkout_worker::retrieve(app, &a)
        .await
        .map_err(|_| unavailable())?;
    let paid = if kind == "session" {
        object["payment_status"] == "paid" && object["status"] == "complete"
    } else {
        object["status"] == "succeeded"
    };
    if a.provider_request.is_null() {
        if paid {
            app.db()?
                .order_paid(&a.id, id, a.amount_grosze, now())
                .await
                .map_err(|_| unavailable())?;
        }
    } else if !app
        .db()?
        .checkout_resolve(&a.id, kind, &object, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(unavailable());
    }
    Ok(Json(
        json!({"status":if paid{"paid"}else{"unpaid"},"livemode":false,"total_grosze":a.amount_grosze,"currency":"pln","quote_id":a.draft_id,"order_id":a.id,"checkout_status":object["status"],"url":object["url"]}),
    ))
}
#[utoipa::path(get,path="/api/checkout/session/{id}",tag="Checkout",operation_id="legacy_session_status",params(("id"=String,Path)),responses((status=200,body=Value),(status=404,body=Value)),security(("session"=[]),("guest"=[])))]
async fn session_status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    payment_status(&app, &headers, &id, "session").await
}
#[utoipa::path(get,path="/api/checkout/intent/{id}",tag="Checkout",operation_id="legacy_intent_status",params(("id"=String,Path)),responses((status=200,body=Value),(status=404,body=Value)),security(("session"=[]),("guest"=[])))]
async fn intent_status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    payment_status(&app, &headers, &id, "intent").await
}
async fn intent_secret(app: &App, a: &rapidmaker_storage::PaymentAttempt) -> Result<Json<Value>> {
    let object = super::checkout_worker::retrieve(app, a)
        .await
        .map_err(|_| unavailable())?;
    if !matches!(
        object["status"].as_str(),
        Some("requires_payment_method" | "requires_confirmation" | "requires_action")
    ) {
        return Err(conflict());
    }
    let secret = object["client_secret"].as_str().ok_or_else(unavailable)?;
    Ok(Json(
        json!({"client_secret":secret,"payment_intent_id":a.provider_id,"order_id":a.id,"total_grosze":a.amount_grosze}),
    ))
}
#[utoipa::path(get,path="/api/checkout/intent/{id}/resume",tag="Checkout",operation_id="legacy_intent_resume",params(("id"=String,Path)),responses((status=200,body=Value),(status=404,body=Value),(status=409,body=Value)),security(("session"=[]),("guest"=[])))]
async fn resume(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let a = by_provider(&app, &headers, &id, "intent").await?;
    intent_secret(&app, &a).await
}
#[utoipa::path(get,path="/api/checkout/config",tag="Checkout",operation_id="legacy_checkout_config",responses((status=200,body=Value)))]
async fn config(State(app): State<Arc<App>>) -> Json<Value> {
    Json(json!({"publishable_key":app.publishable_key}))
}
#[utoipa::path(get,path="/api/checkout/status",tag="Checkout",operation_id="legacy_checkout_status",responses((status=200,body=Value)))]
async fn status(State(app): State<Arc<App>>) -> Json<Value> {
    Json(
        json!({"stripe_configured":app.stripe.is_some(),"furgonetka_configured":app.shipping.is_some(),"webhook_configured":app.webhook_secret.is_some()}),
    )
}
pub(super) fn router() -> OpenApiRouter<Arc<App>> {
    OpenApiRouter::new()
        .routes(routes!(points))
        .routes(routes!(quote))
        .routes(routes!(session))
        .routes(routes!(intent))
        .routes(routes!(session_status))
        .routes(routes!(intent_status))
        .routes(routes!(resume))
        .routes(routes!(config))
        .routes(routes!(status))
}
