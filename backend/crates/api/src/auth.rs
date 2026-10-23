//! Client email-code authentication. Local operator authorization remains separate.
use axum::{
    Json, Router,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rapidmaker_auth::{AuthKey, hash_token, random_code, random_token, valid_token};
use rapidmaker_notifications::{Mailer, Notification};
use rapidmaker_storage::{AuthSession, Storage};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) type Error = (StatusCode, Json<Value>);
pub(crate) fn error(status: StatusCode, message: &str) -> Error {
    (status, Json(json!({"message":message})))
}
pub(crate) fn unavailable() -> Error {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Usługa jest chwilowo niedostępna.",
    )
}
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
const SESSION: &str = "rapidmaker_session";
const ATTEMPT: &str = "rapidmaker_auth_attempt";

pub(crate) struct Auth {
    pub storage: Option<Storage>,
    key: Option<AuthKey>,
    origin: String,
    mailer: Arc<Mailer>,
}
impl Auth {
    pub fn new(
        storage: Option<Storage>,
        mailer: Arc<Mailer>,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mode = std::env::var("AUTH_MODE").unwrap_or_else(|_| "disabled".into());
        let origin =
            std::env::var("AUTH_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:18430".into());
        let key = match mode.as_str() {
            "disabled" => None,
            "localhost"
                if matches!(
                    origin.as_str(),
                    "http://127.0.0.1:18430" | "http://localhost:18430"
                ) && storage.is_some() =>
            {
                Some(AuthKey::from_hex(&std::env::var("AUTH_CODE_KEY")?)?)
            }
            _ => return Err("Unsupported auth mode or missing local database/origin".into()),
        };
        Ok(Arc::new(Self {
            storage,
            key,
            origin,
            mailer,
        }))
    }
    pub fn database(&self) -> Result<&Storage, Error> {
        self.storage.as_ref().ok_or_else(unavailable)
    }
    fn key(&self) -> Result<&AuthKey, Error> {
        self.key.as_ref().ok_or_else(unavailable)
    }
    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), Error> {
        if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(self.origin.as_str()) {
            return Err(error(StatusCode::FORBIDDEN, "Niedozwolone źródło żądania."));
        }
        Ok(())
    }
    pub async fn current(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.key()?;
        let token = cookie(headers, SESSION)
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Zaloguj się."))?;
        let session = self
            .database()?
            .auth_session(token, now())
            .await
            .map_err(|_| unavailable())?
            .filter(|s| s.user.role == "client")
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Sesja wygasła."))?;
        Ok(session)
    }
    pub async fn mutation(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.check_origin(headers)?;
        let session = self.current(headers).await?;
        let token = cookie(headers, SESSION).ok_or_else(unavailable)?;
        let csrf = headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if !self.key()?.verify_csrf(token, csrf) {
            return Err(error(
                StatusCode::FORBIDDEN,
                "Nieprawidłowy token formularza.",
            ));
        }
        Ok(session)
    }
}
pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (k, v) = part.trim().split_once('=')?;
            (k == name && valid_token(v)).then_some(v)
        })
}
fn response(body: Value, cookies: &[String]) -> Result<Response, Error> {
    let mut r = Json(body).into_response();
    for c in cookies {
        let v = c.parse().map_err(|_| unavailable())?;
        r.headers_mut().append(header::SET_COOKIE, v);
    }
    Ok(r)
}
fn set_cookie(name: &str, value: &str, age: u32) -> String {
    format!("{name}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={age}")
}

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/api/auth/start", post(start))
        .route("/api/auth/verify", post(verify))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/logout-all", post(logout_all))
        .route(
            "/api/account/notifications",
            get(preferences).put(save_preferences),
        )
        .with_state(auth)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    email: String,
}
async fn start(
    State(auth): State<Arc<Auth>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Start>,
) -> Result<Response, Error> {
    auth.check_origin(&headers)?;
    let key = auth.key()?;
    let email = input.email.trim().to_lowercase();
    if !email.is_ascii() || !rapidmaker_notifications::valid_address(&email) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Podaj poprawny adres email.",
        ));
    }
    let browser = random_token().map_err(|_| unavailable())?;
    let id = uuid::Uuid::new_v4().to_string();
    let code = random_code().map_err(|_| unavailable())?;
    let browser_hash = hash_token(&browser);
    let verifier = key.verifier(&id, &email, &browser_hash, &code);
    let db = auth.database()?;
    if !db
        .auth_start(
            &id,
            &email,
            &browser_hash,
            &verifier,
            &hash_token(&peer.ip().to_string()),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
    {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Spróbuj ponownie później.",
        ));
    }
    if let Err(reason) = auth
        .mailer
        .send(&email, Notification::LoginCode(&code))
        .await
    {
        eprintln!("event=auth_mail_failed reason={reason}");
        db.auth_invalidate(&id).await.map_err(|_| unavailable())?;
        return Err(unavailable());
    }
    let mut r = response(
        json!({"challenge_id":id}),
        &[set_cookie(ATTEMPT, &browser, 600)],
    )?;
    *r.status_mut() = StatusCode::ACCEPTED;
    Ok(r)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Verify {
    challenge_id: String,
    code: String,
}
async fn verify(
    State(auth): State<Arc<Auth>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Verify>,
) -> Result<Response, Error> {
    auth.check_origin(&headers)?;
    if uuid::Uuid::parse_str(&input.challenge_id).is_err()
        || input.code.len() != 8
        || !input.code.bytes().all(|v| v.is_ascii_digit())
    {
        return Err(error(StatusCode::BAD_REQUEST, "Nieprawidłowy kod."));
    }
    let browser = cookie(&headers, ATTEMPT).ok_or_else(|| {
        error(
            StatusCode::UNAUTHORIZED,
            "Rozpocznij logowanie w tej przeglądarce.",
        )
    })?;
    let token = random_token().map_err(|_| unavailable())?;
    let session = auth
        .database()?
        .auth_verify(
            auth.key()?,
            &input.challenge_id,
            browser,
            &input.code,
            &token,
            &hash_token(&peer.ip().to_string()),
            cookie(&headers, SESSION),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::UNAUTHORIZED,
                "Kod jest nieprawidłowy lub wygasł.",
            )
        })?;
    response(
        json!({"user":session.user,"csrf_token":auth.key()?.csrf(&token),"admin_verified":false}),
        &[
            set_cookie(SESSION, &token, 30 * 86400),
            set_cookie(ATTEMPT, "", 0),
        ],
    )
}
async fn me(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let s = auth.current(&headers).await?;
    Ok(Json(
        json!({"user":s.user,"expires_at":s.expires_at,"csrf_token":auth.key()?.csrf(cookie(&headers,SESSION).ok_or_else(unavailable)?),"admin_verified":false}),
    ))
}
async fn logout(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_): Json<Value>,
) -> Result<Response, Error> {
    auth.mutation(&headers).await?;
    auth.database()?
        .auth_logout(cookie(&headers, SESSION).ok_or_else(unavailable)?)
        .await
        .map_err(|_| unavailable())?;
    response(json!({"ok":true}), &[set_cookie(SESSION, "", 0)])
}
async fn logout_all(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_): Json<Value>,
) -> Result<Response, Error> {
    let s = auth.mutation(&headers).await?;
    auth.database()?
        .auth_logout_all(&s.user.id)
        .await
        .map_err(|_| unavailable())?;
    response(json!({"ok":true}), &[set_cookie(SESSION, "", 0)])
}
async fn preferences(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let s = auth.current(&headers).await?;
    Ok(Json(
        json!({"email_order_updates":s.user.email_order_updates,"email_chat_messages":s.user.email_chat_messages}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferences {
    email_order_updates: bool,
    email_chat_messages: bool,
}
async fn save_preferences(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(p): Json<Preferences>,
) -> Result<Json<Value>, Error> {
    let s = auth.mutation(&headers).await?;
    let user = auth
        .database()?
        .auth_preferences(&s.user.id, p.email_order_updates, p.email_chat_messages)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    Ok(Json(
        json!({"email_order_updates":user.email_order_updates,"email_chat_messages":user.email_chat_messages}),
    ))
}
