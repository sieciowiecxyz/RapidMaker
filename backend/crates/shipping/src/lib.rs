//! Domestic address validation and bounded Furgonetka sandbox adapter. Shipment purchases are restricted to sandbox.
mod address;
mod operations;
mod shipments;
pub use address::{DeliveryChoice, Invoice, Method, Recipient, Validation};
pub use operations::PickupDate;
use rapidmaker_packing::Estimate;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub use shipments::{
    CommandStatus, MeasuredParcel, PreparedPackage, ProviderPackage, ShipQuote, ShipmentError,
    Tracking, matches_prepared,
};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, OnceCell, Semaphore};
use utoipa::ToSchema;

const API: &str = "https://api.sandbox.furgonetka.pl";
const SERVICES_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const QUOTE_TTL: Duration = Duration::from_secs(120);
const QUOTE_ERROR_TTL: Duration = Duration::from_secs(2);
const MAX_QUOTE_KEYS: usize = 64;
const MAX_QUOTE_KEY_BYTES: usize = 131072;

fn quote_key(
    plans: &[Estimate],
    recipient: &Recipient,
    choice: &DeliveryChoice,
) -> Result<Vec<u8>, Error> {
    // Keep plan order: the cached selected index refers to this exact ordering.
    let plans = plans
        .iter()
        .map(|plan| {
            (
                plan.packaging_grosze,
                plan.parcels
                    .iter()
                    .map(|parcel| (parcel.outer_mm, parcel.weight_g))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&(plans, recipient, choice)).map_err(|_| unavailable())
}

type CachedQuote = (Instant, Result<(usize, Quote), Error>);
#[derive(Default)]
struct QuoteCache(Mutex<HashMap<Vec<u8>, Arc<OnceCell<CachedQuote>>>>);
impl QuoteCache {
    async fn get(
        &self,
        key: Vec<u8>,
        load: impl Future<Output = Result<(usize, Quote), Error>>,
    ) -> Result<(usize, Quote), Error> {
        if key.len() > MAX_QUOTE_KEY_BYTES {
            return Err(unavailable());
        }
        let entry = {
            let mut entries = self.0.lock().await;
            entries.retain(|_, cell| match cell.get() {
                Some((saved, result)) => {
                    saved.elapsed()
                        < if result.as_ref().is_ok_and(|(_, q)| !q.manual_required) {
                            QUOTE_TTL
                        } else {
                            QUOTE_ERROR_TTL
                        }
                }
                // A cancelled initializer leaves an empty cell; retain active waiters only.
                None => Arc::strong_count(cell) > 1,
            });
            if !entries.contains_key(&key) && entries.len() >= MAX_QUOTE_KEYS {
                return Err(unavailable());
            }
            Arc::clone(entries.entry(key).or_default())
        };
        entry
            .get_or_init(|| async {
                let mut result = load.await;
                if let Ok((_, quote)) = &mut result
                    && !quote.manual_required
                {
                    let expires_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .ok()
                        .and_then(|d| d.as_secs().checked_add(QUOTE_TTL.as_secs()))
                        .and_then(|s| i64::try_from(s).ok());
                    if let Some(expires_at) = expires_at {
                        quote.expires_at = Some(expires_at);
                    } else {
                        result = Err(unavailable());
                    }
                }
                (Instant::now(), result)
            })
            .await
            .1
            .clone()
    }
}

#[derive(Default)]
struct ServicesCache(Mutex<Option<(Instant, Arc<Value>)>>);
impl ServicesCache {
    async fn get(
        &self,
        load: impl Future<Output = Result<Value, Error>>,
    ) -> Result<Arc<Value>, Error> {
        // Holding the async lock during refresh coalesces misses; cancellation releases it.
        let mut entry = self.0.lock().await;
        if let Some((saved, data)) = &*entry
            && saved.elapsed() < SERVICES_TTL
        {
            return Ok(Arc::clone(data));
        }
        let data = load.await?;
        if !data["services"].is_array() {
            return Err(unavailable());
        }
        let data = Arc::new(data);
        *entry = Some((Instant::now(), Arc::clone(&data)));
        Ok(data)
    }
}
#[derive(Clone, Debug)]
pub enum Error {
    Validation(Validation),
    Unavailable(String),
    NoService(String),
}
fn unavailable() -> Error {
    Error::Unavailable("Dostawca dostawy jest chwilowo niedostępny. Spróbuj ponownie.".into())
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PointAddress {
    pub street: String,
    pub postcode: String,
    pub city: String,
    pub country_code: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub code: String,
    pub name: String,
    pub address: PointAddress,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub id: String,
    pub name: String,
    pub price_grosze: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub offers: Vec<Offer>,
    pub point: Option<Point>,
    pub manual_required: bool,
    pub reason: Option<String>,
    pub expires_at: Option<i64>,
}
impl Quote {
    pub fn pickup() -> Self {
        Self {
            offers: vec![Offer {
                id: "pickup".into(),
                name: "Odbiór osobisty".into(),
                price_grosze: 0,
            }],
            point: None,
            manual_required: false,
            reason: None,
            expires_at: None,
        }
    }
    fn manual(reason: &str) -> Self {
        Self {
            offers: vec![],
            point: None,
            manual_required: true,
            reason: Some(reason.into()),
            expires_at: None,
        }
    }
}
#[derive(Clone)]
pub struct Sandbox {
    client: Client,
    token: String,
    sender: Value,
    services: Arc<ServicesCache>,
    quotes: Arc<QuoteCache>,
    quote_slots: Arc<Semaphore>,
}
impl Sandbox {
    pub fn new(token: String, sender: Value) -> Result<Self, Error> {
        if token.is_empty()
            || token.len() > 8192
            || token.chars().any(char::is_control)
            || !sender.is_object()
            || serde_json::to_vec(&sender).map_or(true, |v| v.len() > 16384)
        {
            return Err(unavailable());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| unavailable())?;
        Ok(Self {
            client,
            token,
            sender,
            services: Arc::default(),
            quotes: Arc::default(),
            quote_slots: Arc::new(Semaphore::new(4)),
        })
    }
    fn build_request(
        &self,
        method: reqwest::Method,
        path: &str,
        version: u8,
        body: Option<&Value>,
    ) -> Result<reqwest::Request, Error> {
        let mime = format!("application/vnd.furgonetka.v{version}+json");
        let mut request = self
            .client
            .request(method, format!("{API}{path}"))
            .bearer_auth(&self.token)
            .header("Accept", &mime);
        // Furgonetka rejects bodyless GETs carrying Content-Type with an unstructured 400.
        if let Some(body) = body {
            request = request.header("Content-Type", mime).json(body);
        }
        request.build().map_err(|_| unavailable())
    }
    #[tracing::instrument(skip_all, level = "debug")]
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        version: u8,
        body: Option<&Value>,
    ) -> Result<Response, Error> {
        let operation = request_operation(path);
        let started = std::time::Instant::now();
        let result = async {
            self.client
                .execute(self.build_request(method, path, version, body)?)
                .await
                .map_err(|_| unavailable())
        }
        .await;
        match &result {
            Ok(response) if response.status().is_success() => tracing::debug!(
                event = "provider_response",
                provider = "furgonetka",
                operation = operation,
                status = response.status().as_u16(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                "Provider responded"
            ),
            Ok(response) => tracing::warn!(
                event = "provider_rejected",
                provider = "furgonetka",
                operation = operation,
                status = response.status().as_u16(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                reason = "http_status",
                "Provider rejected request"
            ),
            Err(_) => tracing::warn!(
                event = "provider_request_failed",
                provider = "furgonetka",
                operation = operation,
                elapsed_ms = started.elapsed().as_millis() as u64,
                reason = "transport_or_configuration",
                "Provider request failed"
            ),
        }
        result
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn points(&self, query: &str) -> Result<Vec<Point>, Error> {
        match self.find_points(query, false).await {
            Err(Error::NoService(_)) => Err(unavailable()),
            result => result,
        }
    }
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn verify_point(&self, id: &str) -> Result<Point, Error> {
        if !address::valid_point_id(id) {
            return Err(Error::Validation(Validation::field(
                "delivery.point_id",
                "Wybierz dostępny punkt odbioru.",
            )));
        }
        self.find_points(id, true)
            .await?
            .into_iter()
            .find(|p| p.code == id)
            .ok_or_else(|| {
                Error::Validation(Validation::field(
                    "delivery.point_id",
                    "Punkt jest niedostępny. Wybierz inny.",
                ))
            })
    }
    async fn find_points(&self, query: &str, exact: bool) -> Result<Vec<Point>, Error> {
        if !(2..=100).contains(&query.len()) || query.chars().any(char::is_control) {
            return Err(Error::Validation(Validation::field(
                "postal_code",
                "Podaj kod pocztowy lub miejscowość do wyszukania punktów.",
            )));
        }
        let mut filters = json!({"services":["inpost"],"point_types":["delivery_points"],"type":"parcel_machine","country_codes":["PL"],"limit":12});
        if exact {
            filters["point_id"] = json!(query);
        }
        let postcode = query.len() == 6
            && query.as_bytes()[2] == b'-'
            && query
                .bytes()
                .enumerate()
                .all(|(i, c)| i == 2 || c.is_ascii_digit());
        let location = if !exact && postcode {
            json!({"address":{"postcode":query,"country_code":"PL"},"points_max_distance":10})
        } else {
            json!({"search_phrase":query})
        };
        let data = provider_json(
            self.request(
                reqwest::Method::POST,
                "/points/map",
                1,
                Some(&json!({"location":location,"filters":filters})),
            )
            .await?,
        )
        .await?;
        let points = data["points"].as_array().ok_or_else(unavailable)?;
        let mut result = vec![];
        for point in points.iter().take(100) {
            if point["active"] != true
                || point["is_delivery_point"] != true
                || point["service"] != "inpost"
            {
                continue;
            }
            let Some(code) = point["code"]
                .as_str()
                .filter(|id| address::valid_point_id(id))
            else {
                return Err(unavailable());
            };
            if exact && code != query {
                continue;
            }
            let field = |key: &str| {
                point["address"][key]
                    .as_str()
                    .filter(|v| !v.is_empty() && v.len() <= 254 && !v.chars().any(char::is_control))
                    .map(str::to_owned)
                    .ok_or_else(unavailable)
            };
            let country = point["address"]["country_code"].as_str().unwrap_or("PL");
            if country != "PL" {
                continue;
            }
            result.push(Point {
                code: code.into(),
                name: point["name"]
                    .as_str()
                    .filter(|v| v.len() <= 254)
                    .unwrap_or(code)
                    .into(),
                address: PointAddress {
                    street: field("street")?,
                    postcode: field("postcode")?,
                    city: field("city")?,
                    country_code: "PL".into(),
                },
            });
            if result.len() == 12 {
                break;
            }
        }
        Ok(result)
    }
    /// Compare all bounded plans using provider prices, then validate cheapest first.
    /// All returned offers use the selected plan, preserving the checkout snapshot contract.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn quote_best(
        &self,
        plans: &[Estimate],
        recipient: &Recipient,
        choice: &DeliveryChoice,
    ) -> Result<(usize, Quote), Error> {
        let mut recipient = recipient.clone();
        recipient.normalize(choice).map_err(Error::Validation)?;
        if plans.is_empty() || plans.len() > 5 {
            return Err(unavailable());
        }
        if choice.method == Method::Pickup {
            return Ok((0, Quote::pickup()));
        }
        if plans
            .iter()
            .any(|p| p.status != "estimated" || p.parcels.is_empty() || p.parcels.len() > 30)
        {
            return Ok((0, Quote::manual("Paczki wymagają indywidualnej wyceny.")));
        }
        let key = quote_key(plans, &recipient, choice)?;
        self.quotes
            .get(key, async {
                let _permit = self.quote_slots.try_acquire().map_err(|_| unavailable())?;
                tokio::time::timeout(
                    Duration::from_secs(30),
                    self.quote_inner(plans, &recipient, choice),
                )
                .await
                .map_err(|_| unavailable())?
            })
            .await
    }
    async fn quote_inner(
        &self,
        plans: &[Estimate],
        recipient: &Recipient,
        choice: &DeliveryChoice,
    ) -> Result<(usize, Quote), Error> {
        let data = self
            .services
            .get(async {
                provider_json(
                    self.request(reqwest::Method::GET, "/account/services", 1, None)
                        .await?,
                )
                .await
            })
            .await?;
        let services = data["services"]
            .as_array()
            .ok_or_else(unavailable)?
            .iter()
            .filter_map(|service| {
                let name = service["service"].as_str()?;
                if if choice.method == Method::Locker {
                    name != "inpost"
                } else {
                    !["dpd", "dhl", "ups", "gls", "fedex"].contains(&name)
                } {
                    return None;
                }
                Some((id(&service["id"])?, name.to_owned()))
            })
            .take(10)
            .collect::<Vec<_>>();
        if services.is_empty() {
            return Err(Error::Unavailable(
                "Konto Furgonetki nie ma aktywnych usług tej metody dostawy.".into(),
            ));
        }
        let street = if recipient.apartment.is_empty() {
            recipient.street.clone()
        } else {
            format!("{} / {}", recipient.street, recipient.apartment)
        };
        let mut receiver = json!({"name":recipient.name,"email":recipient.email,"phone":recipient.phone,"street":street,"city":recipient.city,"country_code":"PL","postcode":recipient.postal_code});
        let point = if choice.method == Method::Locker {
            let point = self
                .verify_point(choice.point_id.as_deref().ok_or_else(|| {
                    Error::Validation(Validation::field(
                        "delivery.point_id",
                        "Wybierz punkt odbioru.",
                    ))
                })?)
                .await?;
            receiver["point"] = json!(point.code);
            receiver["street"] = json!(point.address.street);
            receiver["city"] = json!(point.address.city);
            receiver["postcode"] = json!(point.address.postcode);
            Some(point)
        } else {
            None
        };
        let mut packages = Vec::new();
        let mut rates = Vec::new();
        let mut rejection = None;
        for (plan, packing) in plans.iter().enumerate() {
            let parcels = packing
                .parcels
                .iter()
                .map(|p| {
                    json!({
                        "width":p.outer_mm[0].div_ceil(10), "depth":p.outer_mm[1].div_ceil(10),
                        "height":p.outer_mm[2].div_ceil(10), "weight":f64::from(p.weight_g)/1000.,
                        "quantity":1, "type":"package"
                    })
                })
                .collect::<Vec<_>>();
            let package = json!({"pickup":self.sender,"receiver":receiver,"service_id":provider_id(&services[0].0),"parcels":parcels});
            let response = provider_json(self.request(reqwest::Method::POST,
                "/packages/calculate-price", 2, Some(&json!({"package":package,
                "services":{"service_id":services.iter().map(|(id,_)|provider_id(id)).collect::<Vec<_>>()}}))).await?).await;
            packages.push(package);
            let response = match response {
                Ok(value) => value,
                Err(Error::Validation(v)) => {
                    rejection = Some(Error::Validation(v));
                    continue;
                }
                Err(Error::NoService(_)) => continue,
                Err(e) => return Err(e),
            };
            for row in response["services_prices"]
                .as_array()
                .ok_or_else(unavailable)?
            {
                if row["available"] != true {
                    if let Some(error) = classify_rejection(row) {
                        match error {
                            Error::Unavailable(_) => return Err(error),
                            _ => rejection = Some(error),
                        }
                    }
                    continue;
                }
                let service_id = id(&row["service_id"]).ok_or_else(unavailable)?;
                let Some((_, name)) = services.iter().find(|(id, _)| id == &service_id) else {
                    return Err(unavailable());
                };
                if rates
                    .iter()
                    .any(|(p, o): &(usize, Offer)| *p == plan && o.id == service_id)
                {
                    return Err(unavailable());
                }
                rates.push((
                    plan,
                    Offer {
                        id: service_id,
                        name: name.to_uppercase(),
                        price_grosze: money(&row["pricing"]["price_gross"])
                            .ok_or_else(unavailable)?,
                    },
                ));
            }
        }
        let ranked = rank_offers(plans, rates)?;
        let mut selected = None;
        let mut valid = vec![];
        let mut validations = 0;
        for (plan, offer) in ranked {
            if selected.is_some_and(|selected| selected != plan) {
                continue;
            }
            if validations == 10 {
                break;
            }
            validations += 1;
            packages[plan]["service_id"] = provider_id(&offer.id);
            let response = self
                .request(
                    reqwest::Method::POST,
                    "/packages/validate",
                    2,
                    Some(&packages[plan]),
                )
                .await?;
            if response.status() == StatusCode::NO_CONTENT {
                selected = Some(plan);
                valid.push(offer);
            } else {
                match provider_json(response).await {
                    Err(Error::Validation(v)) => rejection = Some(Error::Validation(v)),
                    Err(Error::NoService(_)) => (),
                    Err(e) => return Err(e),
                    Ok(_) => return Err(unavailable()),
                }
            }
            if valid.len() == 3 {
                break;
            }
        }
        if let Some(plan) = selected {
            return Ok((
                plan,
                Quote {
                    offers: valid,
                    point,
                    manual_required: false,
                    reason: None,
                    expires_at: None,
                },
            ));
        }
        if let Some(error) = rejection {
            return Err(error);
        }
        Ok((
            0,
            Quote::manual(
                "Brak zatwierdzonej usługi dla sprawdzonych paczek. Poproś o indywidualną wycenę.",
            ),
        ))
    }
}
fn rank_offers(
    plans: &[Estimate],
    offers: Vec<(usize, Offer)>,
) -> Result<Vec<(usize, Offer)>, Error> {
    let mut ranked = offers
        .into_iter()
        .map(|(plan, offer)| {
            let packing = plans.get(plan).ok_or_else(unavailable)?;
            let total = offer
                .price_grosze
                .checked_add(packing.packaging_grosze)
                .ok_or_else(unavailable)?;
            Ok((total, packing.parcels.len(), plan, offer))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    ranked.sort_by(|a, b| (a.0, a.1, a.2, &a.3.id).cmp(&(b.0, b.1, b.2, &b.3.id)));
    Ok(ranked
        .into_iter()
        .map(|(_, _, plan, offer)| (plan, offer))
        .collect())
}

fn id(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64)
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|v| v.to_string()))
}
fn provider_id(id: &str) -> Value {
    id.parse::<u64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::from(id))
}
fn money(value: &Value) -> Option<u32> {
    let text = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_number().map(ToString::to_string))?;
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty()
        || whole.len() > 6
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let amount =
        whole
            .parse::<u32>()
            .ok()?
            .checked_mul(100)?
            .checked_add(match fraction.len() {
                0 => 0,
                1 => fraction.parse::<u32>().ok()? * 10,
                _ => fraction.parse().ok()?,
            })?;
    (amount > 0 && amount <= 10_000_000).then_some(amount)
}
fn classify_rejection(body: &Value) -> Option<Error> {
    let mut fields = std::collections::BTreeMap::new();
    for issue in body["errors"].as_array().into_iter().flatten() {
        let path = issue["path"]
            .as_str()
            .unwrap_or("")
            .strip_prefix("/package")
            .unwrap_or(issue["path"].as_str().unwrap_or(""));
        if path.starts_with("/pickup/") {
            return Some(Error::Unavailable(
                "Konfiguracja adresu nadawcy wymaga poprawy przez obsługę.".into(),
            ));
        }
        let field = match path {
            "/receiver/street" => "street",
            "/receiver/postcode" => "postal_code",
            "/receiver/city" => "city",
            "/receiver/phone" => "phone",
            "/receiver/email" => "email",
            "/receiver/name" => "name",
            "/receiver/point" => "delivery.point_id",
            _ => continue,
        };
        fields.insert(
            field.into(),
            "Przewoźnik wymaga poprawienia tego pola.".into(),
        );
    }
    (!fields.is_empty()).then_some(Error::Validation(Validation {
        field_errors: fields,
    }))
}
fn rejection(body: &Value) -> Error {
    if let Some(error) = classify_rejection(body) {
        return error;
    }
    if body["errors"].as_array().is_some_and(|errors| {
        errors.iter().any(|issue| {
            issue["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("/parcels/") || path == "/service_id")
        })
    }) {
        Error::NoService("Przewoźnik nie przyjmuje tych paczek lub tej metody dostawy.".into())
    } else {
        unavailable()
    }
}
// Only trusted endpoint templates enter logs; IDs, queries and addresses never do.
fn request_operation(path: &str) -> &'static str {
    if path.starts_with("/pickup-commands/") {
        "pickup_command"
    } else if path.starts_with("/cancel-command/") {
        "cancel_command"
    } else if path.starts_with("/order-commands/") {
        "order_command"
    } else if path.starts_with("/points") {
        "pickup_points"
    } else if path.starts_with("/services") {
        "services"
    } else if path.starts_with("/packages/pickup-date-proposals") {
        "pickup_proposals"
    } else if path.starts_with("/packages/validate") {
        "validate_packages"
    } else if path.starts_with("/packages/calculate-price") {
        "quote_packages"
    } else if path.contains("/label") {
        "shipment_label"
    } else if path.contains("/tracking") {
        "shipment_tracking"
    } else {
        "packages"
    }
}

async fn provider_json(mut response: Response) -> Result<Value, Error> {
    let status = response.status();
    if status == StatusCode::UNAUTHORIZED
        || status == StatusCode::FORBIDDEN
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
    {
        return Err(unavailable());
    }
    let mut body = vec![];
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if body.len() + chunk.len() > 1_000_000 {
            return Err(unavailable());
        }
        body.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&body).map_err(|_| unavailable())?;
    if status.is_success() {
        Ok(value)
    } else if status == StatusCode::BAD_REQUEST
        || status == StatusCode::UNPROCESSABLE_ENTITY
        || status == StatusCode::CONFLICT
    {
        Err(rejection(&value))
    } else {
        Err(unavailable())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_key_uses_delivery_inputs_and_ignores_model_placements() {
        let mut plans = rapidmaker_packing::candidates(rapidmaker_packing::EstimateRequest {
            parts: vec![rapidmaker_packing::Part {
                id: "private-part-id".into(),
                quantity: 12,
                analysis_id: None,
                dimensions_mm: Some([120, 80, 45]),
                weight_g: Some(60),
            }],
            delivery: rapidmaker_packing::Delivery::Courier,
        });
        assert!(plans.len() > 1);
        let mut recipient = Recipient::default();
        let mut choice = DeliveryChoice::default();
        let key = quote_key(&plans, &recipient, &choice).unwrap();
        plans[0].parcels[0].placements[0].part_id = "other-part".into();
        plans[0].parcels[0].placements[0].position_mm = [5; 3];
        assert_eq!(quote_key(&plans, &recipient, &choice).unwrap(), key);
        plans[0].parcels[0].weight_g += 1;
        assert_ne!(quote_key(&plans, &recipient, &choice).unwrap(), key);
        plans[0].parcels[0].weight_g -= 1;
        plans[0].packaging_grosze += 1;
        assert_ne!(quote_key(&plans, &recipient, &choice).unwrap(), key);
        plans[0].packaging_grosze -= 1;
        recipient.email = "changed@example.com".into();
        assert_ne!(quote_key(&plans, &recipient, &choice).unwrap(), key);
        recipient.email.clear();
        choice.method = Method::Locker;
        assert_ne!(quote_key(&plans, &recipient, &choice).unwrap(), key);
        choice.method = Method::Courier;
        plans.swap(0, 1);
        assert_ne!(quote_key(&plans, &recipient, &choice).unwrap(), key);
    }
    #[tokio::test]
    async fn quote_cache_coalesces_preserves_expiry_and_cools_down_errors() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let cache = QuoteCache::default();
        let key = b"normalized request".to_vec();
        let calls = AtomicUsize::new(0);
        let slots = Semaphore::new(1);
        let load = || async {
            let _permit = slots.try_acquire().map_err(|_| unavailable())?;
            calls.fetch_add(1, Ordering::Relaxed);
            tokio::task::yield_now().await;
            Ok((1, Quote::pickup()))
        };
        let (first, second) = tokio::join!(
            cache.get(key.clone(), load()),
            cache.get(key.clone(), load())
        );
        let expiry = first.unwrap().1.expires_at;
        assert!(expiry.is_some());
        assert_eq!(second.unwrap().1.expires_at, expiry);
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let cached = cache
            .get(key.clone(), async { panic!("cache miss") })
            .await
            .unwrap();
        assert_eq!(cached.1.expires_at, expiry);
        {
            let mut entries = cache.0.lock().await;
            Arc::get_mut(entries.get_mut(&key).unwrap())
                .unwrap()
                .get_mut()
                .unwrap()
                .0 = Instant::now() - QUOTE_TTL;
        }
        assert!(
            cache
                .get(key.clone(), async { Err(unavailable()) })
                .await
                .is_err()
        );
        assert!(
            cache
                .get(key.clone(), async { panic!("error cooldown lost") })
                .await
                .is_err()
        );
        {
            let mut entries = cache.0.lock().await;
            Arc::get_mut(entries.get_mut(&key).unwrap())
                .unwrap()
                .get_mut()
                .unwrap()
                .0 = Instant::now() - QUOTE_ERROR_TTL;
        }
        cache.get(key, load()).await.unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
    #[tokio::test]
    async fn quote_cache_bounds_capacity_and_recovers_after_cancellation() {
        let cache = Arc::new(QuoteCache::default());
        assert!(
            cache
                .get(vec![0; MAX_QUOTE_KEY_BYTES + 1], async {
                    panic!("oversized key")
                })
                .await
                .is_err()
        );
        let pending_cache = Arc::clone(&cache);
        let (started, entered) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            pending_cache
                .get(vec![0], async {
                    started.send(()).unwrap();
                    std::future::pending().await
                })
                .await
        });
        entered.await.unwrap();
        for key in 1..MAX_QUOTE_KEYS as u8 {
            cache
                .get(vec![key], async { Ok((0, Quote::pickup())) })
                .await
                .unwrap();
        }
        assert!(
            cache
                .get(vec![255], async { panic!("full cache") })
                .await
                .is_err()
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        cache
            .get(vec![0], async { Ok((0, Quote::manual("manual"))) })
            .await
            .unwrap();
        assert_eq!(cache.0.lock().await.len(), MAX_QUOTE_KEYS);
        assert!(
            cache
                .get(vec![0], async { panic!("manual cooldown lost") })
                .await
                .unwrap()
                .1
                .manual_required
        );
    }
    #[tokio::test]
    async fn services_cache_coalesces_refreshes_expires_and_does_not_cache_failures() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let api = Sandbox::new("test-token".into(), json!({})).unwrap();
        let other = api.clone();
        assert!(Arc::ptr_eq(&api.services, &other.services));
        let calls = AtomicUsize::new(0);
        let load = || async {
            calls.fetch_add(1, Ordering::Relaxed);
            tokio::task::yield_now().await;
            Ok(json!({"services":[{"id":1,"service":"dpd"}]}))
        };
        let (first, second) = tokio::join!(api.services.get(load()), other.services.get(load()));
        assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        api.services.0.lock().await.as_mut().unwrap().0 = Instant::now() - SERVICES_TTL;
        assert!(
            api.services
                .get(async { Err(unavailable()) })
                .await
                .is_err()
        );
        assert!(
            api.services
                .get(async { Ok(json!({"unexpected":"response"})) })
                .await
                .is_err()
        );
        api.services.get(load()).await.unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
    #[tokio::test]
    async fn services_refresh_cancellation_releases_waiters() {
        let cache = Arc::new(ServicesCache::default());
        let pending_cache = Arc::clone(&cache);
        let (started, entered) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            pending_cache
                .get(async {
                    started.send(()).unwrap();
                    std::future::pending().await
                })
                .await
        });
        entered.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            cache.get(async { Ok(json!({"services":[]})) }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result["services"], json!([]));
    }
    #[test]
    fn ranking_compares_shipping_plus_packaging_and_keeps_plan_identity() {
        let mut plans = rapidmaker_packing::candidates(rapidmaker_packing::EstimateRequest {
            parts: vec![rapidmaker_packing::Part {
                id: "part".into(),
                quantity: 12,
                analysis_id: None,
                dimensions_mm: Some([120, 80, 45]),
                weight_g: Some(60),
            }],
            delivery: rapidmaker_packing::Delivery::Courier,
        });
        assert!(plans.len() > 1);
        plans[0].packaging_grosze = 900;
        plans[1].packaging_grosze = 250;
        let offer = |id: &str, price| Offer {
            id: id.into(),
            name: id.into(),
            price_grosze: price,
        };
        let ranked = rank_offers(
            &plans,
            vec![
                (0, offer("cheap-postage", 1000)),
                (1, offer("cheap-total", 1500)),
                (0, offer("other-carrier", 2000)),
            ],
        )
        .unwrap();
        assert_eq!(
            ranked
                .iter()
                .map(|(plan, o)| (*plan, o.id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (1, "cheap-total"),
                (0, "cheap-postage"),
                (0, "other-carrier")
            ]
        );
        plans[0].packaging_grosze = u32::MAX;
        assert!(rank_offers(&plans, vec![(0, offer("overflow", 1))]).is_err());
        assert!(rank_offers(&plans, vec![(plans.len(), offer("bad-plan", 1))]).is_err());
    }
    #[tokio::test]
    #[ignore = "requires FURGONETKA_SANDBOX_TOKEN and FURGONETKA_SENDER_JSON; only quotes, never ships"]
    async fn sandbox_compares_actual_prices_for_multiple_plans() {
        let token = std::env::var("FURGONETKA_SANDBOX_TOKEN").unwrap();
        let sender: Value =
            serde_json::from_str(&std::env::var("FURGONETKA_SENDER_JSON").unwrap()).unwrap();
        let recipient = Recipient {
            name: "Rapidmaker test".into(),
            email: "void@sieciowiec.xyz".into(),
            phone: "+48500100200".into(),
            street: "Jarzębinowa 3".into(),
            postal_code: "05-500".into(),
            city: "Piaseczno".into(),
            country: "PL".into(),
            ..Default::default()
        };
        let api = Sandbox::new(token, sender).unwrap();
        let mut plans = rapidmaker_packing::candidates(rapidmaker_packing::EstimateRequest {
            parts: vec![rapidmaker_packing::Part {
                id: "part".into(),
                quantity: 12,
                analysis_id: None,
                dimensions_mm: Some([120, 80, 45]),
                weight_g: Some(60),
            }],
            delivery: rapidmaker_packing::Delivery::Courier,
        });
        plans.truncate(3);
        assert!(plans.len() > 1);
        let mut expected = u32::MAX;
        for (i, plan) in plans.iter().enumerate() {
            let (_, quote) = api
                .quote_best(
                    std::slice::from_ref(plan),
                    &recipient,
                    &DeliveryChoice::default(),
                )
                .await
                .unwrap();
            for offer in quote.offers {
                let total = offer.price_grosze + plan.packaging_grosze;
                expected = expected.min(total);
                eprintln!(
                    "sandbox plan={i} cartons={:?} pieces_per_parcel={:?} shipping={} packaging={} total={total}",
                    plan.parcels.iter().map(|p| p.box_id).collect::<Vec<_>>(),
                    plan.parcels
                        .iter()
                        .map(|p| p.placements.len())
                        .collect::<Vec<_>>(),
                    offer.price_grosze,
                    plan.packaging_grosze
                );
            }
        }
        let (selected, quote) = api
            .quote_best(&plans, &recipient, &DeliveryChoice::default())
            .await
            .unwrap();
        assert!(!quote.manual_required);
        let best = quote.offers.first().unwrap().price_grosze + plans[selected].packaging_grosze;
        assert_eq!(best, expected);
        assert!(
            quote
                .offers
                .windows(2)
                .all(|o| o[0].price_grosze <= o[1].price_grosze)
        );
        eprintln!(
            "sandbox cheapest plan={selected} parcels={} total={best}",
            plans[selected].parcels.len()
        );
    }
    #[test]
    fn provider_wire_contract_and_unstructured_failures() {
        let api = Sandbox::new("test-token".into(), json!({})).unwrap();
        let request = api
            .build_request(reqwest::Method::GET, "/account/services", 1, None)
            .unwrap();
        assert!(!request.headers().contains_key("Content-Type"));
        assert!(request.body().is_none());
        let request = api
            .build_request(
                reqwest::Method::POST,
                "/packages/validate",
                2,
                Some(&json!({})),
            )
            .unwrap();
        assert_eq!(
            request.headers()["Content-Type"],
            "application/vnd.furgonetka.v2+json"
        );
        assert!(matches!(
            rejection(&json!({"message":"Bad request"})),
            Error::Unavailable(_)
        ));
        assert!(matches!(
            rejection(
                &json!({"errors":[{"path":"/parcels/0/weight","code":"packageWeightLessThan"}]})
            ),
            Error::NoService(_)
        ));
    }
    #[test]
    fn carrier_amount_and_errors_are_not_silently_reinterpreted() {
        assert_eq!(money(&json!("16.99")), Some(1699));
        assert_eq!(money(&json!(16.9)), Some(1690));
        for value in [
            json!(0),
            json!(-1),
            json!("1.001"),
            json!("1e3"),
            json!("NaN"),
        ] {
            assert_eq!(money(&value), None);
        }
        assert!(matches!(
            classify_rejection(&json!({"errors":[{"path":"/pickup/street"}]})),
            Some(Error::Unavailable(_))
        ));
        let Some(Error::Validation(v)) =
            classify_rejection(&json!({"errors":[{"path":"/receiver/postcode"}]}))
        else {
            panic!("field error lost")
        };
        assert!(v.field_errors.contains_key("postal_code"));
        assert!(classify_rejection(&json!({"errors":[{"path":"/parcels/0/weight"}]})).is_none());
    }
}
