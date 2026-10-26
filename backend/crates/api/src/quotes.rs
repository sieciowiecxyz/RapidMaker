//! Durable measured quotes; only the worker invokes the isolated slicer.
use axum::{
    Json,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_pricing::Rates;
use rapidmaker_slicer::Options;
use rapidmaker_storage::{Material, SlicerJob, Storage};
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
    Ok(Json(customer_catalog(&catalog)))
}
// Canonical production profiles behind the fixed five-family storefront and legacy orders.
const CUSTOMER_MATERIALS: &[(&str, &str)] = &[
    ("GFA00", "PLA"),
    ("GFB00", "ABS"),
    ("GFU01", "TPU"),
    ("GFG02", "PETG"),
    ("GFB01", "ASA"),
    ("GFN03", "PA-CF"),
    ("GFA01", "PLA Matte"),
    ("GFA02", "PLA Metal"),
    ("GFA05", "PLA Silk"),
    ("GFA06", "PLA Silk+"),
    ("GFA07", "PLA Marble"),
    ("GFA08", "PLA Sparkle"),
    ("GFA09", "PLA Tough"),
    ("GFA10", "PLA Tough+"),
    ("GFA11", "PLA Aero"),
    ("GFA12", "PLA Glow"),
    ("GFA13", "PLA Dynamic"),
    ("GFA15", "PLA Galaxy"),
    ("GFA16", "PLA Wood"),
    ("GFA17", "PLA Translucent"),
    ("GFA18", "PLA Lite"),
    ("GFA19", "PLA Pure"),
    ("GFA50", "PLA-CF"),
];
pub(super) fn customer_material(id: &str) -> bool {
    CUSTOMER_MATERIALS.iter().any(|(known, _)| *known == id)
}
pub(super) fn approved_cost(material: &Material) -> Option<u32> {
    if !customer_material(&material.id)
        || !material.production_enabled
        || !rapidmaker_slicer::material_supported(&material.id)
    {
        return None;
    }
    material
        .cost_net_grosze_per_kg
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
}
pub(super) fn known_color(material: &Material, code: &str) -> bool {
    material.colors.iter().any(|color| color.code == code)
        || (material.id == "GFN03" && material.colors.is_empty() && code == "black")
}
fn customer_catalog(catalog: &[Material]) -> Value {
    json!({"materials":CUSTOMER_MATERIALS.iter().filter_map(|(id,name)|catalog.iter().find(|m|m.id==*id).map(|m| {
        // PA-CF has no upstream swatch list; black is our explicit storefront choice, not a Bambu SKU.
        let colors = if m.id == "GFN03" && m.colors.is_empty() {
            json!([{"code":"black","name":"Black","rgba":["#000000FF"],"in_stock":false}])
        } else { json!(m.colors) };
        json!({"id":m.id,"name":name,"density_g_cm3":m.density_g_cm3,"colors":colors,
            "available":approved_cost(m).is_some() && colors.as_array().is_some_and(|c| !c.is_empty())})
    })).collect::<Vec<_>>()})
}

fn manual(reason: &str, message: &str) -> Error {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"code":"manual_quote_required","reason":reason,"message":message})),
    )
}
fn automatic_geometry(dimensions: [f64; 3]) -> Result<(), Error> {
    use rapidmaker_slicer::{ModelFitError, check_model_dimensions};
    check_model_dimensions(dimensions).map_err(|e|match e {
        ModelFitError::InvalidDimensions=>error(StatusCode::BAD_REQUEST,"Model nie ma prawidłowych wymiarów przestrzennych."),
        ModelFitError::SizeLimit=>manual("size_limit","Model przekracza 500 mm na osi. Poproś o wycenę indywidualną."),
        ModelFitError::PrinterFit=>manual("printer_fit","Model wymaga indywidualnego ułożenia, podziału lub innej drukarki. Poproś o wycenę indywidualną."),
    })
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
    input.options.validate().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Wybierz ilość 1–1000, jakość draft/standard/fine, 2–8 ścian i wypełnienie 5–100%.",
        )
    })?;
    let model = super::files::resolve(storage, &headers, &input.model_id).await?;
    automatic_geometry(model.record.dimensions_mm)?;
    let material = storage
        .materials()
        .await
        .map_err(|_| unavailable())?
        .into_iter()
        .find(|m| m.id == input.material_id)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Nieznany materiał."))?;
    if !customer_material(&material.id) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Wybierz materiał obsługiwany przez konfigurator.",
        ));
    }
    if !known_color(&material, &input.color_code) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Wybrany kolor nie należy do tego materiału.",
        ));
    }
    let cost=approved_cost(&material).ok_or_else(||manual("profile","Materiał wymaga zatwierdzenia bezpiecznego profilu i jawnej ceny zakupu. Poproś o wycenę indywidualną."))?;
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
    tracing::info!(event="slicer_queued", job_id=%job.id, model_id=%model.record.id);
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

#[cfg(test)]
mod tests {
    use super::*;
    use rapidmaker_storage::Color;
    fn material(id: &str) -> Material {
        Material {
            id: id.into(),
            name: "Operator profile name".into(),
            density_g_cm3: 1.24,
            cost_net_grosze_per_kg: Some(8000),
            production_enabled: true,
            source: json!({}),
            colors: vec![Color {
                code: "known".into(),
                name: "Black".into(),
                rgba: vec!["#000000FF".into()],
                in_stock: false,
            }],
        }
    }
    #[test]
    fn customer_catalog_preserves_approved_profiles_and_colors_without_stock() {
        let mut pla = material("GFA00");
        pla.colors.push(Color {
            code: "second".into(),
            name: "White".into(),
            rgba: vec!["#FFFFFFFF".into()],
            in_stock: false,
        });
        let rows = vec![
            material("GFS00"),
            material("GFA50"),
            material("GFB00"),
            material("GFU01"),
            pla,
        ];
        let catalog = customer_catalog(&rows);
        let families = catalog["materials"].as_array().unwrap();
        assert_eq!(
            families
                .iter()
                .map(|m| m["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["PLA", "ABS", "TPU", "PLA-CF"]
        );
        assert_eq!(families[0]["colors"].as_array().unwrap().len(), 2);
        assert!(families.iter().all(|m| m["available"] == true));
        assert!(
            families
                .iter()
                .flat_map(|m| m["colors"].as_array().unwrap())
                .all(|c| c["in_stock"] == false)
        );
        assert!(known_color(&rows[4], "second"));
        assert!(!known_color(&rows[1], "second"));
        assert!(!known_color(&rows[4], "invented"));
        assert_eq!(approved_cost(&rows[0]), None);
        assert_eq!(approved_cost(&rows[1]), Some(8000));
        let mut tpu = material("GFU01");
        tpu.production_enabled = false;
        assert_eq!(customer_catalog(&[tpu])["materials"][0]["available"], false);
    }
    #[test]
    fn pa_black_choice_does_not_implicitly_approve_production() {
        let mut pa = material("GFN03");
        pa.colors.clear();
        pa.production_enabled = false;
        assert!(known_color(&pa, "black"));
        assert!(!known_color(&pa, "invented"));
        assert_eq!(customer_catalog(&[pa])["materials"][0]["available"], false);
    }
    #[test]
    fn stock_does_not_change_measured_price_but_profile_and_purchase_price_remain_required() {
        let mut pla = material("GFA00");
        let rates = Rates {
            version: "fixture".into(),
            setup_grosze: 1200,
            handling_per_plate_grosze: 200,
            machine_per_hour_grosze: 1000,
            energy_per_kwh_grosze: 120,
            printer_watts: 200,
            loss_bps: 500,
            margin_bps: 3000,
            vat_bps: 2300,
            minimum_net_grosze: 0,
        };
        let price =
            rapidmaker_pricing::calculate(3600, 100_000, 2, approved_cost(&pla).unwrap(), &rates)
                .unwrap();
        assert!(known_color(&pla, "known"));
        assert_eq!(
            (
                price.filament_grosze,
                price.energy_grosze,
                price.handling_grosze
            ),
            (800, 24, 400)
        );
        assert_eq!(
            (
                price.cost_net_grosze,
                price.net_grosze,
                price.vat_grosze,
                price.gross_grosze
            ),
            (3596, 5138, 1182, 6320)
        );
        pla.colors[0].in_stock = true;
        let stocked =
            rapidmaker_pricing::calculate(3600, 100_000, 2, approved_cost(&pla).unwrap(), &rates)
                .unwrap();
        assert_eq!(price.gross_grosze, stocked.gross_grosze);
        pla.production_enabled = false;
        assert_eq!(approved_cost(&pla), None);
        pla.production_enabled = true;
        for cost in [None, Some(0), Some(-1), Some(i64::MAX)] {
            pla.cost_net_grosze_per_kg = cost;
            assert_eq!(approved_cost(&pla), None);
        }
    }
    #[test]
    fn unprintable_geometry_has_explicit_manual_routing_without_faked_dimensions() {
        for (dimensions, reason) in [
            ([500.01, 1.0, 1.0], "size_limit"),
            ([256.0, 239.0, 250.0], "printer_fit"),
        ] {
            let (status, Json(body)) = automatic_geometry(dimensions).unwrap_err();
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(body["code"], "manual_quote_required");
            assert_eq!(body["reason"], reason);
        }
        assert!(automatic_geometry([238.0, 256.0, 250.0]).is_ok());
        assert!(automatic_geometry([200.0, 10.0, 251.0]).is_ok());
        assert_eq!(
            automatic_geometry([f64::NAN, 1.0, 1.0]).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }
    #[tokio::test]
    #[ignore = "requires isolated F1_TEST_DATABASE_URL"]
    async fn postgres_customer_catalog_ignores_stock_and_keeps_operator_profiles() {
        let url = std::env::var("F1_TEST_DATABASE_URL").expect("dedicated test database");
        let parsed = rapidmaker_auth::passkeys::Url::parse(&url).unwrap();
        assert!(parsed.path().starts_with("/rapidmaker_test_"));
        let storage = Storage::connect(&url).await.unwrap();
        storage.import_bambu().await.unwrap();
        let pool = sqlx::PgPool::connect(&url).await.unwrap();
        sqlx::query("UPDATE catalog_colors SET in_stock=false")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE catalog_materials SET production_enabled=true,cost_net_grosze_per_kg=8000 WHERE id IN ('GFA00','GFB00','GFU01')").execute(&pool).await.unwrap();
        let operator = storage.materials().await.unwrap();
        assert_eq!(operator.len(), 49);
        let app = Arc::new(App {
            storage: Some(storage),
            rates: None,
        });
        let Json(catalog) = materials(State(app.clone())).await.unwrap();
        let families = catalog["materials"].as_array().unwrap();
        assert_eq!(families.len(), CUSTOMER_MATERIALS.len());
        for family in families {
            let profile = operator.iter().find(|m| family["id"] == m.id).unwrap();
            assert_eq!(
                family["available"],
                approved_cost(profile).is_some() && !profile.colors.is_empty()
            );
            assert_eq!(
                family["colors"].as_array().unwrap().len(),
                profile.colors.len()
            );
            assert!(
                profile
                    .colors
                    .iter()
                    .all(|c| !c.in_stock && known_color(profile, &c.code))
            );
            assert_eq!(
                approved_cost(profile).is_some(),
                ["GFA00", "GFB00", "GFU01"].contains(&profile.id.as_str())
            );
        }
        sqlx::query("UPDATE catalog_materials SET cost_net_grosze_per_kg=NULL WHERE id='GFU01'")
            .execute(&pool)
            .await
            .unwrap();
        let Json(catalog) = materials(State(app)).await.unwrap();
        assert_eq!(catalog["materials"][2]["available"], false);
        assert!(
            !catalog["materials"][2]["colors"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        pool.close().await;
    }
}
