//! Operator endpoints require the shared database session and verified passkey.
use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use rapidmaker_pricing::Rates;
use rapidmaker_storage::{Color, MaterialInput, Storage, validate_color};
use serde_json::{Value, json};
use std::sync::Arc;

struct App {
    storage: Option<Storage>,
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
    let orders = super::orders::admin_router(database.clone());
    let app = Arc::new(App {
        storage: database,
        fallback_rates,
    });
    Ok(Router::new()
        .merge(orders)
        .route("/api/admin/materials", get(materials))
        .route("/api/admin/materials/{id}", put(save_material))
        .route("/api/admin/materials/{id}/colors/{code}", put(save_color))
        .route("/api/admin/pricing", get(pricing).put(save_pricing))
        .route_layer(middleware::from_fn(authorize))
        .route("/api/admin/session", get(session))
        .with_state(app))
}

async fn session(
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    if !auth.enabled() {
        return Ok(Json(
            json!({"configured":false,"authenticated":false,"admin_verified":false,"session":null}),
        ));
    }
    match auth.administrator(&headers, false).await {
        Ok(session) => Ok(Json(
            json!({"configured":true,"authenticated":true,"admin_verified":true,"session":session.user,"expires_at":session.expires_at}),
        )),
        Err((StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN, _)) => Ok(Json(
            json!({"configured":true,"authenticated":false,"admin_verified":false,"session":null}),
        )),
        Err(error) => Err(error),
    }
}

async fn authorize(
    Extension(auth): Extension<Arc<super::auth::Auth>>,
    request: Request,
    next: Next,
) -> Response {
    let mutate = !matches!(*request.method(), Method::GET | Method::HEAD);
    match auth.administrator(request.headers(), mutate).await {
        Ok(_) => next.run(request).await,
        Err(error) => error.into_response(),
    }
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
