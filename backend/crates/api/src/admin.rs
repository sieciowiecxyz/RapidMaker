//! Local operator API. Browser mock roles never authorize database writes.
use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use rapidmaker_pricing::Rates;
use rapidmaker_storage::{Color, MaterialInput, Storage, validate_color};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;

struct App {
    storage: Option<Storage>,
    token_hash: Option<[u8; 32]>,
    fallback_rates: Option<Rates>,
}
type Error = (StatusCode, Json<Value>);
fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message":message})))
}
fn unavailable() -> Error {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Panel operatora nie jest skonfigurowany.",
    )
}
fn storage(app: &App) -> Result<&Storage, Error> {
    app.storage.as_ref().ok_or_else(unavailable)
}

pub fn router(
    database: Option<Storage>,
    fallback_rates: Option<Rates>,
) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let token_hash = std::env::var("ADMIN_API_TOKEN")
        .ok()
        .map(|token| {
            if !(32..=256).contains(&token.len()) {
                return Err("ADMIN_API_TOKEN must contain 32..256 bytes");
            }
            Ok(Sha256::digest(token.as_bytes()).into())
        })
        .transpose()?;
    let app = Arc::new(App {
        storage: database,
        token_hash,
        fallback_rates,
    });
    Ok(Router::new()
        .route("/api/admin/materials", get(materials))
        .route("/api/admin/materials/{id}", put(save_material))
        .route("/api/admin/materials/{id}/colors/{code}", put(save_color))
        .route("/api/admin/pricing", get(pricing).put(save_pricing))
        .route_layer(middleware::from_fn_with_state(app.clone(), authorize))
        .with_state(app))
}

async fn authorize(State(app): State<Arc<App>>, request: Request, next: Next) -> Response {
    let Some(expected) = &app.token_hash else {
        return unavailable().into_response();
    };
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() <= 256);
    if !supplied.is_some_and(|v| {
        let digest: [u8; 32] = Sha256::digest(v.as_bytes()).into();
        bool::from(digest.ct_eq(expected))
    }) {
        return error(StatusCode::UNAUTHORIZED, "Brak uprawnień operatora.").into_response();
    }
    next.run(request).await
}

async fn materials(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"materials": storage(&app)?.materials().await.map_err(|_| unavailable())?}),
    ))
}
async fn save_material(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    Json(input): Json<MaterialInput>,
) -> Result<Json<Value>, Error> {
    input.validate().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Sprawdź nazwę, gęstość i dodatnią cenę netto/kg. Włączenie materiału wymaga ceny.",
        )
    })?;
    storage(&app)?
        .save_material(&id, &input)
        .await
        .map_err(|_| {
            error(
                StatusCode::BAD_REQUEST,
                "Nie udało się zapisać materiału. Sprawdź identyfikator i dane.",
            )
        })?;
    Ok(Json(json!({"saved":true})))
}
async fn save_color(
    State(app): State<Arc<App>>,
    Path((id, code)): Path<(String, String)>,
    Json(input): Json<Color>,
) -> Result<Json<Value>, Error> {
    if code != input.code {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Kod koloru nie odpowiada adresowi żądania.",
        ));
    }
    validate_color(&input).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Podaj kod, nazwę i od 1 do 16 kolorów #RRGGBBAA.",
        )
    })?;
    storage(&app)?.save_color(&id, &input).await.map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Nie udało się zapisać koloru. Sprawdź materiał i dane.",
        )
    })?;
    Ok(Json(json!({"saved":true})))
}
async fn pricing(State(app): State<Arc<App>>) -> Result<Json<Value>, Error> {
    let configured = storage(&app)?
        .pricing_config()
        .await
        .map_err(|_| unavailable())?;
    let rates = match configured {
        Some(value) => value,
        None => serde_json::to_value(&app.fallback_rates).map_err(|_| unavailable())?,
    };
    Ok(Json(json!({"rates":rates})))
}
async fn save_pricing(
    State(app): State<Arc<App>>,
    Json(input): Json<Rates>,
) -> Result<Json<Value>, Error> {
    input.validate().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Sprawdź stawki i wersję cennika. Marża musi wynosić więcej niż 0 i mniej niż 100%.",
        )
    })?;
    let value = serde_json::to_value(input).map_err(|_| unavailable())?;
    if !storage(&app)?
        .save_pricing_config(&value)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(error(
            StatusCode::CONFLICT,
            "Zmiana stawek wymaga nowego numeru wersji cennika.",
        ));
    }
    Ok(Json(json!({"saved":true})))
}
