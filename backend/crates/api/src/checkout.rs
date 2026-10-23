use axum::{
    Json,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rapidmaker_packing::{Delivery, Estimate, EstimateRequest, estimate};
use rapidmaker_pricing::PrintLine;
use rapidmaker_storage::Storage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use utoipa_axum::{router::OpenApiRouter, routes};

const FURGONETKA: &str = "https://api.sandbox.furgonetka.pl";
const STRIPE: &str = "https://api.stripe.com/v1/checkout/sessions";
const STRIPE_INTENTS: &str = "https://api.stripe.com/v1/payment_intents";
const TTL: u64 = 600;
const MAX_QUOTES: usize = 128;
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message":message})))
}
fn unavailable() -> Error {
    error(
        StatusCode::BAD_GATEWAY,
        "Dostawca testowy jest chwilowo niedostępny. Spróbuj ponownie.",
    )
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Default, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(default, deny_unknown_fields)]
#[schema(as = CheckoutContact)]
struct Contact {
    #[serde(rename = "emailOrderUpdates", default = "yes")]
    email_order_updates: bool,
    name: String,
    email: String,
    phone: String,
    street: String,
    city: String,
    #[serde(rename = "postalCode")]
    postal_code: String,
    #[serde(rename = "company")]
    _company: String,
    #[serde(rename = "nip")]
    _nip: String,
    #[serde(rename = "pickupPoint")]
    pickup_point: String,
}
fn yes() -> bool {
    true
}
impl Contact {
    fn valid_identity(&self) -> bool {
        [&self.name, &self.email, &self.phone]
            .iter()
            .all(|v| !v.trim().is_empty() && v.len() <= 254 && !v.chars().any(char::is_control))
            && self.email.contains('@')
            && self.phone.len() >= 5
    }
    fn valid_locker(&self) -> bool {
        self.valid_identity()
            && !self.pickup_point.is_empty()
            && self.pickup_point.len() <= 64
            && self
                .pickup_point
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
    fn valid(&self) -> bool {
        [
            &self.name,
            &self.email,
            &self.phone,
            &self.street,
            &self.city,
            &self.postal_code,
        ]
        .iter()
        .all(|v| !v.trim().is_empty() && v.len() <= 254 && !v.chars().any(char::is_control))
            && self.email.contains('@')
            && self.phone.len() >= 5
            && self.postal_code.len() == 6
            && self.postal_code.as_bytes()[2] == b'-'
            && self
                .postal_code
                .chars()
                .enumerate()
                .all(|(i, c)| i == 2 || c.is_ascii_digit())
    }
    fn receiver(&self) -> Value {
        json!({"name":self.name,"email":self.email,"phone":self.phone,
        "street":self.street,"city":self.city,"country_code":"PL","postcode":self.postal_code})
    }
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = ShippingPointSearch)]
struct PointSearch {
    search: String,
}

async fn locker_points(app: &App, search: &str, exact: bool) -> Result<Vec<Value>, Error> {
    if !(2..=100).contains(&search.len()) || search.chars().any(char::is_control) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Podaj miejscowość, kod pocztowy lub nazwę punktu.",
        ));
    }
    let mut filters = json!({"services":["inpost"],"point_types":["delivery_points"],
        "type":"parcel_machine","country_codes":["PL"],"limit":12});
    if exact {
        filters["point_id"] = json!(search);
    }
    let postal_query = !exact
        && search.len() == 6
        && search.as_bytes()[2] == b'-'
        && search
            .chars()
            .enumerate()
            .all(|(i, c)| i == 2 || c.is_ascii_digit());
    // Explicit Polish postcode avoids treating it as a point ID or ambiguous text.
    // Furgonetka orders results by distance from the resolved area, not the user's home.
    let location = if postal_query {
        json!({"address":{"postcode":search,"country_code":"PL"},"points_max_distance":10})
    } else {
        json!({"search_phrase":search})
    };
    let token = app.carrier_token.as_ref().ok_or_else(unavailable)?;
    let data = bounded_json(
        app.client
            .post(format!("{FURGONETKA}/points/map"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.furgonetka.v1+json")
            .header("Content-Type", "application/vnd.furgonetka.v1+json")
            .json(&json!({"location":location,"filters":filters}))
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    Ok(data["points"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            p["active"] == true && p["is_delivery_point"] == true && p["service"] == "inpost"
        })
        .filter_map(|p| {
            let code = p["code"].as_str()?;
            if exact && code != search {
                return None;
            }
            Some(json!({"code":code,"name":p["name"],"address":p["address"]}))
        })
        .collect())
}
#[utoipa::path(operation_id = "checkout_points", post,
    path = "/api/shipping/points",
    tag = "Shipping",
    summary = "Wyszukaj punkty odbioru",
    request_body = PointSearch,
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna")))]
async fn points(
    State(app): State<Arc<App>>,
    Json(input): Json<PointSearch>,
) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"points":locker_points(&app,input.search.trim(),false).await?}),
    ))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = CheckoutQuoteRequest)]
struct QuoteRequest {
    parts: Vec<super::cart::CartPart>,
    delivery: Delivery,
    contact: Option<Contact>,
    #[serde(default)]
    manual: bool,
}
#[derive(Clone, Serialize, utoipa::ToSchema)]
#[schema(as = ShippingOffer)]
struct Offer {
    id: String,
    name: String,
    price_grosze: u32,
}
#[derive(Serialize, utoipa::ToSchema)]
#[schema(as = CheckoutQuote)]
struct QuoteResponse {
    id: String,
    expires_at: u64,
    packing: Estimate,
    offers: Vec<Offer>,
    print_grosze: u32,
    print_lines: Vec<PrintLine>,
    setup_grosze: u32,
    packaging_grosze: u32,
    production_demo: bool,
    manual: bool,
    stripe_configured: bool,
    furgonetka_configured: bool,
    reason: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    field_errors: HashMap<String, String>,
}
#[derive(Clone)]
struct Session {
    id: String,
    url: String,
    offer_id: String,
    total: u32,
}
#[derive(Clone)]
struct Intent {
    id: String,
    client_secret: String,
    total: u32,
}
struct StoredQuote {
    order_data: Option<Value>,
    owner: String,
    quote_ids: Vec<String>,
    expires_at: u64,
    subtotal: u32,
    offers: Vec<Offer>,
    attempted_offer: Option<String>,
    session: Option<Session>,
    intent: Option<Intent>,
    attempted_intent: bool,
}
struct App {
    storage: Option<Storage>,
    client: reqwest::Client,
    stripe_key: Option<String>,
    publishable_key: Option<String>,
    carrier_token: Option<String>,
    sender: Option<Value>,
    quotes: Mutex<HashMap<String, StoredQuote>>,
}
impl App {
    fn from_env(storage: Option<Storage>) -> Self {
        let stripe_key = std::env::var("STRIPE_SECRET_KEY")
            .or_else(|_| std::env::var("STRIPE_TEST_SECRET_KEY"))
            .ok()
            .filter(|key| test_key(key));
        let carrier_token = std::env::var("FURGONETKA_SANDBOX_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let sender = std::env::var("FURGONETKA_SENDER_JSON")
            .ok()
            .and_then(|v| serde_json::from_str::<Value>(&v).ok())
            .filter(Value::is_object);
        Self {
            storage,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("HTTP client"),
            stripe_key,
            publishable_key: std::env::var("STRIPE_PUBLISHABLE_KEY")
                .ok()
                .filter(|v| v.starts_with("pk_test_")),
            carrier_token,
            sender,
            quotes: Mutex::new(HashMap::new()),
        }
    }
    fn carrier_configured(&self) -> bool {
        self.carrier_token.is_some() && self.sender.is_some()
    }
}
fn test_key(key: &str) -> bool {
    key.starts_with("sk_test_") || key.starts_with("rk_test_")
}
async fn bounded_json(response: reqwest::Response) -> Result<Value, Error> {
    if !response.status().is_success() {
        return Err(unavailable());
    }
    response_json(response).await
}
async fn response_json(mut response: reqwest::Response) -> Result<Value, Error> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if body.len() + chunk.len() > 1_000_000 {
            return Err(unavailable());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| unavailable())
}
fn service_id(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        Some(value.to_owned())
    } else {
        value.as_u64().map(|v| v.to_string())
    }
}
fn provider_id(id: &str) -> Value {
    id.parse::<u64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::from(id))
}
fn money_grosze(value: &Value) -> Option<u32> {
    let price = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    if !price.is_finite() || price <= 0. || price > 100_000. {
        return None;
    }
    Some((price * 100.).round() as u32)
}
fn parse_offers(rates: &Value, services: &[(String, String)]) -> Vec<Offer> {
    let mut offers = vec![];
    for rate in rates["services_prices"].as_array().into_iter().flatten() {
        if rate["available"] != true {
            continue;
        }
        let Some(id) = service_id(&rate["service_id"]) else {
            continue;
        };
        let Some((_, name)) = services.iter().find(|(known, _)| *known == id) else {
            continue;
        };
        let Some(price) = money_grosze(&rate["pricing"]["price_gross"]) else {
            continue;
        };
        if !offers.iter().any(|v: &Offer| v.id == id) {
            offers.push(Offer {
                id,
                name: name.to_uppercase(),
                price_grosze: price,
            });
        }
    }
    offers.sort_by_key(|v| v.price_grosze);
    offers
}
fn carrier_rejection(body: &Value) -> Error {
    let errors: Vec<_> = body["errors"].as_array().into_iter().flatten().collect();
    if errors.iter().any(|e| {
        e["path"]
            .as_str()
            .is_some_and(|p| p.starts_with("/pickup/"))
    }) {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Dane nadawcy w konfiguracji dostawy wymagają poprawy. Skontaktuj się z obsługą; nie zmieniaj swojego adresu.",
        );
    }
    let mut fields = HashMap::new();
    for issue in errors {
        let (field, message) = match issue["path"].as_str().unwrap_or("") {
            "/receiver/street" if issue["code"] == "noNumber" => {
                ("street", "Dodaj numer budynku do ulicy.")
            }
            "/receiver/street" => ("street", "Sprawdź ulicę i numer budynku."),
            "/receiver/postcode" => ("postalCode", "Sprawdź kod pocztowy dostawy."),
            "/receiver/city" => ("city", "Sprawdź miejscowość dostawy."),
            "/receiver/phone" => ("phone", "Sprawdź numer telefonu odbiorcy."),
            "/receiver/email" => ("email", "Sprawdź adres email odbiorcy."),
            "/receiver/name" => ("name", "Sprawdź imię i nazwisko odbiorcy."),
            "/receiver/point" => ("pickupPoint", "Wybierz inny dostępny punkt odbioru."),
            _ => continue,
        };
        fields.insert(field, message);
    }
    if !fields.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"message":"Przewoźnik wymaga poprawienia wskazanych danych odbiorcy.","field_errors":fields}),
            ),
        );
    }
    error(
        StatusCode::CONFLICT,
        "Przewoźnik nie przyjmuje tych paczek lub wymaga dodatkowych danych przesyłki. Wybierz inną metodę lub poproś o indywidualną wycenę.",
    )
}
async fn carrier_json(response: reqwest::Response) -> Result<Value, Error> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Dostęp do API Furgonetki wygasł lub konto nie ma uprawnień. Obsługa musi poprawić połączenie; adres odbiorcy nie jest przyczyną.",
        )),
        StatusCode::TOO_MANY_REQUESTS => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Furgonetka ograniczyła liczbę zapytań. Spróbuj ponownie za chwilę.",
        )),
        status if status.is_success() => response_json(response).await,
        status if status.is_server_error() => Err(unavailable()),
        _ => Err(carrier_rejection(&response_json(response).await?)),
    }
}
async fn carrier_offers(
    app: &App,
    packing: &Estimate,
    contact: &Contact,
    locker: bool,
) -> Result<Vec<Offer>, Error> {
    let token = app.carrier_token.as_ref().ok_or_else(unavailable)?;
    let services = carrier_json(
        app.client
            .get(format!("{FURGONETKA}/account/services"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.furgonetka.v1+json")
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    let services: Vec<_> = services["services"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|service| {
            let name = service["service"].as_str()?;
            if if locker {
                name != "inpost"
            } else {
                !["dpd", "dhl", "ups", "gls", "fedex"].contains(&name)
            } {
                return None;
            }
            Some((service_id(&service["id"])?, name.to_owned()))
        })
        .take(10)
        .collect();
    if services.is_empty() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Konto Furgonetki nie ma skonfigurowanych usług dla tej metody dostawy. Skontaktuj się z obsługą.",
        ));
    }
    let parcels: Vec<_> = packing
        .parcels
        .iter()
        .map(|p| {
            json!({"width":p.outer_mm[0].div_ceil(10),
        "depth":p.outer_mm[1].div_ceil(10),"height":p.outer_mm[2].div_ceil(10),
        "weight":f64::from(p.weight_g)/1000.,"quantity":1,"type":"package"})
        })
        .collect();
    let mut receiver = contact.receiver();
    if locker {
        let point = locker_points(app, &contact.pickup_point, true)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                error(
                    StatusCode::CONFLICT,
                    "Punkt jest niedostępny. Wybierz inny.",
                )
            })?;
        receiver["point"] = point["code"].clone();
        for key in ["street", "city", "postcode"] {
            receiver[key] = point["address"][key].clone();
        }
    }
    let package = json!({"pickup":app.sender,"receiver":receiver,"service_id":provider_id(&services[0].0),"parcels":parcels});
    let rates=carrier_json(app.client.post(format!("{FURGONETKA}/packages/calculate-price"))
        .bearer_auth(token).header("Accept","application/vnd.furgonetka.v2+json")
        .header("Content-Type","application/vnd.furgonetka.v2+json")
        .json(&json!({"package":package,"services":{"service_id":services.iter().map(|v|provider_id(&v.0)).collect::<Vec<_>>()}}))
        .send().await.map_err(|_|unavailable())?).await?;
    let mut validated = vec![];
    let mut rejection = None;
    // Try subsequent carriers if a cheaper one rejects the parcel/address.
    // Bound requests by account services (max 10), return up to three valid offers.
    for offer in parse_offers(&rates, &services) {
        let mut package = package.clone();
        package["service_id"] = provider_id(&offer.id);
        let response = app
            .client
            .post(format!("{FURGONETKA}/packages/validate"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.furgonetka.v2+json")
            .header("Content-Type", "application/vnd.furgonetka.v2+json")
            .json(&package)
            .send()
            .await
            .map_err(|_| unavailable())?;
        if response.status() == StatusCode::NO_CONTENT {
            validated.push(offer);
        } else {
            let failure = carrier_json(response)
                .await
                .err()
                .unwrap_or_else(unavailable);
            if failure.0 == StatusCode::SERVICE_UNAVAILABLE || failure.0 == StatusCode::BAD_GATEWAY
            {
                return Err(failure);
            }
            rejection = Some(failure);
        }
        if validated.len() == 3 {
            break;
        }
    }
    if validated.is_empty() {
        if let Some(failure) = rejection {
            return Err(failure);
        }
        for rate in rates["services_prices"].as_array().into_iter().flatten() {
            if rate["errors"]
                .as_array()
                .is_some_and(|errors| !errors.is_empty())
            {
                return Err(carrier_rejection(rate));
            }
        }
    }
    Ok(validated)
}
#[utoipa::path(operation_id = "checkout_quote", post,
    path = "/api/checkout/quote",
    tag = "Checkout",
    summary = "Wyceń dostawę i zamówienie",
    request_body = QuoteRequest,
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = QuoteResponse), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan"), (status = 422, body = Value, description = "Pola odbiorcy wymagają poprawy"), (status = 429, body = Value, description = "Limit ofert")))]
async fn quote(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(request): Json<QuoteRequest>,
) -> Result<Json<QuoteResponse>, Error> {
    let cart = super::cart::prepare(
        app.storage.as_ref(),
        &headers,
        request.parts,
        request.manual,
    )
    .await?;
    let delivery = request.delivery;
    let mut packing = estimate(EstimateRequest {
        parts: cart.parts,
        delivery: match delivery {
            Delivery::Courier => Delivery::Courier,
            Delivery::Locker => Delivery::Locker,
            Delivery::Pickup => Delivery::Pickup,
        },
    });
    packing.geometry_source = "parsed_model_slicer_mass_upper_bound";
    let print_lines = cart.lines;
    let setup_grosze = cart.setup;
    let print_grosze = cart.total;
    let mut reason = packing.reason.map(str::to_owned);
    let mut field_errors = HashMap::new();
    let mut offers = vec![];
    if cart.manual {
        reason = Some("Wymagania projektu wymagają ręcznej wyceny przed płatnością.".into());
    } else if packing.status == "estimated" {
        match delivery {
            Delivery::Pickup => offers.push(Offer {
                id: "pickup".into(),
                name: "Odbiór osobisty".into(),
                price_grosze: 0,
            }),
            Delivery::Courier | Delivery::Locker => {
                let locker = matches!(delivery, Delivery::Locker);
                if !request
                    .contact
                    .as_ref()
                    .is_some_and(|c| if locker { c.valid_locker() } else { c.valid() })
                {
                    reason = Some(
                        if locker {
                            "Uzupełnij dane kontaktowe i wybierz Paczkomat InPost."
                        } else {
                            "Uzupełnij imię, email, telefon i pełny polski adres dostawy."
                        }
                        .into(),
                    );
                } else if !app.carrier_configured() {
                    reason=Some("Brak połączenia z sandboxem Furgonetki. Cena kuriera nie jest jeszcze dostępna.".into());
                } else {
                    match carrier_offers(&app,&packing,request.contact.as_ref().unwrap(),locker).await {
                        Ok(found) if !found.is_empty()=>offers=found,
                        Ok(_)=>reason=Some("Brak dostępnej usługi dla tego zestawu paczek i adresu. Wybierz inną metodę lub poproś o indywidualną wycenę; nie oznacza to automatycznie błędnego adresu.".into()),
                        Err((_, body)) => {
                            reason=body.0["message"].as_str().map(str::to_owned);
                            field_errors=serde_json::from_value(body.0["field_errors"].clone()).unwrap_or_default();
                        },
                    }
                }
            }
        }
    }
    let expires_at = cart.expires_at.min(now() + TTL);
    let id = uuid::Uuid::new_v4().to_string();
    let packaging_grosze = packing.packaging_grosze;
    let mut quotes = app.quotes.lock().await;
    quotes.retain(|_, v| v.expires_at > now());
    if quotes.len() >= MAX_QUOTES {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Za dużo aktywnych wycen. Spróbuj później.",
        ));
    }
    quotes.insert(
        id.clone(),
        StoredQuote {
            order_data: Some(json!({"contact":request.contact,"lines":print_lines})),
            owner: cart.owner,
            quote_ids: cart.quote_ids,
            expires_at,
            subtotal: print_grosze.saturating_add(packaging_grosze),
            offers: offers.clone(),
            attempted_offer: None,
            session: None,
            intent: None,
            attempted_intent: false,
        },
    );
    Ok(Json(QuoteResponse {
        id,
        expires_at,
        packing,
        offers,
        print_grosze,
        print_lines,
        setup_grosze,
        packaging_grosze,
        production_demo: true,
        manual: cart.manual,
        stripe_configured: app.stripe_key.is_some(),
        furgonetka_configured: app.carrier_configured(),
        reason,
        field_errors,
    }))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = CheckoutSessionRequest)]
struct SessionRequest {
    quote_id: String,
    offer_id: String,
}
fn selected_total(quote: &StoredQuote, offer_id: &str, time: u64) -> Result<u32, Error> {
    if quote.expires_at <= time {
        return Err(error(
            StatusCode::CONFLICT,
            "Wycena wygasła. Przelicz zamówienie.",
        ));
    }
    let offer = quote
        .offers
        .iter()
        .find(|v| v.id == offer_id)
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "Wybierz potwierdzoną ofertę dostawy.",
            )
        })?;
    quote
        .subtotal
        .checked_add(offer.price_grosze)
        .filter(|v| *v > 0 && *v <= 99_999_999)
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "Wartość zamówienia wymaga ręcznej wyceny.",
            )
        })
}
fn stripe_form(quote_id: &str, total: u32) -> Vec<(String, String)> {
    vec![
        ("mode", "payment"),
        ("payment_method_types[0]", "card"),
        ("line_items[0][price_data][currency]", "pln"),
        (
            "line_items[0][price_data][product_data][name]",
            "Rapidmaker — zamówienie TESTOWE, bez realizacji",
        ),
        ("line_items[0][quantity]", "1"),
        (
            "success_url",
            "http://127.0.0.1:18430/zamowienie?stripe_session={CHECKOUT_SESSION_ID}",
        ),
        (
            "cancel_url",
            "http://127.0.0.1:18430/zamowienie?stripe_cancelled=1",
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .chain([
        (
            "line_items[0][price_data][unit_amount]".into(),
            total.to_string(),
        ),
        ("metadata[quote_id]".into(), quote_id.into()),
    ])
    .collect()
}
#[utoipa::path(operation_id = "checkout_session", post,
    path = "/api/checkout/session",
    tag = "Checkout",
    summary = "Utwórz testową sesję Stripe Checkout",
    request_body = SessionRequest,
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan")))]
async fn session(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(request): Json<SessionRequest>,
) -> Result<Json<Value>, Error> {
    let owner = super::cart::owner(app.storage.as_ref(), &headers).await?;
    let key = app.stripe_key.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Stripe sandbox nie jest skonfigurowany.",
        )
    })?;
    // Serialize creation, including provider retries, so one quote never opens two payable sessions.
    let mut quotes = app.quotes.lock().await;
    let quote = quotes.get_mut(&request.quote_id).ok_or_else(|| {
        error(
            StatusCode::NOT_FOUND,
            "Nie znaleziono wyceny. Przelicz zamówienie.",
        )
    })?;
    authorize(quote, &owner)?;
    let total = selected_total(quote, &request.offer_id, now())?;
    if quote.attempted_intent {
        return Err(error(
            StatusCode::CONFLICT,
            "Płatność jest już otwarta na stronie zamówienia.",
        ));
    }
    if quote
        .attempted_offer
        .as_ref()
        .is_some_and(|id| id != &request.offer_id)
    {
        return Err(error(
            StatusCode::CONFLICT,
            "Rozpoczęto płatność dla innej dostawy. Utwórz nową wycenę.",
        ));
    }
    // Persist the choice BEFORE the network call: a lost response must not allow
    // a second session with another delivery offer / idempotency key.
    quote.attempted_offer = Some(request.offer_id.clone());
    if let Some(existing) = &quote.session {
        if existing.offer_id != request.offer_id {
            return Err(error(
                StatusCode::CONFLICT,
                "Sesja ma już wybraną dostawę. Utwórz nową wycenę.",
            ));
        }
        return Ok(Json(json!({"url":existing.url,"session_id":existing.id})));
    }
    validate_payment(&app, quote).await?;
    hold_order_files(&app, &request.quote_id, quote).await?;
    let result = bounded_json(
        app.client
            .post(STRIPE)
            .bearer_auth(key)
            .header(
                "Idempotency-Key",
                format!("rapidmaker-{}", request.quote_id),
            )
            .form(&stripe_form(&request.quote_id, total))
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    let id = result["id"]
        .as_str()
        .filter(|id| id.starts_with("cs_test_"))
        .ok_or_else(unavailable)?;
    let url = result["url"]
        .as_str()
        .filter(|url| url.starts_with("https://checkout.stripe.com/"))
        .ok_or_else(unavailable)?;
    if !session_matches(&result, &request.quote_id, total) {
        return Err(unavailable());
    }
    persist_order(
        &app,
        &request.quote_id,
        quote,
        &headers,
        "session",
        id,
        total,
    )
    .await?;
    quote.session = Some(Session {
        id: id.into(),
        url: url.into(),
        offer_id: request.offer_id,
        total,
    });
    Ok(Json(json!({"url":url,"session_id":id})))
}
#[utoipa::path(operation_id = "checkout_payment_config", get,
    path = "/api/checkout/config",
    tag = "Checkout",
    summary = "Pobierz publiczny klucz Stripe",
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna")))]
async fn payment_config(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    let key = app
        .publishable_key
        .as_ref()
        .filter(|_| app.stripe_key.is_some())
        .ok_or_else(|| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Płatność na stronie wymaga konfiguracji Stripe sandbox.",
            )
        })?;
    Ok(Json(json!({"publishable_key":key})))
}
fn intent_matches(result: &Value, quote_id: &str, total: u32) -> bool {
    result["livemode"] == false
        && result["amount"].as_u64() == Some(u64::from(total))
        && result["currency"] == "pln"
        && result["metadata"]["quote_id"] == quote_id
}
#[utoipa::path(operation_id = "checkout_intent", post,
    path = "/api/checkout/intent",
    tag = "Checkout",
    summary = "Utwórz testowy PaymentIntent",
    request_body = SessionRequest,
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan")))]
async fn intent(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(request): Json<SessionRequest>,
) -> Result<Json<Value>, Error> {
    let owner = super::cart::owner(app.storage.as_ref(), &headers).await?;
    let key = app
        .stripe_key
        .as_ref()
        .filter(|_| app.publishable_key.is_some())
        .ok_or_else(|| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Płatność na stronie wymaga konfiguracji Stripe sandbox.",
            )
        })?;
    // Hold the bounded store lock across creation to serialize retries for one quote.
    let mut quotes = app.quotes.lock().await;
    let quote = quotes.get_mut(&request.quote_id).ok_or_else(|| {
        error(
            StatusCode::NOT_FOUND,
            "Nie znaleziono wyceny. Przelicz zamówienie.",
        )
    })?;
    authorize(quote, &owner)?;
    let total = selected_total(quote, &request.offer_id, now())?;
    if quote
        .attempted_offer
        .as_ref()
        .is_some_and(|id| id != &request.offer_id || !quote.attempted_intent)
    {
        return Err(error(
            StatusCode::CONFLICT,
            "Rozpoczęto już inną płatność. Utwórz nową wycenę.",
        ));
    }
    // Persist before the request: a lost response must not permit another payment mode/offer.
    quote.attempted_offer = Some(request.offer_id.clone());
    quote.attempted_intent = true;
    if let Some(existing) = &quote.intent {
        return Ok(Json(json!({"client_secret":existing.client_secret,
            "payment_intent_id":existing.id,"total_grosze":existing.total})));
    }
    validate_payment(&app, quote).await?;
    hold_order_files(&app, &request.quote_id, quote).await?;
    let form = [
        ("amount", total.to_string()),
        ("currency", "pln".into()),
        ("metadata[quote_id]", request.quote_id.clone()),
        ("allowed_payment_method_types[0]", "card".into()),
        (
            "description",
            "Rapidmaker — zamówienie TESTOWE, bez realizacji".into(),
        ),
    ];
    let result = bounded_json(
        app.client
            .post(STRIPE_INTENTS)
            .bearer_auth(key)
            .header(
                "Idempotency-Key",
                format!("rapidmaker-intent-{}", request.quote_id),
            )
            .form(&form)
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    let id = result["id"]
        .as_str()
        .filter(|id| id.starts_with("pi_"))
        .ok_or_else(unavailable)?;
    let secret = result["client_secret"]
        .as_str()
        .filter(|secret| secret.starts_with(&format!("{id}_secret_")))
        .ok_or_else(unavailable)?;
    if !intent_matches(&result, &request.quote_id, total) {
        return Err(unavailable());
    }
    persist_order(
        &app,
        &request.quote_id,
        quote,
        &headers,
        "intent",
        id,
        total,
    )
    .await?;
    quote.intent = Some(Intent {
        id: id.into(),
        client_secret: secret.into(),
        total,
    });
    Ok(Json(
        json!({"client_secret":secret,"payment_intent_id":id,"total_grosze":total}),
    ))
}
#[utoipa::path(operation_id = "checkout_intent_status", get,
    path = "/api/checkout/intent/{id}",
    tag = "Checkout",
    summary = "Sprawdź testową płatność",
    params(("id" = String, Path, description = "Identyfikator płatności Stripe")),
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan")))]
async fn intent_status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    let owner = super::cart::owner(app.storage.as_ref(), &headers).await?;
    let key = app.stripe_key.as_ref().ok_or_else(unavailable)?;
    let quotes = app.quotes.lock().await;
    let (quote_id, intent) = quotes
        .iter()
        .filter(|(_, quote)| quote.owner == owner)
        .find_map(|(quote_id, quote)| {
            quote
                .intent
                .as_ref()
                .filter(|intent| intent.id == id)
                .map(|intent| (quote_id.clone(), intent.clone()))
        })
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Nie znaleziono lokalnej płatności testowej.",
            )
        })?;
    drop(quotes);
    let result = bounded_json(
        app.client
            .get(format!("{STRIPE_INTENTS}/{}", intent.id))
            .bearer_auth(key)
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    if !intent_matches(&result, &quote_id, intent.total) || result["id"] != intent.id {
        return Err(unavailable());
    }
    let paid = result["status"] == "succeeded";
    if paid && let Some(db) = &app.storage {
        db.order_paid(&quote_id, &intent.id, i64::from(intent.total), now() as i64)
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(Json(
        json!({"status":if paid {"paid"} else {"unpaid"},"livemode":false,
        "total_grosze":intent.total,"currency":"pln","quote_id":quote_id,
        "checkout_status":if paid {"complete"} else if result["status"] == "canceled" {"expired"} else if result["status"] == "processing" {"processing"} else {"open"},"url":""}),
    ))
}
// Resume requires the model owner cookie and the provider's current payment state.
#[utoipa::path(operation_id = "checkout_resume_intent", get,
    path = "/api/checkout/intent/{id}/resume",
    tag = "Checkout",
    summary = "Wznów własną otwartą płatność testową",
    params(("id" = String, Path, description = "Identyfikator płatności Stripe")),
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan")))]
async fn resume_intent(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    let owner = super::cart::owner(app.storage.as_ref(), &headers).await?;
    let status = intent_status(State(app.clone()), headers, Path(id.clone()))
        .await?
        .0;
    if status["status"] == "paid" || status["checkout_status"] != "open" {
        return Err(error(
            StatusCode::CONFLICT,
            "Płatność została zakończona lub jest przetwarzana. Odśwież jej status.",
        ));
    }
    let quotes = app.quotes.lock().await;
    let existing = quotes
        .values()
        .filter(|quote| quote.owner == owner)
        .find_map(|quote| quote.intent.as_ref().filter(|intent| intent.id == id))
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Nie znaleziono lokalnej płatności testowej.",
            )
        })?;
    Ok(Json(
        json!({"client_secret":existing.client_secret,"payment_intent_id":existing.id,"total_grosze":existing.total}),
    ))
}
fn session_matches(result: &Value, quote_id: &str, total: u32) -> bool {
    result["livemode"] == false
        && result["amount_total"].as_u64() == Some(u64::from(total))
        && result["currency"] == "pln"
        && result["metadata"]["quote_id"] == quote_id
}
#[utoipa::path(operation_id = "checkout_session_status", get,
    path = "/api/checkout/session/{id}",
    tag = "Checkout",
    summary = "Sprawdź własną sesję Stripe Checkout",
    params(("id" = String, Path, description = "Identyfikator płatności Stripe")),
    security(("session" = []), ("guest" = [])),
    responses((status = 200, body = Value), (status = 400, body = Value, description = "Nieprawidłowe dane"), (status = 502, body = Value, description = "Błąd integracji testowej"), (status = 503, body = Value, description = "Integracja lub baza niedostępna"), (status = 401, body = Value, description = "Wymagana własna sesja"), (status = 404, body = Value, description = "Nie znaleziono własnego zasobu"), (status = 409, body = Value, description = "Wycena wygasła lub płatność zmieniła stan")))]
async fn session_status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    let owner = super::cart::owner(app.storage.as_ref(), &headers).await?;
    let key = app.stripe_key.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Stripe sandbox nie jest skonfigurowany.",
        )
    })?;
    let quotes = app.quotes.lock().await;
    let (quote_id, session) = quotes
        .iter()
        .filter(|(_, quote)| quote.owner == owner)
        .find_map(|(quote_id, quote)| {
            quote
                .session
                .as_ref()
                .filter(|s| s.id == id)
                .map(|s| (quote_id.clone(), s.clone()))
        })
        .ok_or_else(|| {
            error(
                StatusCode::NOT_FOUND,
                "Nie znaleziono lokalnej sesji testowej.",
            )
        })?;
    drop(quotes);
    let result = bounded_json(
        app.client
            .get(format!("{STRIPE}/{}", session.id))
            .bearer_auth(key)
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    if !session_matches(&result, &quote_id, session.total) || result["id"] != session.id {
        return Err(unavailable());
    }
    if result["payment_status"] == "paid"
        && let Some(db) = &app.storage
    {
        db.order_paid(
            &quote_id,
            &session.id,
            i64::from(session.total),
            now() as i64,
        )
        .await
        .map_err(|_| unavailable())?;
    }
    Ok(Json(
        json!({"status":if result["payment_status"]=="paid" {"paid"} else {"unpaid"},
        "livemode":false,"total_grosze":session.total,"currency":"pln","quote_id":quote_id,
        "checkout_status":result["status"],"url":session.url}),
    ))
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
            "Nieprawidłowe żądanie. Sprawdź adres API, format JSON i dopuszczalne pola.",
        )
        .into_response();
    }
    response
}
fn authorize(quote: &StoredQuote, owner: &str) -> Result<(), Error> {
    if quote.owner != owner {
        return Err(error(StatusCode::NOT_FOUND, "Nie znaleziono wyceny."));
    }
    Ok(())
}
async fn validate_payment(app: &App, quote: &StoredQuote) -> Result<(), Error> {
    let storage = app.storage.as_ref().ok_or_else(unavailable)?;
    if !quote
        .order_data
        .as_ref()
        .and_then(|d| d["contact"]["email"].as_str())
        .is_some_and(rapidmaker_notifications::valid_address)
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Podaj poprawny email odbiorcy przed płatnością.",
        ));
    }
    super::cart::validate(storage, &quote.owner, &quote.quote_ids).await?;
    Ok(())
}

async fn hold_order_files(app: &App, id: &str, quote: &StoredQuote) -> Result<(), Error> {
    let db = app.storage.as_ref().ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Magazyn modeli jest niedostępny.",
        )
    })?;
    // A provider timeout can still mean a payable intent exists; keep this durable hold.
    let held = db
        .hold_order_files(id, &quote.owner, &quote.quote_ids, now() as i64)
        .await
        .map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Nie można zabezpieczyć modeli zamówienia.",
            )
        })?;
    if !held {
        return Err(error(
            StatusCode::CONFLICT,
            "Modele zamówienia wygasły lub zmieniły właściciela. Przelicz zamówienie.",
        ));
    }
    Ok(())
}

async fn persist_order(
    app: &App,
    id: &str,
    quote: &StoredQuote,
    headers: &HeaderMap,
    kind: &str,
    provider: &str,
    total: u32,
) -> Result<(), Error> {
    let db = app.storage.as_ref().ok_or_else(unavailable)?;
    let data = quote.order_data.as_ref().ok_or_else(unavailable)?;
    let (owner, user) = super::cart::identity(Some(db), headers).await?;
    if owner != quote.owner {
        return Err(error(
            StatusCode::CONFLICT,
            "Sesja właściciela zmieniła się podczas płatności. Wczytaj zamówienie ponownie.",
        ));
    }
    db.create_order(
        &rapidmaker_storage::OrderInput {
            id,
            owner_hash: &quote.owner,
            user_id: user.as_deref(),
            contact: &data["contact"],
            lines: &data["lines"],
            amount_grosze: i64::from(total),
            provider_kind: kind,
            provider_id: provider,
            guest_updates: data["contact"]["emailOrderUpdates"]
                .as_bool()
                .unwrap_or(true),
        },
        now() as i64,
    )
    .await
    .map_err(|_| unavailable())
}

pub(super) async fn refresh_order(db: &Storage, id: &str, order: &Value) -> Result<(), Error> {
    if order["status"] != "awaiting_payment" {
        return Ok(());
    }
    if !db
        .poll_order_payment(id, now() as i64)
        .await
        .map_err(|_| unavailable())?
    {
        return Ok(());
    }
    let app = App::from_env(None);
    let key = app.stripe_key.as_ref().ok_or_else(unavailable)?;
    let provider = order["provider_id"]
        .as_str()
        .filter(|p| p.len() <= 128 && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .ok_or_else(unavailable)?;
    let total = order["amount_grosze"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(unavailable)?;
    let intent = order["provider_kind"] == "intent";
    if !(if intent {
        provider.starts_with("pi_")
    } else {
        provider.starts_with("cs_test_")
    }) {
        return Err(unavailable());
    }
    let endpoint = if intent { STRIPE_INTENTS } else { STRIPE };
    let result = bounded_json(
        app.client
            .get(format!("{endpoint}/{provider}"))
            .bearer_auth(key)
            .send()
            .await
            .map_err(|_| unavailable())?,
    )
    .await?;
    if result["id"] != provider
        || !(if intent {
            intent_matches(&result, id, total)
        } else {
            session_matches(&result, id, total)
        })
    {
        return Err(unavailable());
    }
    if (intent && result["status"] == "succeeded")
        || (!intent && result["payment_status"] == "paid")
    {
        db.order_paid(id, provider, i64::from(total), now() as i64)
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(())
}

#[utoipa::path(operation_id = "checkout_status", get, path = "/api/checkout/status", tag = "Checkout",
    summary = "Sprawdź konfigurację integracji testowych",
    responses((status = 200, body = Value, description = "Dostępność Stripe i Furgonetki")))]
async fn status(State(app): State<Arc<App>>) -> Json<Value> {
    Json(
        json!({"stripe_configured":app.stripe_key.is_some(),"furgonetka_configured":app.carrier_configured()}),
    )
}

pub fn router(storage: Option<Storage>) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(points))
        .routes(routes!(status))
        .routes(routes!(payment_config))
        .routes(routes!(intent))
        .routes(routes!(intent_status))
        .routes(routes!(resume_intent))
        .routes(routes!(quote))
        .routes(routes!(session))
        .routes(routes!(session_status))
        .with_state(Arc::new(App::from_env(storage)))
}
pub fn protect(router: axum::Router, auth: Arc<super::auth::Auth>) -> axum::Router {
    router
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(auth, local_only))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carrier_address_errors_are_actionable_without_echoing_provider_values() {
        let (status, body) = carrier_rejection(
            &json!({"errors":[{"path":"/receiver/street","code":"noNumber","message":"provider supplied value"}]}),
        );
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body.0["field_errors"]["street"],
            "Dodaj numer budynku do ulicy."
        );
        assert!(!body.0.to_string().contains("provider supplied value"));
        let (status, body) =
            carrier_rejection(&json!({"errors":[{"path":"/pickup/street","code":"noNumber"}]}));
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.0["field_errors"].is_null());
    }
    #[test]
    fn unavailable_or_invalid_carrier_price_never_becomes_free() {
        let rates = json!({"services_prices":[
            {"service_id":1,"available":false,"pricing":{"price_gross":16.99}},
            {"service_id":2,"available":true,"pricing":{"price_gross":0}},
            {"service_id":3,"available":true,"pricing":{"price_gross":"NaN"}},
            {"service_id":4,"available":true,"pricing":{"price_gross":"16.99"}},
            {"service_id":999,"available":true,"pricing":{"price_gross":1}}]});
        let services = (1..=4)
            .map(|n| (n.to_string(), "dpd".into()))
            .collect::<Vec<_>>();
        let offers = parse_offers(&rates, &services);
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].price_grosze, 1699);
        assert_eq!(offers[0].id, "4");
    }
    #[test]
    fn server_total_expires_and_rejects_tampering() {
        let quote = StoredQuote {
            order_data: None,
            owner: super::super::files::owner(&test_headers()).unwrap(),
            quote_ids: Vec::new(),
            expires_at: 100,
            subtotal: 4500,
            offers: vec![Offer {
                id: "pickup".into(),
                name: "".into(),
                price_grosze: 0,
            }],
            attempted_offer: None,
            session: None,
            intent: None,
            attempted_intent: false,
        };
        assert_eq!(
            authorize(&quote, "other").unwrap_err().0,
            StatusCode::NOT_FOUND
        );
        assert!(authorize(&quote, &quote.owner).is_ok());
        assert_eq!(selected_total(&quote, "pickup", 99).unwrap(), 4500);
        assert!(selected_total(&quote, "pickup", 100).is_err());
        assert!(selected_total(&quote, "invented", 99).is_err());
        assert!(
            serde_json::from_value::<SessionRequest>(
                json!({"quote_id":"x","offer_id":"pickup","total":1})
            )
            .is_err()
        );
        assert!(
            serde_json::from_value::<QuoteRequest>(
                json!({"parts":[],"delivery":"pickup","print_grosze":1})
            )
            .is_err()
        );
    }
    #[test]
    fn stripe_requires_correct_server_snapshot_and_test_mode() {
        let valid = json!({"livemode":false,"amount_total":4500,"currency":"pln","metadata":{"quote_id":"q"}});
        assert!(session_matches(&valid, "q", 4500));
        assert!(!session_matches(&valid, "q", 1));
        assert!(!session_matches(&valid, "other", 4500));
        let mut live = valid.clone();
        live["livemode"] = json!(true);
        assert!(!session_matches(&live, "q", 4500));
        assert!(test_key("sk_test_sample"));
        assert!(!test_key("sk_live_sample"));
    }
    #[test]
    fn hostile_hostnames_and_partial_addresses_rejected() {
        assert!(local_authority("127.0.0.1:18430"));
        for host in [
            "127.0.0.1.evil.test:18430",
            "evil.test",
            "localhost",
            "localhost:80",
        ] {
            assert!(!local_authority(host));
        }
        assert!(!Contact::default().valid());
    }
    fn test_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("rapidmaker_guest={}", "a".repeat(64))
                .parse()
                .unwrap(),
        );
        headers
    }
    fn test_app() -> Arc<App> {
        Arc::new(App {
            storage: None,
            client: reqwest::Client::new(),
            stripe_key: Some("sk_test_local_test".into()),
            publishable_key: Some("pk_test_local_test".into()),
            carrier_token: None,
            sender: None,
            quotes: Mutex::new(HashMap::new()),
        })
    }
    #[tokio::test]
    async fn retries_reuse_the_existing_session_and_reject_changed_offers() {
        let app = test_app();
        app.quotes.lock().await.insert(
            "q".into(),
            StoredQuote {
                order_data: None,
                owner: super::super::files::owner(&test_headers()).unwrap(),
                quote_ids: Vec::new(),
                expires_at: now() + TTL,
                subtotal: 3900,
                offers: vec![
                    Offer {
                        id: "pickup".into(),
                        name: "".into(),
                        price_grosze: 0,
                    },
                    Offer {
                        id: "other".into(),
                        name: "".into(),
                        price_grosze: 100,
                    },
                ],
                attempted_offer: Some("pickup".into()),
                intent: None,
                attempted_intent: false,
                session: Some(Session {
                    id: "cs_test_existing".into(),
                    url: "https://checkout.stripe.com/existing".into(),
                    offer_id: "pickup".into(),
                    total: 3900,
                }),
            },
        );
        for _ in 0..2 {
            let result = session(
                State(app.clone()),
                test_headers(),
                Json(SessionRequest {
                    quote_id: "q".into(),
                    offer_id: "pickup".into(),
                }),
            )
            .await
            .unwrap();
            assert_eq!(result.0["session_id"], "cs_test_existing");
        }
        assert_eq!(
            session(
                State(app.clone()),
                test_headers(),
                Json(SessionRequest {
                    quote_id: "q".into(),
                    offer_id: "other".into()
                })
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        // Simulate Stripe having created a session but its response being lost.
        // The chosen offer is still bound, even though no session was stored.
        app.quotes.lock().await.get_mut("q").unwrap().session = None;
        assert_eq!(
            session(
                State(app.clone()),
                test_headers(),
                Json(SessionRequest {
                    quote_id: "q".into(),
                    offer_id: "other".into()
                })
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        app.quotes.lock().await.get_mut("q").unwrap().expires_at = now() - 1;
        assert_eq!(
            session(
                State(app),
                test_headers(),
                Json(SessionRequest {
                    quote_id: "q".into(),
                    offer_id: "pickup".into()
                })
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
    }
    #[tokio::test]
    async fn intent_retries_bind_offer_and_exclude_hosted_checkout() {
        let app = test_app();
        app.quotes.lock().await.insert(
            "q".into(),
            StoredQuote {
                order_data: None,
                owner: super::super::files::owner(&test_headers()).unwrap(),
                quote_ids: Vec::new(),
                expires_at: now() + TTL,
                subtotal: 3900,
                offers: vec![
                    Offer {
                        id: "pickup".into(),
                        name: "".into(),
                        price_grosze: 0,
                    },
                    Offer {
                        id: "other".into(),
                        name: "".into(),
                        price_grosze: 100,
                    },
                ],
                attempted_offer: Some("pickup".into()),
                attempted_intent: true,
                session: None,
                intent: Some(Intent {
                    id: "pi_existing".into(),
                    client_secret: "pi_existing_secret_test".into(),
                    total: 3900,
                }),
            },
        );
        let request = || {
            Json(SessionRequest {
                quote_id: "q".into(),
                offer_id: "pickup".into(),
            })
        };
        for _ in 0..2 {
            let result = intent(State(app.clone()), test_headers(), request())
                .await
                .unwrap()
                .0;
            assert_eq!(result["payment_intent_id"], "pi_existing");
            assert_eq!(result["total_grosze"], 3900);
        }
        assert_eq!(
            session(State(app.clone()), test_headers(), request())
                .await
                .unwrap_err()
                .0,
            StatusCode::CONFLICT
        );
        assert_eq!(
            intent(
                State(app.clone()),
                test_headers(),
                Json(SessionRequest {
                    quote_id: "q".into(),
                    offer_id: "other".into()
                })
            )
            .await
            .unwrap_err()
            .0,
            StatusCode::CONFLICT
        );
        app.quotes.lock().await.get_mut("q").unwrap().expires_at = now() - 1;
        assert_eq!(
            intent(State(app), test_headers(), request())
                .await
                .unwrap_err()
                .0,
            StatusCode::CONFLICT
        );
    }
    #[test]
    fn intent_verification_rejects_wrong_amount_quote_and_live_mode() {
        let valid =
            json!({"livemode":false,"amount":3900,"currency":"pln","metadata":{"quote_id":"q"}});
        assert!(intent_matches(&valid, "q", 3900));
        assert!(!intent_matches(&valid, "q", 3800));
        assert!(!intent_matches(&valid, "other", 3900));
        let mut live = valid;
        live["livemode"] = json!(true);
        assert!(!intent_matches(&live, "q", 3900));
    }
    #[tokio::test]
    async fn quote_manual_is_not_payable_and_prices_are_server_owned() {
        let app = test_app();
        let request = serde_json::from_value(
            json!({"parts":[{"id":"part","quantity":3}],"delivery":"pickup","manual":true}),
        )
        .unwrap();
        let result = quote(State(app.clone()), test_headers(), Json(request))
            .await
            .unwrap()
            .0;
        assert!(result.offers.is_empty());
        assert!(result.reason.is_some());
        assert_eq!(result.print_grosze, 0);
        assert_eq!(result.setup_grosze, 0);
        assert_eq!(
            result.print_grosze + result.packaging_grosze,
            app.quotes.lock().await.get(&result.id).unwrap().subtotal
        );
        let request = serde_json::from_value(
            json!({"parts":[{"id":"part","quantity":1000}],"delivery":"pickup"}),
        )
        .unwrap();
        let result = quote(State(app), test_headers(), Json(request))
            .await
            .unwrap()
            .0;
        assert!(result.offers.is_empty());
        assert!(result.manual);
        assert_eq!(result.print_grosze, 0);
    }
}
