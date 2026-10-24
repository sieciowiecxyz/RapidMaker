//! Operator approval of measured parcels and cost; provider writes run after durable booking.
use super::{
    auth::Auth,
    checkout::{App, Problem, Result, error, now, unavailable},
};
use axum::{
    Extension, Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use rapidmaker_shipping::{
    DeliveryChoice, MeasuredParcel, Recipient, ShipQuote, ShipmentError as ProviderError,
    matches_prepared,
};
use rapidmaker_storage::{Shipment, ShipmentError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Prepare {
    /// Zero creates a draft; subsequent writes require its current version.
    version: i64,
    parcels: Vec<MeasuredParcel>,
    #[serde(default)]
    refund_reviewed: bool,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Version {
    version: i64,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Book {
    version: i64,
    request_id: String,
    quote_id: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Handover {
    package_ids: Vec<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Recover {
    package_id: String,
    provider_id: String,
}
#[derive(Deserialize, utoipa::IntoParams)]
#[serde(default, deny_unknown_fields)]
struct Label {
    format: String,
}
impl Default for Label {
    fn default() -> Self {
        Self {
            format: "a6".into(),
        }
    }
}

pub(super) fn router() -> OpenApiRouter<Arc<App>> {
    OpenApiRouter::new()
        .routes(routes!(get, prepare))
        .routes(routes!(quote))
        .routes(routes!(book))
        .routes(routes!(label))
        .routes(routes!(handover))
        .routes(routes!(recover))
        .routes(routes!(reopen))
        .routes(routes!(account))
}
pub(super) fn domain(e: ShipmentError) -> super::checkout::Error {
    let (status, code, message) = match e {
        ShipmentError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "Nie znaleziono zamówienia lub przygotowanej dostawy.",
        ),
        ShipmentError::Forbidden => (
            StatusCode::FORBIDDEN,
            "forbidden",
            "Brak uprawnień operatora.",
        ),
        ShipmentError::Conflict => (
            StatusCode::CONFLICT,
            "shipment_changed",
            "Dostawa lub akceptacja kosztu uległa zmianie. Pobierz aktualne dane.",
        ),
        ShipmentError::NotReady => (
            StatusCode::CONFLICT,
            "not_ready",
            "Nadanie wymaga opłaconego zamówienia gotowego do wysyłki.",
        ),
        ShipmentError::Refund => (
            StatusCode::CONFLICT,
            "refund_review",
            "Najpierw rozstrzygnij zwrot i potwierdź zakres wysyłki.",
        ),
        ShipmentError::Invalid => (
            StatusCode::BAD_REQUEST,
            "invalid_shipment",
            "Podaj prawidłowe wymiary/masę paczek i wersję. Odbiór osobisty nie wymaga nadania.",
        ),
    };
    error(status, code, message)
}
pub(super) fn provider_error(e: ProviderError) -> super::checkout::Error {
    match e {
        ProviderError::Invalid(message) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "shipping_rejected",
            &message,
        ),
        ProviderError::Unauthorized => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "shipping_account",
            "Sprawdź token i uprawnienia konta Furgonetki.",
        ),
        ProviderError::NotFound => error(
            StatusCode::CONFLICT,
            "shipping_not_found",
            "Nie znaleziono operacji u przewoźnika. Wymaga uzgodnienia przez operatora.",
        ),
        _ => unavailable(),
    }
}
pub(super) async fn stored(app: &App, id: &str) -> Result<Shipment> {
    app.db()?
        .shipment_get(id)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| domain(ShipmentError::NotFound))
}

#[utoipa::path(get,path="/api/admin/orders/{id}/shipment",operation_id="shipment_admin_get",tag="Admin",
 summary="Dane zamówienia, szacowane paczki i aktualny stan nadania.",params(("id"=String,Path)),responses((status=200,body=Value),(status=401,body=Problem),(status=403,body=Problem),(status=404,body=Problem)),security(("session"=[])))]
async fn get(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    auth.administrator(&headers, false).await?;
    let source = app
        .db()?
        .shipment_source(&id)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| domain(ShipmentError::NotFound))?;
    Ok(Json(
        json!({"order":source,"shipment":app.db()?.shipment_get(&id).await.map_err(|_|unavailable())?}),
    ))
}
#[utoipa::path(put,path="/api/admin/orders/{id}/shipment",operation_id="shipment_prepare",tag="Admin",
 summary="Potwierdź rzeczywiste paczki; nie zamawia przewozu.",params(("id"=String,Path)),request_body=Prepare,
 responses((status=200,body=Shipment),(status=400,body=Problem),(status=403,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn prepare(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Prepare>,
) -> Result<Json<Shipment>> {
    let user = auth.administrator(&headers, true).await?.user;
    let parcels = serde_json::to_value(input.parcels).map_err(|_| unavailable())?;
    Ok(Json(
        app.db()?
            .shipment_save(
                &id,
                input.version,
                &parcels,
                input.refund_reviewed,
                &user.id,
                now(),
            )
            .await
            .map_err(|_| unavailable())?
            .map_err(domain)?,
    ))
}
pub(super) fn request(
    s: &Shipment,
) -> Result<(Recipient, DeliveryChoice, String, Vec<MeasuredParcel>)> {
    let c = &s.input["contact"];
    let text = |key: &str| c[key].as_str().unwrap_or_default().to_owned();
    let recipient = Recipient {
        name: text("name"),
        email: text("email"),
        phone: text("phone"),
        street: text("street"),
        city: text("city"),
        country: "PL".into(),
        postal_code: c["postalCode"]
            .as_str()
            .or_else(|| c["postal_code"].as_str())
            .unwrap_or_default()
            .into(),
        ..Default::default()
    };
    let f = &s.input["fulfillment"];
    let delivery = if f["delivery"].is_string() {
        json!({"method":f["delivery"],"point_id":c["pickupPoint"]})
    } else {
        f["delivery"].clone()
    };
    let choice: DeliveryChoice =
        serde_json::from_value(delivery).map_err(|_| domain(ShipmentError::Invalid))?;
    let service = f["offer"]["id"]
        .as_str()
        .ok_or_else(|| domain(ShipmentError::Invalid))?
        .to_owned();
    let parcels = serde_json::from_value(s.input["parcels"].clone())
        .map_err(|_| domain(ShipmentError::Invalid))?;
    Ok((recipient, choice, service, parcels))
}
pub(super) async fn current_quote(app: &App, s: &Shipment) -> Result<ShipQuote> {
    let (recipient, choice, service, parcels) = request(s)?;
    let shipping = app.shipping.as_ref().ok_or_else(unavailable)?;
    let reference = format!(
        "RM{}",
        s.id.replace('-', "").chars().take(16).collect::<String>()
    );
    let _permit = app.permit()?;
    tokio::time::timeout(
        Duration::from_secs(60),
        shipping.quote_shipment(&recipient, &choice, &service, &parcels, &reference),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(provider_error)
}
fn quote_json(s: &Shipment, quote: ShipQuote) -> Value {
    let paid = s.input["fulfillment"]["offer"]["price_grosze"]
        .as_i64()
        .unwrap_or(0);
    json!({"id":uuid::Uuid::new_v4().to_string(),"expires_at":now()+120,"price_grosze":quote.price_grosze,"collected_shipping_grosze":paid,"difference_grosze":i64::from(quote.price_grosze)-paid,"packaging_grosze":s.input["fulfillment"]["packaging_grosze"],"packages":quote.packages})
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/quote",operation_id="shipment_quote",tag="Admin",
 summary="Sprawdź aktualny koszt wybranej usługi dla potwierdzonych paczek.",params(("id"=String,Path)),request_body=Version,
 responses((status=200,body=Shipment),(status=403,body=Problem),(status=409,body=Problem),(status=422,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn quote(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Version>,
) -> Result<Json<Shipment>> {
    let user = auth.administrator(&headers, true).await?.user;
    app.provider_limit(&super::checkout::hash(&user.id)).await?;
    let s = stored(&app, &id).await?;
    if s.version != input.version || s.state != "draft" {
        return Err(domain(ShipmentError::Conflict));
    }
    let value = quote_json(&s, current_quote(&app, &s).await?);
    if !app
        .db()?
        .shipment_quote_save(&id, s.version, &value, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(domain(ShipmentError::Conflict));
    }
    Ok(Json(stored(&app, &id).await?))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/book",operation_id="shipment_book",tag="Admin",
 summary="Zatwierdź koszt i zleć nadanie; 202 oznacza zapis, nie potwierdzenie nadania.",
 description="Sandbox. Płatne zamówienie przesyłek bez zamówienia podjazdu kuriera. Powtórz identyczny UUID request_id po utracie odpowiedzi.",params(("id"=String,Path)),request_body=Book,
 responses((status=202,body=Shipment),(status=400,body=Problem),(status=403,body=Problem),(status=409,body=Problem),(status=422,body=Problem),(status=503,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn book(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Book>,
) -> Result<(StatusCode, Json<Shipment>)> {
    let user = auth.administrator(&headers, true).await?.user;
    let request = uuid::Uuid::parse_str(&input.request_id)
        .map_err(|_| domain(ShipmentError::Invalid))?
        .to_string();
    let s = stored(&app, &id).await?;
    if s.request_id.as_deref() != Some(&request) {
        if s.version != input.version || s.state != "draft" {
            return Err(domain(ShipmentError::Conflict));
        }
        let accepted = s
            .quote
            .as_ref()
            .filter(|q| q["id"] == input.quote_id)
            .ok_or_else(|| domain(ShipmentError::Conflict))?;
        app.provider_limit(&super::checkout::hash(&user.id)).await?;
        let fresh = quote_json(&s, current_quote(&app, &s).await?);
        if accepted["expires_at"].as_i64().is_none_or(|t| t <= now())
            || accepted["price_grosze"] != fresh["price_grosze"]
            || accepted["packages"] != fresh["packages"]
        {
            app.db()?
                .shipment_quote_save(&id, s.version, &fresh, now())
                .await
                .map_err(|_| unavailable())?;
            return Err(error(
                StatusCode::CONFLICT,
                "shipping_price_changed",
                "Koszt lub dane oferty zmieniły się. Pobierz dostawę i zatwierdź aktualną ofertę.",
            ));
        }
    }
    let result = app
        .db()?
        .shipment_book(
            &id,
            input.version,
            &request,
            &input.quote_id,
            &user.id,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .map_err(domain)?;
    Ok((StatusCode::ACCEPTED, Json(result)))
}
#[utoipa::path(get,path="/api/admin/orders/{id}/shipment/labels/{package_id}",operation_id="shipment_label",tag="Admin",
 summary="Pobierz prywatną etykietę PDF A6 lub A4.",params(("id"=String,Path),("package_id"=String,Path),Label),
 responses((status=200,content_type="application/pdf",body=String),(status=403,body=Problem),(status=409,body=Problem)),security(("session"=[])))]
async fn label(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path((id, package)): Path<(String, String)>,
    Query(input): Query<Label>,
) -> Result<Response> {
    auth.administrator(&headers, false).await?;
    if !matches!(input.format.as_str(), "a6" | "a4") {
        return Err(domain(ShipmentError::Invalid));
    }
    let s = stored(&app, &id).await?;
    if app
        .db()?
        .shipment_action_busy(&id)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(domain(ShipmentError::Conflict));
    }
    let p = s
        .packages
        .iter()
        .find(|p| {
            p.id == package && matches!(p.state.as_str(), "booked" | "dispatched" | "delivered")
        })
        .ok_or_else(|| domain(ShipmentError::Conflict))?;
    let _permit = app.permit()?;
    let data = app
        .shipping
        .as_ref()
        .ok_or_else(unavailable)?
        .shipment_label(
            p.provider_id
                .as_deref()
                .ok_or_else(|| domain(ShipmentError::Conflict))?,
            &input.format,
        )
        .await
        .map_err(provider_error)?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=rapidmaker-label.pdf",
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(data))
        .map_err(|_| unavailable())
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/handover",operation_id="shipment_handover",tag="Admin",
 summary="Potwierdź rzeczywiste przekazanie wskazanych przesyłek przewoźnikowi.",params(("id"=String,Path)),request_body=Handover,
 responses((status=200,body=Shipment),(status=403,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn handover(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Handover>,
) -> Result<Json<Shipment>> {
    let user = auth.administrator(&headers, true).await?.user;
    Ok(Json(
        app.db()?
            .shipment_handover(&id, &input.package_ids, &user.id, now())
            .await
            .map_err(|_| unavailable())?
            .map_err(domain)?,
    ))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/reconcile",operation_id="shipment_recover",tag="Admin",
 summary="Odzyskaj ID dodanej przesyłki po niepewnym POST; wymaga zgodnego dowodu Furgonetki.",params(("id"=String,Path)),request_body=Recover,
 responses((status=200,body=Shipment),(status=403,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn recover(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Recover>,
) -> Result<Json<Shipment>> {
    let user = auth.administrator(&headers, true).await?.user;
    let s = stored(&app, &id).await?;
    let p = s
        .packages
        .iter()
        .find(|p| p.id == input.package_id)
        .ok_or_else(|| domain(ShipmentError::NotFound))?;
    let _permit = app.permit()?;
    let actual = app
        .shipping
        .as_ref()
        .ok_or_else(unavailable)?
        .get_shipment(&input.provider_id)
        .await
        .map_err(provider_error)?;
    if actual.state != "waiting" || !matches_prepared(&actual, &p.payload) {
        return Err(domain(ShipmentError::Conflict));
    }
    if !app
        .db()?
        .shipment_recover(&id, &p.id, &actual.provider_id, &user.id, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(domain(ShipmentError::Conflict));
    }
    Ok(Json(stored(&app, &id).await?))
}
#[utoipa::path(get,path="/api/account/orders/{id}/shipment",operation_id="shipment_account_get",tag="Zamówienia",
 summary="Stan własnej dostawy z zapisanej historii; bez wywołań przewoźnika.",params(("id"=String,Path)),
 responses((status=200,body=Value),(status=401,body=Problem),(status=404,body=Problem)),security(("session"=[]),("guest"=[])))]
async fn account(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>> {
    let (user, owner) = super::orders::scope(&auth, &headers, false).await?;
    app.db()?
        .order(&id, user.as_deref(), owner.as_deref())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| domain(ShipmentError::NotFound))?;
    let s = app
        .db()?
        .shipment_get(&id)
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(match s {
        None => json!({"state":"not_prepared","packages":[]}),
        Some(s) => {
            json!({"state":s.state,"packages":s.packages.iter().map(|p|json!({"id":p.id,"state":p.state,"tracking":p.tracking})).collect::<Vec<_>>()})
        }
    }))
}

#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/reopen",operation_id="shipment_reopen",tag="Admin",
 summary="Odblokuj ponowną akceptację kosztu po pewnym odrzuceniu; nie tworzy nowych przesyłek.",
 description="Każda istniejąca przesyłka musi nadal oczekiwać na zamówienie, a komenda nie może istnieć. Nie odblokowuje niepewnego wyniku płatnego zamówienia.",params(("id"=String,Path)),request_body=Version,
 responses((status=200,body=Shipment),(status=403,body=Problem),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn reopen(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Version>,
) -> Result<Json<Shipment>> {
    let user = auth.administrator(&headers, true).await?.user;
    let s = stored(&app, &id).await?;
    if s.version != input.version || s.state != "manual_review" || s.packages.is_empty() {
        return Err(domain(ShipmentError::Conflict));
    }
    app.provider_limit(&super::checkout::hash(&user.id)).await?;
    let api = app.shipping.as_ref().ok_or_else(unavailable)?;
    let _permit = app.permit()?;
    match api.shipment_command(&s.command_uuid).await {
        Err(ProviderError::NotFound) => {}
        Err(e) => return Err(provider_error(e)),
        Ok(_) => return Err(domain(ShipmentError::Conflict)),
    }
    for p in &s.packages {
        let actual = api
            .get_shipment(
                p.provider_id
                    .as_deref()
                    .ok_or_else(|| domain(ShipmentError::Conflict))?,
            )
            .await
            .map_err(provider_error)?;
        if actual.state != "waiting" || !matches_prepared(&actual, &p.payload) {
            return Err(domain(ShipmentError::Conflict));
        }
    }
    Ok(Json(
        app.db()?
            .shipment_reopen(&id, input.version, &user.id, now())
            .await
            .map_err(|_| unavailable())?
            .map_err(domain)?,
    ))
}
