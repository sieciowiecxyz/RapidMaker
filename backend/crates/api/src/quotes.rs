//! First measured quote flow. Existing checkout is deliberately a separate legacy adapter.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use rapidmaker_pricing::{Rates, calculate};
use rapidmaker_slicer::{BatchEstimate, Options};
use rapidmaker_storage::Storage;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};

static UPLOAD: Semaphore = Semaphore::const_new(1);
type Error = (StatusCode, Json<Value>);
type Cache = HashMap<String, (Instant, BatchEstimate)>;
struct App {
    storage: Option<Storage>,
    rates: Option<Rates>,
    cache: Mutex<Cache>,
}
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message": message})))
}
fn unavailable() -> Error {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Automatyczna wycena nie jest skonfigurowana. Poproś o wycenę indywidualną.",
    )
}

pub fn router(
    storage: Option<Storage>,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let rates = std::env::var("PRICING_RATES_JSON")
        .ok()
        .map(|value| serde_json::from_str::<Rates>(&value))
        .transpose()?;
    if let Some(rates) = &rates {
        rates.validate()?;
    }
    let admin = super::admin::router(storage.clone(), rates.clone())?;
    let files = super::files::router(storage.clone())?;
    Ok(Router::new()
        .route("/api/catalog/materials", get(materials))
        .route(
            "/api/quotes/estimate",
            post(estimate).layer(DefaultBodyLimit::max(8192)),
        )
        .with_state(Arc::new(App {
            storage,
            rates,
            cache: Mutex::new(HashMap::new()),
        }))
        .merge(admin)
        .merge(files))
}

async fn materials(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    let storage = app.storage.as_ref().ok_or_else(unavailable)?;
    let catalog = storage.materials().await.map_err(|_| unavailable())?;
    // Purchase costs and raw production presets are operator data, not the public catalog.
    Ok(Json(
        json!({"materials": catalog.into_iter().map(|m| json!({
        "id":m.id,"name":m.name,"density_g_cm3":m.density_g_cm3,"colors":m.colors,
        "available":m.production_enabled && m.cost_net_grosze_per_kg.is_some()
            && m.colors.iter().any(|color| color.in_stock)
            && rapidmaker_slicer::material_supported(&m.id),
    })).collect::<Vec<_>>()}),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EstimateRequest {
    model_id: String,
    material_id: String,
    color_code: String,
    options: Options,
}

async fn estimate(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<EstimateRequest>,
) -> Result<Json<Value>, Error> {
    let storage = app.storage.as_ref().ok_or_else(unavailable)?;
    let configured = storage.pricing_config().await.map_err(|_| unavailable())?;
    let rates = configured
        .map(serde_json::from_value::<Rates>)
        .transpose()
        .map_err(|_| unavailable())?
        .or_else(|| app.rates.clone())
        .ok_or_else(unavailable)?;
    rates.validate().map_err(|_| unavailable())?;
    let model = super::files::resolve(storage, &headers, &input.model_id).await?;
    let material_id = input.material_id.as_str();
    let color = input.color_code.as_str();
    let options = input.options;
    let quantity = options.quantity;
    options.validate().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Wybierz jakość draft/standard/fine, 2–8 ścian i wypełnienie 5–100%.",
        )
    })?;
    let material = storage
        .materials()
        .await
        .map_err(|_| unavailable())?
        .into_iter()
        .find(|m| m.id == material_id)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Nieznany materiał."))?;
    let cost = material
        .cost_net_grosze_per_kg
        .and_then(|v| u32::try_from(v).ok())
        .filter(|_| material.production_enabled)
        .ok_or_else(|| {
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Materiał wymaga zatwierdzenia profilu i ceny zakupu przez operatora.",
            )
        })?;
    if !material
        .colors
        .iter()
        .any(|c| c.code == color && c.in_stock)
    {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Wybrany kolor jest niedostępny.",
        ));
    }
    let _permit = UPLOAD.try_acquire().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Trwa wycena innego modelu. Spróbuj za chwilę.",
        )
    })?;
    let model_hash = &model.record.sha256;
    let extension = &model.record.extension;
    let cache_key = format!(
        "{model_hash}:{extension}:{material_id}:{}:{}:{}",
        rapidmaker_slicer::PROFILE_VERSION,
        material.source["profiles_sha256"],
        serde_json::to_string(&options).map_err(|_| unavailable())?
    );
    let mut cache = app.cache.lock().await;
    cache.retain(|_, (created, _)| created.elapsed() < Duration::from_secs(600));
    if !cache.contains_key(&cache_key) {
        drop(cache);
        let measured = rapidmaker_slicer::slice_job(&model.mesh, material_id, &options).await.map_err(|e| match e {
            rapidmaker_slicer::Error::Busy => error(StatusCode::TOO_MANY_REQUESTS, "Slicer jest zajęty. Spróbuj za chwilę."),
            rapidmaker_slicer::Error::UnsupportedMaterial => error(StatusCode::UNPROCESSABLE_ENTITY, "Ten materiał wymaga ręcznej wyceny; brak zatwierdzonego profilu slicera."),
            _ => error(StatusCode::UNPROCESSABLE_ENTITY, "Nie udało się przygotować wyceny slicerem. Sprawdź model lub poproś o ocenę indywidualną."),
        })?;
        cache = app.cache.lock().await;
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(cache_key.clone(), (Instant::now(), measured));
    }
    let measured = &cache[&cache_key].1;
    let price = calculate(
        measured.time_seconds,
        (measured.filament_grams * 1000.0).ceil() as u64,
        measured.plates,
        cost,
        &rates,
    )
    .map_err(|_| {
        error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Ta konfiguracja wymaga indywidualnej wyceny.",
        )
    })?;
    let id = uuid::Uuid::new_v4().to_string();
    let expires_at = model.record.expires_at.min(super::cart::now() + 600);
    let request = json!({"owner_hash":model.record.owner_hash,"model_id":model.record.id,"expires_at":expires_at,"dimensions_mm":model.record.dimensions_mm,"model_sha256":model_hash,"material_id":material_id,"color_code":color,"quantity":quantity,
        "slicer_profile_version":rapidmaker_slicer::PROFILE_VERSION,"options":options,"purchase_net_grosze_per_kg":cost,"rates":rates,"profiles_sha256":material.source["profiles_sha256"]});
    let result = json!({"id":id,"model_id":model.record.id,"expires_at":expires_at,"status":"estimated_poc","currency":"PLN","quantity":quantity,
        "measurement":measured,"plates":measured.plates,"batch_strategy":"arranged_plate_batches","options":options,
        "price":price,"pricing_version":rates.version,"payable":false,
        "notice":"Wycena testowa z ułożenia modeli na płytach. Powtarzalne pełne płyty liczone z jednego pomiaru, ostatnia osobno. Dostawa i opakowanie osobno; kalibracja i checkout pozostają do podłączenia."});
    drop(cache);
    storage
        .save_quote(&id, &request, &result)
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(result))
}
