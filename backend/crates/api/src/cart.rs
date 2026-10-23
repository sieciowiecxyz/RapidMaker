//! Checkout consumes owned, immutable slicer quotes, never browser prices or geometry.
use axum::{
    Json,
    http::{HeaderMap, StatusCode},
};
use rapidmaker_packing::Part;
use rapidmaker_pricing::{Price, PrintLine, Rates};
use rapidmaker_storage::Storage;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message":message})))
}
fn stale() -> Error {
    error(
        StatusCode::CONFLICT,
        "Wycena wygasła lub konfiguracja się zmieniła. Przelicz zamówienie.",
    )
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn owner(headers: &HeaderMap) -> Result<String, Error> {
    super::files::owner(headers).ok_or_else(|| {
        error(
            StatusCode::UNAUTHORIZED,
            "Prześlij model, aby utworzyć sesję zamówienia.",
        )
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CartPart {
    id: String,
    quantity: u32,
    analysis_id: Option<String>,
    quote_id: Option<String>,
}
pub struct Cart {
    pub owner: String,
    pub expires_at: u64,
    pub parts: Vec<Part>,
    pub lines: Vec<PrintLine>,
    pub setup: u32,
    pub total: u32,
    pub quote_ids: Vec<String>,
    pub manual: bool,
}
async fn rates(storage: &Storage) -> Result<Rates, Error> {
    let configured = storage.pricing_config().await.map_err(|_| stale())?;
    let rates = if let Some(value) = configured {
        serde_json::from_value(value)
    } else {
        serde_json::from_str(&std::env::var("PRICING_RATES_JSON").unwrap_or_default())
    }
    .map_err(|_| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Operator musi skonfigurować cennik.",
        )
    })?;
    Rates::validate(&rates).map_err(|_| stale())?;
    Ok(rates)
}
// Recheck availability and policy before issuing a checkout offer or opening payment.
pub async fn validate(
    storage: &Storage,
    owner: &str,
    ids: &[String],
) -> Result<Vec<(Value, Value)>, Error> {
    let policy = serde_json::to_value(rates(storage).await?).map_err(|_| stale())?;
    let catalog = storage.materials().await.map_err(|_| stale())?;
    let mut snapshots = Vec::new();
    for id in ids {
        let (request, result) = storage
            .quote(id, owner, now())
            .await
            .map_err(|_| stale())?
            .ok_or_else(stale)?;
        let material = catalog
            .iter()
            .find(|m| request["material_id"] == m.id)
            .ok_or_else(stale)?;
        if request["slicer_profile_version"] != rapidmaker_slicer::PROFILE_VERSION
            || request["rates"] != policy
            || !material.production_enabled
            || material.cost_net_grosze_per_kg != request["purchase_net_grosze_per_kg"].as_i64()
            || material.source["profiles_sha256"] != request["profiles_sha256"]
            || !material
                .colors
                .iter()
                .any(|c| c.in_stock && request["color_code"] == c.code)
        {
            return Err(stale());
        }
        let model_id = request["model_id"].as_str().ok_or_else(stale)?;
        if storage
            .model(model_id, owner, now())
            .await
            .map_err(|_| stale())?
            .is_none()
        {
            return Err(stale());
        }
        snapshots.push((request, result));
    }
    Ok(snapshots)
}
pub async fn prepare(
    storage: Option<&Storage>,
    headers: &HeaderMap,
    input: Vec<CartPart>,
    manual: bool,
) -> Result<Cart, Error> {
    if input.is_empty() || input.len() > 10 {
        return Err(error(StatusCode::BAD_REQUEST, "Dodaj od 1 do 10 części."));
    }
    let mut unique = HashSet::new();
    if input.iter().any(|p| {
        p.id.is_empty()
            || p.id.len() > 128
            || !(1..=1000).contains(&p.quantity)
            || !unique.insert(p.id.clone())
    }) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Sprawdź części i liczbę sztuk (1–1000).",
        ));
    }
    let owner = owner(headers)?;
    let mut cart = Cart {
        owner,
        expires_at: (now() + 600) as u64,
        parts: Vec::new(),
        lines: Vec::new(),
        setup: 0,
        total: 0,
        quote_ids: Vec::new(),
        manual,
    };
    if manual
        || input
            .iter()
            .any(|p| p.analysis_id.is_none() || p.quote_id.is_none())
    {
        cart.manual = true;
        return Ok(cart);
    }
    let storage = storage.ok_or_else(|| {
        error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Magazyn wycen jest niedostępny.",
        )
    })?;
    cart.quote_ids = input.iter().filter_map(|p| p.quote_id.clone()).collect();
    let snapshots = validate(storage, &cart.owner, &cart.quote_ids).await?;
    let policy: Rates =
        serde_json::from_value(snapshots[0].0["rates"].clone()).map_err(|_| stale())?;
    let mut prices = Vec::new();
    for (part, (request, result)) in input.iter().zip(snapshots) {
        if request["model_id"].as_str() != part.analysis_id.as_deref()
            || request["quantity"].as_u64() != Some(part.quantity.into())
        {
            return Err(stale());
        }
        let dimensions: [f64; 3] =
            serde_json::from_value(request["dimensions_mm"].clone()).map_err(|_| stale())?;
        let mass = result["measurement"]["filament_grams"]
            .as_f64()
            .filter(|g| g.is_finite() && *g > 0.0 && *g <= 1_000_000.0)
            .ok_or_else(stale)?;
        let price: Price = serde_json::from_value(result["price"].clone()).map_err(|_| stale())?;
        cart.expires_at = cart
            .expires_at
            .min(request["expires_at"].as_u64().ok_or_else(stale)?);
        cart.parts.push(Part {
            id: part.id.clone(),
            quantity: part.quantity,
            analysis_id: None,
            dimensions_mm: Some(dimensions.map(|v| v.ceil() as u32)),
            weight_g: Some((mass / f64::from(part.quantity)).ceil() as u32),
        });
        prices.push((part.id.clone(), part.quantity, price));
    }
    // The slicer mass includes supports/purge: conservative shipping upper bound, not finished mass.
    let inputs: Vec<_> = prices
        .iter()
        .map(|(id, quantity, price)| rapidmaker_pricing::OrderPart {
            model_id: id,
            quantity: *quantity,
            price,
        })
        .collect();
    let order = rapidmaker_pricing::combine(&inputs, &policy).map_err(|_| stale())?;
    let (price, lines, setup) = (order.price, order.lines, order.setup_grosze);
    cart.total = price.gross_grosze;
    cart.lines = lines;
    cart.setup = setup;
    Ok(cart)
}
