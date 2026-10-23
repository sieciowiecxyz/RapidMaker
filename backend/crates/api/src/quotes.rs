//! Durable measured quotes; only the worker invokes the isolated slicer.
use axum::{
    Json,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_pricing::Rates;
use rapidmaker_slicer::Options;
use rapidmaker_storage::{SlicerJob, Storage};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter as Router, routes};

type Error = (StatusCode, Json<Value>);
struct App {
    storage: Option<Storage>,
    rates: Option<Rates>,
}
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message": message})))
}
fn unavailable() -> Error {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Automatyczna wycena jest niedostępna. Spróbuj później lub poproś o wycenę indywidualną.",
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
        .routes(routes!(materials))
        .routes(routes!(estimate))
        .routes(routes!(status))
        .layer(DefaultBodyLimit::max(8192))
        .with_state(Arc::new(App { storage, rates }))
        .merge(admin)
        .merge(files))
}
#[utoipa::path(operation_id = "quotes_materials", get, path = "/api/catalog/materials", tag = "Katalog",
    responses((status = 200, description = "Materiały, kolory i dostępność", body = Value)))]
async fn materials(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    let catalog = app
        .storage
        .as_ref()
        .ok_or_else(unavailable)?
        .materials()
        .await
        .map_err(|_| unavailable())?;
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
#[derive(utoipa::ToSchema)]
#[schema(as = SlicerEstimateRequest)]
struct EstimateRequest {
    model_id: String,
    material_id: String,
    color_code: String,
    options: Options,
}
/// Server-generated snapshot. Never contains a browser-provided path or price.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SliceInput {
    pub model_sha256: String,
    pub material_id: String,
    pub color_code: String,
    pub options: Options,
    pub purchase_net_grosze_per_kg: u32,
    pub rates: Rates,
    pub profiles_sha256: Value,
    pub slicer_profile_version: String,
}
#[utoipa::path(operation_id = "quotes_estimate", post, path = "/api/quotes/estimate", tag = "Wycena", request_body = EstimateRequest,
    responses((status = 202, description = "Trwałe zadanie: id, status, poll_url i opcjonalny wynik", body = Value),
        (status = 400, description = "Nieprawidłowe opcje", body = Value), (status = 429, description = "Pełna kolejka", body = Value)),
    security(("session" = []), ("guest" = [])))]
async fn estimate(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<EstimateRequest>,
) -> Result<(StatusCode, Json<Value>), Error> {
    let storage = app.storage.as_ref().ok_or_else(unavailable)?;
    let rates = storage
        .pricing_config()
        .await
        .map_err(|_| unavailable())?
        .map(serde_json::from_value::<Rates>)
        .transpose()
        .map_err(|_| unavailable())?
        .or_else(|| app.rates.clone())
        .ok_or_else(unavailable)?;
    rates.validate().map_err(|_| unavailable())?;
    input.options.validate().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Wybierz ilość 1–1000, jakość draft/standard/fine, 2–8 ścian i wypełnienie 5–100%.",
        )
    })?;
    let model = super::files::resolve(storage, &headers, &input.model_id).await?;
    let material = storage
        .materials()
        .await
        .map_err(|_| unavailable())?
        .into_iter()
        .find(|m| m.id == input.material_id)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Nieznany materiał."))?;
    let cost = material
        .cost_net_grosze_per_kg
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| {
            *v > 0
                && material.production_enabled
                && rapidmaker_slicer::material_supported(&material.id)
        })
        .ok_or_else(|| {
            error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "Materiał wymaga zatwierdzenia profilu i ceny zakupu przez operatora.",
            )
        })?;
    if !material
        .colors
        .iter()
        .any(|c| c.code == input.color_code && c.in_stock)
    {
        return Err(error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Wybrany kolor jest niedostępny.",
        ));
    }
    let payload = serde_json::to_value(SliceInput {
        model_sha256: model.record.sha256,
        material_id: input.material_id,
        color_code: input.color_code,
        options: input.options,
        purchase_net_grosze_per_kg: cost,
        rates,
        profiles_sha256: material.source["profiles_sha256"].clone(),
        slicer_profile_version: rapidmaker_slicer::PROFILE_VERSION.into(),
    })
    .map_err(|_| unavailable())?;
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([input.model_id, payload])).map_err(|_| unavailable())?
        )
    );
    let job = storage
        .enqueue_slice(
            &model.record.owner_hash,
            &model.record.id,
            &fingerprint,
            &payload,
            super::cart::now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::TOO_MANY_REQUESTS,
                "Kolejka wycen jest pełna lub model wygasł. Spróbuj później.",
            )
        })?;
    Ok((StatusCode::ACCEPTED, Json(public_job(job))))
}
#[utoipa::path(operation_id = "quotes_status", get, path = "/api/quotes/jobs/{id}", tag = "Wycena", params(("id" = String, Path)),
    responses((status = 200, description = "queued/running/succeeded/failed; result po ukończeniu", body = Value),
        (status = 404, description = "Cudze lub wygasłe zadanie", body = Value)), security(("session" = []), ("guest" = [])))]
async fn status(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    let storage = app.storage.as_ref().ok_or_else(unavailable)?;
    let owner = super::cart::owner(Some(storage), &headers).await?;
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(error(StatusCode::NOT_FOUND, "Nie znaleziono zadania."));
    }
    let job = storage
        .slice_job(&id, &owner, super::cart::now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "Nie znaleziono zadania lub wygasło."))?;
    Ok(Json(public_job(job)))
}
fn public_job(job: SlicerJob) -> Value {
    let message = if job.status == "failed" {
        job.error.as_deref().map(|message| match message {
            "worker_interrupted" => {
                "Nie udało się wznowić wyceny po przerwaniu pracy. Zleć ją ponownie."
            }
            "job_expired" => "Upłynął czas oczekiwania na wycenę. Zleć ją ponownie.",
            other => other,
        })
    } else {
        None
    };
    json!({"id":job.id,"status":job.status,"poll_url":format!("/api/quotes/jobs/{}",job.id),
        "result":job.result,"message":message,"expires_at":job.expires_at})
}
