//! Durable checkout. Provider calls happen after PostgreSQL commits, never under a DB lock.
use axum::{
    Extension, Json,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rapidmaker_packing::{Delivery, Estimate, EstimateRequest};
use rapidmaker_payments::Stripe;
use rapidmaker_pricing::PrintLine;
use rapidmaker_shipping::{DeliveryChoice, Method, Recipient, Sandbox};
use rapidmaker_storage::{AttemptInput, CheckoutDraft, PaymentAttempt, Storage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) type Error = (StatusCode, Json<Value>);
pub(super) type Result<T> = std::result::Result<T, Error>;
pub(super) use super::cart::now;
pub(super) struct App {
    pub db: Option<Storage>,
    pub stripe: Option<Stripe>,
    pub shipping: Option<Sandbox>,
    pub webhook_secret: Option<String>,
    pub origin: String,
    pub publishable_key: Option<String>,
    calls: Arc<Semaphore>,
}
impl App {
    pub(super) fn new(
        db: Option<Storage>,
    ) -> std::result::Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let stripe = std::env::var("STRIPE_SECRET_KEY")
            .ok()
            .filter(|v| !v.is_empty())
            .map(Stripe::new)
            .transpose()?;
        let shipping = match (
            std::env::var("FURGONETKA_SANDBOX_TOKEN").ok(),
            std::env::var("FURGONETKA_SENDER_JSON").ok(),
        ) {
            (Some(token), Some(sender)) if !token.is_empty() => Some(
                Sandbox::new(token, serde_json::from_str(&sender)?)
                    .map_err(|_| "Invalid shipping configuration")?,
            ),
            _ => None,
        };
        let origin =
            std::env::var("AUTH_ORIGIN").unwrap_or_else(|_| "http://localhost:18430".into());
        let url = reqwest::Url::parse(&origin)?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("Invalid checkout return origin".into());
        }
        Ok(Arc::new(Self {
            db,
            stripe,
            shipping,
            origin: origin.trim_end_matches('/').into(),
            webhook_secret: std::env::var("STRIPE_WEBHOOK_SECRET")
                .ok()
                .filter(|v| !v.is_empty()),
            publishable_key: std::env::var("STRIPE_PUBLISHABLE_KEY")
                .ok()
                .filter(|v| v.starts_with("pk_test_")),
            calls: Arc::new(Semaphore::new(4)),
        }))
    }
    pub fn db(&self) -> Result<&Storage> {
        self.db.as_ref().ok_or_else(unavailable)
    }
    pub fn stripe(&self) -> Result<&Stripe> {
        self.stripe.as_ref().ok_or_else(unavailable)
    }
    pub async fn provider_limit(&self, owner: &str) -> Result<()> {
        if !self
            .db()?
            .checkout_provider_allowed(owner, now())
            .await
            .map_err(|_| unavailable())?
        {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Za dużo zapytań o dostawę. Spróbuj za minutę.",
            ));
        }
        Ok(())
    }
    pub fn permit(&self) -> Result<OwnedSemaphorePermit> {
        self.calls.clone().try_acquire_owned().map_err(|_| {
            error(
                StatusCode::TOO_MANY_REQUESTS,
                "busy",
                "Spróbuj ponownie za chwilę.",
            )
        })
    }
}
#[derive(Serialize, utoipa::ToSchema)]
pub(super) struct Problem {
    code: String,
    message: String,
    field_errors: BTreeMap<String, String>,
}
pub(super) fn error(status: StatusCode, code: &str, message: &str) -> Error {
    (
        status,
        Json(json!({"code":code,"message":message,"field_errors":{}})),
    )
}
pub(super) fn unavailable() -> Error {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Usługa jest chwilowo niedostępna. Spróbuj ponownie.",
    )
}
pub(super) fn conflict() -> Error {
    error(
        StatusCode::CONFLICT,
        "checkout_changed",
        "Zamówienie lub oferta uległy zmianie. Pobierz aktualne dane.",
    )
}
fn missing() -> Error {
    error(
        StatusCode::NOT_FOUND,
        "not_found",
        "Nie znaleziono zamówienia.",
    )
}
fn validation(v: rapidmaker_shipping::Validation) -> Error {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(
            json!({"code":"invalid_fields","message":"Popraw wskazane pola.","field_errors":v.field_errors}),
        ),
    )
}
pub(super) fn shipping_error(e: rapidmaker_shipping::Error) -> Error {
    match e {
        rapidmaker_shipping::Error::Validation(v) => validation(v),
        _ => unavailable(),
    }
}
#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DraftInput {
    pub parts: Vec<super::cart::CartPart>,
    #[serde(default)]
    pub recipient: Recipient,
    #[serde(default)]
    pub delivery: DeliveryChoice,
    #[serde(default)]
    pub manual: bool,
    #[serde(default = "yes")]
    pub email_order_updates: bool,
}
fn yes() -> bool {
    true
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct DraftPatch {
    version: i64,
    parts: Option<Vec<super::cart::CartPart>>,
    recipient: Option<Recipient>,
    delivery: Option<DeliveryChoice>,
    manual: Option<bool>,
    email_order_updates: Option<bool>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Version {
    version: i64,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PaymentInput {
    pub version: i64,
    pub offer_id: String,
}
#[derive(Serialize, utoipa::ToSchema)]
struct CheckoutOffer {
    id: String,
    name: String,
    price_grosze: u32,
    total_grosze: u32,
    recommended: bool,
}
#[derive(Serialize, utoipa::ToSchema)]
struct QuoteDetails {
    packing: Estimate,
    offers: Vec<CheckoutOffer>,
    print_grosze: u32,
    print_lines: Vec<PrintLine>,
    setup_grosze: u32,
    packaging_grosze: u32,
    expires_at: i64,
    manual_required: bool,
    reason: Option<String>,
    point: Option<rapidmaker_shipping::Point>,
}
#[derive(Serialize, utoipa::ToSchema)]
struct DraftView {
    id: String,
    version: i64,
    input: DraftInput,
    missing_fields: Vec<String>,
    #[schema(value_type = Option<QuoteDetails>)]
    quote: Option<Value>,
    expires_at: i64,
    locked: bool,
    payment: Option<PaymentView>,
}
#[derive(Serialize, utoipa::ToSchema)]
pub(super) struct PaymentView {
    pub id: String,
    pub state: String,
    pub order_id: String,
    pub url: Option<String>,
    pub amount_grosze: i64,
    pub currency: &'static str,
}
impl PaymentView {
    fn from_attempt(a: &PaymentAttempt) -> Self {
        Self {
            id: a.id.clone(),
            order_id: a.id.clone(),
            state: a.state.clone(),
            amount_grosze: a.amount_grosze,
            currency: "pln",
            url: a
                .provider_response
                .as_ref()
                .filter(|_| a.state == "open")
                .and_then(|v| v["url"].as_str())
                .map(str::to_owned),
        }
    }
}
fn normalized_error((status, Json(mut value)): Error) -> Error {
    if value.get("code").is_none() {
        value["code"] = json!("request_failed");
    }
    if value.get("field_errors").is_none() {
        value["field_errors"] = json!({});
    }
    (status, Json(value))
}
pub(super) async fn identity(app: &App, headers: &HeaderMap) -> Result<(String, Option<String>)> {
    super::cart::identity(app.db.as_ref(), headers)
        .await
        .map_err(normalized_error)
}
pub(super) async fn mutation(auth: &super::auth::Auth, headers: &HeaderMap) -> Result<()> {
    if super::auth::session_cookie(headers)
        .map_err(normalized_error)?
        .is_some()
    {
        auth.mutation(headers).await.map_err(normalized_error)?;
    }
    Ok(())
}
pub(super) async fn owned(app: &App, id: &str, headers: &HeaderMap) -> Result<CheckoutDraft> {
    let owner = identity(app, headers).await?.0;
    app.db()?
        .checkout_get(id, &owner, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)
}
async fn view(app: &App, headers: &HeaderMap, draft: CheckoutDraft) -> Result<DraftView> {
    let mut input: DraftInput = serde_json::from_value(draft.input).map_err(|_| unavailable())?;
    let missing_fields = input
        .recipient
        .normalize_draft(&input.delivery)
        .map_err(validation)?;
    let owner = identity(app, headers).await?.0;
    let payment = app
        .db()?
        .checkout_attempt_for_draft(&draft.id, &owner)
        .await
        .map_err(|_| unavailable())?
        .as_ref()
        .map(PaymentView::from_attempt);
    let quote = draft.quote.map(|mut q| {
        if let Some(o) = q.as_object_mut() {
            for k in [
                "quote_ids",
                "contact",
                "lines",
                "guest_updates",
                "_source_version",
            ] {
                o.remove(k);
            }
        }
        q
    });
    Ok(DraftView {
        id: draft.id,
        version: draft.version,
        input,
        missing_fields,
        quote,
        expires_at: draft.expires_at,
        locked: draft.locked,
        payment,
    })
}
pub(super) async fn create_draft(
    app: &App,
    headers: &HeaderMap,
    mut input: DraftInput,
) -> Result<CheckoutDraft> {
    input
        .recipient
        .normalize_draft(&input.delivery)
        .map_err(validation)?;
    // Validate owned server snapshots even while the address is still incomplete.
    super::cart::prepare(app.db.as_ref(), headers, input.parts.clone(), input.manual).await?;
    let (owner, user) = identity(app, headers).await?;
    app.db()?
        .checkout_create(
            &owner,
            user.as_deref(),
            &serde_json::to_value(input).map_err(|_| unavailable())?,
            now(),
        )
        .await
        .map_err(|_| unavailable())
}
#[utoipa::path(post,path="/api/checkout/drafts",tag="Checkout",operation_id="checkout_draft_create",request_body=DraftInput,
 responses((status=201,body=DraftView),(status=401,body=Problem),(status=422,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn create(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Json(input): Json<DraftInput>,
) -> Result<(StatusCode, Json<DraftView>)> {
    mutation(&auth, &headers).await?;
    let draft = create_draft(&app, &headers, input).await?;
    Ok((
        StatusCode::CREATED,
        Json(view(&app, &headers, draft).await?),
    ))
}
#[utoipa::path(get,path="/api/checkout/drafts/{id}",tag="Checkout",operation_id="checkout_draft_get",params(("id"=String,Path)),
 responses((status=200,body=DraftView),(status=404,body=Problem)),security(("session"=[]),("guest"=[])))]
async fn get(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<DraftView>> {
    let draft = owned(&app, &id, &headers).await?;
    Ok(Json(view(&app, &headers, draft).await?))
}
#[utoipa::path(patch,path="/api/checkout/drafts/{id}",tag="Checkout",operation_id="checkout_draft_update",params(("id"=String,Path)),request_body=DraftPatch,
 responses((status=200,body=DraftView),(status=404,body=Problem),(status=409,body=Problem),(status=422,body=Problem)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn update(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<DraftPatch>,
) -> Result<Json<DraftView>> {
    mutation(&auth, &headers).await?;
    let draft = owned(&app, &id, &headers).await?;
    if draft.locked || draft.version != patch.version {
        return Err(conflict());
    }
    let mut input: DraftInput = serde_json::from_value(draft.input).map_err(|_| unavailable())?;
    if let Some(p) = patch.parts {
        input.parts = p;
    }
    if let Some(r) = patch.recipient {
        input.recipient = r;
    }
    if let Some(d) = patch.delivery {
        input.delivery = d;
    }
    if let Some(m) = patch.manual {
        input.manual = m;
    }
    if let Some(e) = patch.email_order_updates {
        input.email_order_updates = e;
    }
    input
        .recipient
        .normalize_draft(&input.delivery)
        .map_err(validation)?;
    super::cart::prepare(app.db.as_ref(), &headers, input.parts.clone(), input.manual).await?;
    let owner = identity(&app, &headers).await?.0;
    let draft = app
        .db()?
        .checkout_update(
            &id,
            &owner,
            patch.version,
            &serde_json::to_value(input).map_err(|_| unavailable())?,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(conflict)?;
    Ok(Json(view(&app, &headers, draft).await?))
}
#[derive(Serialize, utoipa::ToSchema)]
struct Points {
    points: Vec<rapidmaker_shipping::Point>,
}
#[utoipa::path(get,path="/api/checkout/drafts/{id}/points",tag="Dostawa",operation_id="checkout_draft_points",params(("id"=String,Path)),
 responses((status=200,body=Points),(status=422,body=Problem),(status=503,body=Problem)),security(("session"=[]),("guest"=[])))]
async fn points(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Points>> {
    let draft = owned(&app, &id, &headers).await?;
    let input: DraftInput = serde_json::from_value(draft.input).map_err(|_| unavailable())?;
    let query = if !input.recipient.postal_code.is_empty() {
        &input.recipient.postal_code
    } else {
        &input.recipient.city
    };
    if query.is_empty() {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing_location",
            "Uzupełnij kod pocztowy lub miasto odbiorcy.",
        ));
    }
    let owner = identity(&app, &headers).await?.0;
    app.provider_limit(&owner).await?;
    let _permit = app.permit()?;
    let points = app
        .shipping
        .as_ref()
        .ok_or_else(unavailable)?
        .points(query)
        .await
        .map_err(shipping_error)?;
    Ok(Json(Points { points }))
}
pub(super) fn contact(input: &DraftInput, point: Option<&rapidmaker_shipping::Point>) -> Value {
    let r = &input.recipient;
    let street = if r.apartment.is_empty() {
        r.street.clone()
    } else {
        format!("{} / {}", r.street, r.apartment)
    };
    let mut v = json!({"name":r.name,"email":r.email,"phone":r.phone,"street":street,"city":r.city,"postalCode":r.postal_code,"country":"PL","invoice":r.invoice,"emailOrderUpdates":input.email_order_updates,"pickupPoint":input.delivery.point_id});
    if let Some(p) = point {
        v["delivery_address"] = json!(p.address);
    }
    v
}
// A retry of the request that produced this quote may carry its previous version.
// Any actual input edit clears the quote and increments the version in PostgreSQL.
fn reusable_quote(draft: &CheckoutDraft, version: i64, at: i64) -> bool {
    !draft.locked
        && draft.expires_at > at
        && draft.quote_expires_at.is_some_and(|expiry| expiry > at)
        && draft.quote.as_ref().is_some_and(|quote| {
            quote["manual_required"] == false
                && quote["offers"]
                    .as_array()
                    .is_some_and(|offers| !offers.is_empty())
                && (draft.version == version
                    || (version.checked_add(1) == Some(draft.version)
                        && quote["_source_version"].as_i64() == Some(version)))
        })
}
pub(super) async fn quote_draft(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    version: i64,
    allow_incomplete: bool,
) -> Result<CheckoutDraft> {
    let draft = owned(app, id, headers).await?;
    let reuse = reusable_quote(&draft, version, now());
    if draft.locked || (draft.version != version && !reuse) {
        return Err(conflict());
    }
    let mut input: DraftInput =
        serde_json::from_value(draft.input.clone()).map_err(|_| unavailable())?;
    let incomplete = input.recipient.normalize(&input.delivery).err();
    if !allow_incomplete && let Some(v) = &incomplete {
        return Err(validation(v.clone()));
    }
    let cart =
        super::cart::prepare(app.db.as_ref(), headers, input.parts.clone(), input.manual).await?;
    // Even a cache hit must recheck owned models, current rates and stock.
    if reuse {
        return if reusable_quote(&draft, version, now()) {
            Ok(draft)
        } else {
            Err(conflict())
        };
    }
    let mut plans = rapidmaker_packing::candidates(EstimateRequest {
        parts: cart.parts,
        delivery: match input.delivery.method {
            Method::Courier => Delivery::Courier,
            Method::Locker => Delivery::Locker,
            Method::Pickup => Delivery::Pickup,
        },
    });
    for plan in &mut plans {
        plan.geometry_source = "parsed_model_slicer_mass_upper_bound";
    }
    let manual = cart.manual || plans[0].status != "estimated";
    let mut selected_plan = 0;
    let shipping = if incomplete.is_some() {
        rapidmaker_shipping::Quote {
            offers: vec![],
            point: None,
            manual_required: false,
            expires_at: None,
            reason: Some("Uzupełnij dane odbiorcy, aby wybrać dostawę.".into()),
        }
    } else if manual {
        rapidmaker_shipping::Quote {
            offers: vec![],
            point: None,
            manual_required: true,
            expires_at: None,
            reason: Some("Zamówienie wymaga indywidualnej wyceny.".into()),
        }
    } else if input.delivery.method == Method::Pickup {
        rapidmaker_shipping::Quote::pickup()
    } else {
        app.provider_limit(&cart.owner).await?;
        let (plan, quote) = app
            .shipping
            .as_ref()
            .ok_or_else(unavailable)?
            .quote_best(&plans, &input.recipient, &input.delivery)
            .await
            .map_err(shipping_error)?;
        selected_plan = plan;
        quote
    };
    let packing = plans.swap_remove(selected_plan);
    let subtotal = cart
        .total
        .checked_add(packing.packaging_grosze)
        .ok_or_else(conflict)?;
    let mut offers = Vec::new();
    for o in shipping.offers {
        let total = subtotal
            .checked_add(o.price_grosze)
            .filter(|n| *n > 0 && *n <= 99_999_999)
            .ok_or_else(conflict)?;
        offers.push(CheckoutOffer {
            id: o.id,
            name: o.name,
            price_grosze: o.price_grosze,
            total_grosze: total,
            recommended: false,
        });
    }
    offers.sort_by_key(|o| o.total_grosze);
    if let Some(o) = offers.first_mut() {
        o.recommended = true;
    }
    let lines = serde_json::to_value(&cart.lines).map_err(|_| unavailable())?;
    let contact = contact(&input, shipping.point.as_ref());
    let expires_at = (cart.expires_at as i64)
        .min(now() + 600)
        .min(shipping.expires_at.unwrap_or(i64::MAX));
    if expires_at <= now() {
        return Err(conflict());
    }
    let details = QuoteDetails {
        packing,
        offers,
        print_grosze: cart.total,
        print_lines: cart.lines,
        setup_grosze: cart.setup,
        packaging_grosze: subtotal - cart.total,
        expires_at,
        manual_required: shipping.manual_required,
        reason: shipping.reason,
        point: shipping.point,
    };
    let mut value = serde_json::to_value(details).map_err(|_| unavailable())?;
    value["_source_version"] = json!(version);
    value["quote_ids"] = json!(cart.quote_ids);
    value["contact"] = contact;
    value["lines"] = lines;
    value["guest_updates"] = json!(input.email_order_updates);
    if let Some(saved) = app
        .db()?
        .checkout_quote(id, &cart.owner, version, &value, expires_at, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Ok(saved);
    }
    // Another identical request may have committed the shared provider result first.
    let saved = owned(app, id, headers).await?;
    if reusable_quote(&saved, version, now()) {
        Ok(saved)
    } else {
        Err(conflict())
    }
}
#[utoipa::path(post,path="/api/checkout/drafts/{id}/quote",tag="Checkout",operation_id="checkout_draft_quote",params(("id"=String,Path)),request_body=Version,
 responses((status=200,body=DraftView),(status=409,body=Problem),(status=422,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn quote(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Version>,
) -> Result<Json<DraftView>> {
    mutation(&auth, &headers).await?;
    let draft = quote_draft(&app, &headers, &id, input.version, false).await?;
    Ok(Json(view(&app, &headers, draft).await?))
}
pub(super) fn hash(v: &str) -> String {
    format!("{:x}", Sha256::digest(v.as_bytes()))
}
fn provider_form(
    app: &App,
    id: &str,
    draft: &str,
    kind: &str,
    total: i64,
) -> Vec<(String, String)> {
    let mut form = if kind == "session" {
        vec![
            ("mode".into(), "payment".into()),
            ("line_items[0][price_data][currency]".into(), "pln".into()),
            (
                "line_items[0][price_data][unit_amount]".into(),
                total.to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]".into(),
                "Rapidmaker — zamówienie testowe".into(),
            ),
            ("line_items[0][quantity]".into(), "1".into()),
            (
                "success_url".into(),
                format!(
                    "{}/zamowienie?checkout={draft}&stripe_session={{CHECKOUT_SESSION_ID}}",
                    app.origin
                ),
            ),
            (
                "cancel_url".into(),
                format!(
                    "{}/zamowienie?checkout={draft}&stripe_cancelled=1",
                    app.origin
                ),
            ),
        ]
    } else {
        vec![
            ("amount".into(), total.to_string()),
            ("currency".into(), "pln".into()),
        ]
    };
    form.push(("metadata[quote_id]".into(), id.into()));
    form
}
pub(super) async fn start_payment(
    app: &Arc<App>,
    headers: &HeaderMap,
    id: &str,
    input: PaymentInput,
    key: &str,
    kind: &str,
) -> Result<PaymentAttempt> {
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_idempotency_key",
            "Podaj Idempotency-Key (1–128 znaków ASCII bez spacji).",
        ));
    }
    app.stripe()?;
    let draft = owned(app, id, headers).await?;
    let owner = identity(app, headers).await?.0;
    let quote = draft.quote.as_ref().ok_or_else(conflict)?;
    let total = quote["offers"]
        .as_array()
        .and_then(|a| a.iter().find(|o| o["id"] == input.offer_id))
        .and_then(|v| v["total_grosze"].as_i64())
        .ok_or_else(conflict)?;
    if !draft.locked {
        let ids: Vec<String> =
            serde_json::from_value(quote["quote_ids"].clone()).map_err(|_| conflict())?;
        super::cart::validate(app.db.as_ref().ok_or_else(unavailable)?, &owner, &ids).await?;
    }
    let attempt_id = uuid::Uuid::new_v4().to_string();
    let fingerprint = hash(
        &json!({"draft":id,"version":input.version,"offer":input.offer_id,"kind":kind}).to_string(),
    );
    let mut form = provider_form(app, &attempt_id, id, kind, total);
    if kind == "session" {
        form.push((
            "customer_email".into(),
            quote["contact"]["email"]
                .as_str()
                .ok_or_else(conflict)?
                .into(),
        ));
    }
    let form = serde_json::to_value(form).map_err(|_| unavailable())?;
    let attempt = app
        .db()?
        .checkout_prepare(
            &AttemptInput {
                id: &attempt_id,
                draft_id: id,
                owner_hash: &owner,
                version: input.version,
                offer_id: &input.offer_id,
                client_key_hash: &hash(key),
                fingerprint: &fingerprint,
                provider_kind: kind,
                provider_request: &form,
            },
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(conflict)?;
    super::checkout_worker::process_attempt(app, &attempt.id).await;
    app.db()?
        .checkout_attempt(&attempt.id, &owner)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)
}
#[utoipa::path(post,path="/api/checkout/drafts/{id}/payment",tag="Checkout",operation_id="checkout_draft_payment",params(("id"=String,Path),("Idempotency-Key"=String,Header)),request_body=PaymentInput,
 responses((status=200,body=PaymentView),(status=202,body=PaymentView),(status=409,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[]),("guest"=[])))]
async fn payment(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<PaymentInput>,
) -> Result<(StatusCode, Json<PaymentView>)> {
    mutation(&auth, &headers).await?;
    let key = headers
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let attempt = start_payment(&app, &headers, &id, input, key, "session").await?;
    let status = if matches!(attempt.state.as_str(), "open" | "paid") {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    Ok((status, Json(PaymentView::from_attempt(&attempt))))
}
type CheckoutRoutes = (
    OpenApiRouter,
    OpenApiRouter,
    Option<super::checkout_worker::CheckoutWorker>,
    Option<super::shipment_worker::ShipmentWorker>,
);
pub fn router(
    db: Option<Storage>,
) -> std::result::Result<CheckoutRoutes, Box<dyn std::error::Error + Send + Sync>> {
    let app = App::new(db)?;
    let worker = app
        .db
        .as_ref()
        .filter(|_| app.stripe.is_some())
        .map(|_| super::checkout_worker::CheckoutWorker::start(app.clone()));
    let router = OpenApiRouter::new()
        .routes(routes!(create))
        .routes(routes!(get, update))
        .routes(routes!(points))
        .routes(routes!(quote))
        .routes(routes!(payment))
        .merge(super::checkout_legacy::router())
        .merge(super::refunds::router())
        .merge(super::shipments::router())
        .merge(super::shipment_actions::router())
        .with_state(app.clone());
    let shipping_worker = app
        .db
        .as_ref()
        .filter(|_| app.shipping.is_some())
        .map(|_| super::shipment_worker::ShipmentWorker::start(app.clone()));
    let webhook = OpenApiRouter::new()
        .routes(routes!(super::checkout_worker::webhook))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(app);
    Ok((router, webhook, worker, shipping_worker))
}

pub(super) fn local_authority(authority: &str) -> bool {
    [
        "127.0.0.1:18430",
        "127.0.0.1:18431",
        "127.0.0.1:18432",
        "localhost:18430",
        "localhost:18431",
        "localhost:18432",
    ]
    .contains(&authority)
}
async fn local_only(
    State(auth): State<Arc<super::auth::Auth>>,
    request: Request,
    next: Next,
) -> Response {
    if !auth.allows_request(
        request.headers(),
        !matches!(
            *request.method(),
            axum::http::Method::GET | axum::http::Method::HEAD
        ),
    ) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden_origin",
            "Dozwolone wyłącznie lokalne środowisko testowe.",
        )
        .into_response();
    }
    let mut response = next.run(request).await;
    if auth.production() {
        response.headers_mut().insert(
            header::STRICT_TRANSPORT_SECURITY,
            axum::http::HeaderValue::from_static("max-age=31536000"),
        );
    }
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    if response.status().is_client_error()
        && response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_none_or(|v| v != "application/json")
    {
        return error(
            response.status(),
            "invalid_request",
            "Nieprawidłowe żądanie. Sprawdź adres API, format JSON i dopuszczalne pola.",
        )
        .into_response();
    }
    response
}
pub fn protect(router: axum::Router, auth: Arc<super::auth::Auth>) -> axum::Router {
    router
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(auth, local_only))
}

pub(super) use super::checkout_worker::refresh_order;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reusable_quote_requires_unexpired_snapshot_and_exact_input_version() {
        let mut draft = CheckoutDraft {
            id: "draft".into(),
            version: 3,
            input: json!({}),
            quote: Some(
                json!({"_source_version":2,"offers":[{"id":"service"}],"manual_required":false}),
            ),
            quote_expires_at: Some(200),
            expires_at: 1000,
            locked: false,
        };
        assert!(reusable_quote(&draft, 3, 100));
        assert!(reusable_quote(&draft, 2, 100));
        assert!(!reusable_quote(&draft, 1, 100));
        assert!(!reusable_quote(&draft, 2, 200));
        draft.locked = true;
        assert!(!reusable_quote(&draft, 3, 100));
        draft.locked = false;
        draft.version += 1;
        assert!(!reusable_quote(&draft, 2, 100));
        draft.quote = None;
        assert!(!reusable_quote(&draft, 4, 100));
        assert!(!reusable_quote(&draft, i64::MAX, 100));
    }
    #[test]
    fn stripe_form_freezes_server_amount_and_never_recollects_shipping_address() {
        let app = App {
            db: None,
            stripe: None,
            shipping: None,
            webhook_secret: None,
            origin: "http://localhost:18430".into(),
            publishable_key: None,
            calls: Arc::new(Semaphore::new(4)),
        };
        let form = provider_form(&app, "attempt-id", "draft-id", "session", 12345);
        let values: BTreeMap<_, _> = form.into_iter().collect();
        assert_eq!(values["line_items[0][price_data][unit_amount]"], "12345");
        assert_eq!(values["metadata[quote_id]"], "attempt-id");
        assert_eq!(values["line_items[0][price_data][currency]"], "pln");
        assert!(
            !values
                .keys()
                .any(|k| k.starts_with("shipping_address_collection"))
        );
        assert!(!values.contains_key("payment_method_types[0]"));
        assert!(values["success_url"].starts_with("http://localhost:18430/"));
    }
}
