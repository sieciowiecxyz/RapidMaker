//! Local operator API. Browser mock roles never authorize database writes.
use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use rapidmaker_pricing::Rates;
use rapidmaker_storage::{Color, MaterialInput, Storage, validate_color};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;

struct App {
    storage: Option<Storage>,
    token_hash: Option<[u8; 32]>,
    fallback_rates: Option<Rates>,
    sessions: Mutex<HashMap<String, Instant>>,
    attempts: Mutex<(Instant, u32)>,
}
const SESSION_TTL: Duration = Duration::from_secs(6 * 3600);
const COOKIE: &str = "rapidmaker_operator";
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
    let orders = super::orders::admin_router(database.clone());
    let app = Arc::new(App {
        storage: database,
        token_hash,
        fallback_rates,
        sessions: Mutex::new(HashMap::new()),
        attempts: Mutex::new((Instant::now(), 0)),
    });
    Ok(Router::new()
        .merge(orders)
        .route("/api/admin/materials", get(materials))
        .route("/api/admin/materials/{id}", put(save_material))
        .route("/api/admin/materials/{id}/colors/{code}", put(save_color))
        .route("/api/admin/pricing", get(pricing).put(save_pricing))
        .route_layer(middleware::from_fn_with_state(app.clone(), authorize))
        .route(
            "/api/admin/session",
            get(session).post(login).delete(logout),
        )
        .with_state(app))
}

fn valid_secret(app: &App, supplied: &str) -> bool {
    app.token_hash.as_ref().is_some_and(|expected| {
        let digest: [u8; 32] = Sha256::digest(supplied.as_bytes()).into();
        supplied.len() <= 256 && bool::from(digest.ct_eq(expected))
    })
}
fn cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == COOKIE && value.len() == 32 && value.bytes().all(|v| v.is_ascii_hexdigit()))
                .then_some(value)
        })
}
fn authenticated(app: &App, headers: &HeaderMap) -> Result<bool, Error> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if bearer.is_some_and(|token| valid_secret(app, token)) {
        return Ok(true);
    }
    let mut sessions = app.sessions.lock().map_err(|_| unavailable())?;
    sessions.retain(|_, created| created.elapsed() < SESSION_TTL);
    Ok(cookie(headers).is_some_and(|id| sessions.contains_key(id)))
}
async fn session(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let logged_in = authenticated(&app, &headers)?;
    Ok(Json(
        json!({"configured":app.token_hash.is_some(), "authenticated":logged_in,
        "session": logged_in.then(|| json!({"id":"operator","email":"operator@rapidmaker.pl","name":"Operator","role":"admin"}))}),
    ))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
    token: String,
}
async fn login(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(input): Json<Credentials>,
) -> Result<Response, Error> {
    if app.token_hash.is_none() {
        return Err(unavailable());
    }
    {
        let mut attempts = app.attempts.lock().map_err(|_| unavailable())?;
        if attempts.0.elapsed() >= Duration::from_secs(60) {
            *attempts = (Instant::now(), 0);
        }
        if attempts.1 >= 10 {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "Za dużo prób logowania. Spróbuj za minutę.",
            ));
        }
        attempts.1 += 1;
    }
    if !valid_secret(&app, &input.token) {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Nieprawidłowy klucz administratora.",
        ));
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    let mut sessions = app.sessions.lock().map_err(|_| unavailable())?;
    sessions.retain(|_, created| created.elapsed() < SESSION_TTL);
    if let Some(previous) = cookie(&headers) {
        sessions.remove(previous);
    }
    if sessions.len() >= 64 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Za dużo aktywnych sesji administratora.",
        ));
    }
    sessions.insert(id.clone(), Instant::now());
    let mut response = Json(json!({"authenticated":true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={id}; HttpOnly; SameSite=Strict; Path=/api/admin; Max-Age={}",
            SESSION_TTL.as_secs()
        ))
        .map_err(|_| unavailable())?,
    );
    Ok(response)
}
async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Response, Error> {
    if let Some(id) = cookie(&headers) {
        app.sessions.lock().map_err(|_| unavailable())?.remove(id);
    }
    let mut response = Json(json!({"authenticated":false})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "rapidmaker_operator=; HttpOnly; SameSite=Strict; Path=/api/admin; Max-Age=0",
        ),
    );
    Ok(response)
}

async fn authorize(State(app): State<Arc<App>>, request: Request, next: Next) -> Response {
    if app.token_hash.is_none() {
        return unavailable().into_response();
    }
    match authenticated(&app, request.headers()) {
        Ok(true) => {}
        Ok(false) => {
            return error(StatusCode::UNAUTHORIZED, "Zaloguj się jako administrator.")
                .into_response();
        }
        Err(error) => return error.into_response(),
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
