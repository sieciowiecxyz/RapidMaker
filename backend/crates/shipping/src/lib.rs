//! Domestic address validation and bounded Furgonetka sandbox adapter. No labels are purchased.
mod address;
pub use address::{DeliveryChoice, Invoice, Method, Recipient, Validation};
use rapidmaker_packing::Estimate;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;
use utoipa::ToSchema;

const API: &str = "https://api.sandbox.furgonetka.pl";
#[derive(Debug)]
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
        }
    }
    fn manual(reason: &str) -> Self {
        Self {
            offers: vec![],
            point: None,
            manual_required: true,
            reason: Some(reason.into()),
        }
    }
}
#[derive(Clone)]
pub struct Sandbox {
    client: Client,
    token: String,
    sender: Value,
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
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        version: u8,
        body: Option<&Value>,
    ) -> Result<Response, Error> {
        self.client
            .execute(self.build_request(method, path, version, body)?)
            .await
            .map_err(|_| unavailable())
    }
    pub async fn points(&self, query: &str) -> Result<Vec<Point>, Error> {
        match self.find_points(query, false).await {
            Err(Error::NoService(_)) => Err(unavailable()),
            result => result,
        }
    }
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
        tokio::time::timeout(
            Duration::from_secs(30),
            self.quote_inner(plans, &recipient, choice),
        )
        .await
        .map_err(|_| unavailable())?
    }
    async fn quote_inner(
        &self,
        plans: &[Estimate],
        recipient: &Recipient,
        choice: &DeliveryChoice,
    ) -> Result<(usize, Quote), Error> {
        let data = provider_json(
            self.request(reqwest::Method::GET, "/account/services", 1, None)
                .await?,
        )
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
                    "sandbox plan={i} parcels={} shipping={} packaging={} total={total}",
                    plan.parcels.len(),
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
