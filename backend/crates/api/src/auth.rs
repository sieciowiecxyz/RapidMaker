//! Email-code authentication and shared client/operator sessions.
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use rapidmaker_auth::passkeys::{Url, Webauthn, WebauthnBuilder};
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
use utoipa_axum::router::OpenApiRouter as Router;

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
pub(super) fn session_name() -> &'static str {
    if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
        "__Host-rapidmaker_session"
    } else {
        "rapidmaker_session"
    }
}
fn attempt_name() -> &'static str {
    if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
        "__Host-rapidmaker_auth_attempt"
    } else {
        "rapidmaker_auth_attempt"
    }
}

pub(crate) struct Auth {
    pub storage: Option<Storage>,
    key: Option<AuthKey>,
    pub(super) password_work: Arc<tokio::sync::Semaphore>,
    pub(super) dummy_password: Option<String>,
    origin: String,
    mailer: Arc<Mailer>,
    production: bool,
    authority: String,
    pub webauthn: Option<Webauthn>,
    trusted_proxy: Option<std::net::IpAddr>,
}
impl Auth {
    pub fn new(
        storage: Option<Storage>,
        mailer: Arc<Mailer>,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let mode = std::env::var("AUTH_MODE").unwrap_or_else(|_| "disabled".into());
        let production = mode == "production";
        let raw_origin = std::env::var("AUTH_ORIGIN")
            .or_else(|_| {
                if production {
                    Err(())
                } else {
                    Ok("http://localhost:18430".into())
                }
            })
            .map_err(|_| "AUTH_ORIGIN required")?;
        let url = Url::parse(&raw_origin)?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err("AUTH_ORIGIN must be an origin without path or credentials".into());
        }
        let valid = match mode.as_str() {
            "disabled" | "localhost" => url.scheme() == "http" && url.domain() == Some("localhost"),
            "production" => {
                url.scheme() == "https"
                    && url
                        .domain()
                        .is_some_and(|d| d != "localhost" && d.contains('.'))
            }
            _ => false,
        };
        if !valid {
            return Err(
                "Invalid auth mode or origin; localhost passkeys require localhost hostname".into(),
            );
        }
        let origin = url.origin().ascii_serialization();
        let authority = origin
            .split_once("://")
            .ok_or("invalid origin")?
            .1
            .to_owned();
        let key = if mode == "disabled" {
            None
        } else {
            if storage.is_none() {
                return Err("Auth requires PostgreSQL".into());
            }
            Some(AuthKey::from_hex(&std::env::var("AUTH_CODE_KEY")?)?)
        };
        let webauthn = if key.is_some() {
            Some(
                WebauthnBuilder::new(url.domain().ok_or("invalid RP domain")?, &url)?
                    .rp_name("Rapidmaker")
                    .allow_subdomains(false)
                    .build()?,
            )
        } else {
            None
        };
        let trusted_proxy = std::env::var("AUTH_TRUSTED_PROXY")
            .ok()
            .map(|v| v.parse())
            .transpose()?;
        let dummy_password = if key.is_some() {
            Some(
                rapidmaker_auth::credentials::hash_password(
                    &random_token().map_err(|_| "password entropy unavailable")?,
                )
                .map_err(|_| "password hashing unavailable")?,
            )
        } else {
            None
        };
        Ok(Arc::new(Self {
            storage,
            key,
            password_work: Arc::new(tokio::sync::Semaphore::new(2)),
            dummy_password,
            origin,
            mailer,
            production,
            authority,
            webauthn,
            trusted_proxy,
        }))
    }
    pub fn enabled(&self) -> bool {
        self.key.is_some()
    }
    pub fn production(&self) -> bool {
        self.production
    }
    pub fn allows_request(&self, headers: &HeaderMap, mutate: bool) -> bool {
        let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
        let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
        if headers.get_all(header::HOST).iter().count() != 1
            || headers.get_all(header::ORIGIN).iter().count() > 1
            || (headers.contains_key(header::ORIGIN) && origin.is_none())
        {
            return false;
        }
        if self.production {
            return host == Some(self.authority.as_str())
                && if mutate {
                    origin == Some(self.origin.as_str())
                } else {
                    origin.is_none_or(|v| v == self.origin)
                };
        }
        let local = |s: &str| super::checkout::local_authority(s) || s == self.authority;
        host.is_some_and(local)
            && origin.is_none_or(|v| v.strip_prefix("http://").is_some_and(local))
    }
    pub(super) fn client_ip(
        &self,
        peer: SocketAddr,
        headers: &HeaderMap,
    ) -> Result<std::net::IpAddr, Error> {
        if self.trusted_proxy == Some(peer.ip()) {
            headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Nieprawidłowy adres proxy."))
        } else {
            Ok(peer.ip())
        }
    }
    pub async fn administrator(
        &self,
        headers: &HeaderMap,
        mutate: bool,
    ) -> Result<AuthSession, Error> {
        let session = if mutate {
            self.mutation(headers).await?
        } else {
            self.current(headers).await?
        };
        if session.user.role != "admin" || session.admin_verified_at.is_none() {
            return Err(error(
                StatusCode::FORBIDDEN,
                "Wymagane konto administratora i pełne logowanie dwuskładnikowe.",
            ));
        }
        Ok(session)
    }
    pub fn database(&self) -> Result<&Storage, Error> {
        self.storage.as_ref().ok_or_else(unavailable)
    }
    pub(super) fn key(&self) -> Result<&AuthKey, Error> {
        self.key.as_ref().ok_or_else(unavailable)
    }
    pub fn csrf(&self, token: &str) -> Result<String, Error> {
        Ok(self.key()?.csrf(token))
    }
    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), Error> {
        if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(self.origin.as_str()) {
            return Err(error(StatusCode::FORBIDDEN, "Niedozwolone źródło żądania."));
        }
        Ok(())
    }
    pub async fn current(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.key()?;
        let token = session_cookie(headers)?
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Zaloguj się."))?;
        let session = self
            .database()?
            .auth_session(token, now())
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Sesja wygasła."))?;
        Ok(session)
    }
    pub(super) async fn pending(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.key()?;
        let token = session_cookie(headers)?
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Zaloguj się."))?;
        self.database()?
            .auth_pending(token, now())
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Sesja wygasła."))
    }
    pub(super) async fn pending_mutation(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.check_origin(headers)?;
        let session = self.pending(headers).await?;
        let token = session_cookie(headers)?.ok_or_else(unavailable)?;
        if !self.key()?.verify_csrf(
            token,
            headers
                .get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default(),
        ) {
            return Err(error(
                StatusCode::FORBIDDEN,
                "Nieprawidłowy token formularza.",
            ));
        }
        Ok(session)
    }
    pub(super) async fn send_factor(
        &self,
        token: &str,
        session: &AuthSession,
    ) -> Result<(), Error> {
        if session.second_factor == "totp" {
            return Ok(());
        }
        let code = random_code().map_err(|_| unavailable())?;
        if !self
            .database()?
            .auth_factor_start(self.key()?, token, &code, now())
            .await
            .map_err(|_| unavailable())?
        {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "Odczekaj minutę przed ponowną wysyłką kodu.",
            ));
        }
        self.mailer
            .send(&session.user.email, Notification::LoginCode(&code))
            .await
            .map_err(|_| unavailable())
    }
    pub(super) fn session_body(&self, token: &str, s: &AuthSession) -> Result<Value, Error> {
        Ok(
            json!({"user":s.user,"stage":s.stage,"second_factor":s.second_factor,"expires_at":s.expires_at,"csrf_token":self.csrf(token)?,"admin_verified":s.stage=="authenticated" && s.user.role=="admin"}),
        )
    }
    pub(super) fn finish_session(&self, token: &str, s: &AuthSession) -> Result<Response, Error> {
        response(
            self.session_body(token, s)?,
            &[set_cookie(
                session_name(),
                token,
                (s.expires_at - now()).clamp(0, 30 * 86400) as u32,
            )],
        )
    }
    pub async fn mutation(&self, headers: &HeaderMap) -> Result<AuthSession, Error> {
        self.check_origin(headers)?;
        let session = self.current(headers).await?;
        let token = session_cookie(headers)?.ok_or_else(unavailable)?;
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
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        for part in header.to_str().ok()?.split(';') {
            let Some((key, value)) = part.trim().split_once('=') else {
                continue;
            };
            if key == name {
                if found.is_some() || !valid_token(value) {
                    return None;
                }
                found = Some(value);
            }
        }
    }
    found
}

pub(crate) fn session_cookie(headers: &HeaderMap) -> Result<Option<&str>, Error> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        for part in header
            .to_str()
            .map_err(|_| error(StatusCode::UNAUTHORIZED, "Nieprawidłowa sesja."))?
            .split(';')
        {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            if matches!(name, "rapidmaker_session" | "__Host-rapidmaker_session") {
                if name != session_name() || found.is_some() || !valid_token(value) {
                    return Err(error(StatusCode::UNAUTHORIZED, "Nieprawidłowa sesja."));
                }
                found = Some(value);
            }
        }
    }
    Ok(found)
}
pub(super) fn response(body: Value, cookies: &[String]) -> Result<Response, Error> {
    let mut r = Json(body).into_response();
    for c in cookies {
        let v = c.parse().map_err(|_| unavailable())?;
        r.headers_mut().append(header::SET_COOKIE, v);
    }
    Ok(r)
}
pub(super) fn set_cookie(name: &str, value: &str, age: u32) -> String {
    format!(
        "{name}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={age}{}",
        if std::env::var("AUTH_MODE").as_deref() == Ok("production") {
            "; Secure"
        } else {
            ""
        }
    )
}

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .routes(utoipa_axum::routes!(start))
        .routes(utoipa_axum::routes!(verify))
        .routes(utoipa_axum::routes!(me))
        .routes(utoipa_axum::routes!(logout))
        .routes(utoipa_axum::routes!(logout_all))
        .routes(utoipa_axum::routes!(preferences, save_preferences))
        .with_state(auth)
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = AuthStart)]
#[serde(deny_unknown_fields)]
struct Start {
    email: String,
}
#[utoipa::path(
    operation_id = "auth_start",
    post,
    path = "/api/auth/start",
    tag = "Auth",
    summary = "Wyślij kod logowania na email.",
    description = "Wymaga dozwolonego Origin; ustawia HttpOnly cookie próby logowania. Kod i próba są powiązane z przeglądarką.",
    request_body = Start,
    responses((status = 202, description = "Przyjęto; dalszy krok wymaga kodu z emaila.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value))
)]
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
    if !auth
        .database()?
        .auth_enrollment_allowed(&email)
        .await
        .map_err(|_| unavailable())?
    {
        return Err(error(
            StatusCode::FORBIDDEN,
            "Użyj hasła lub passkey, a następnie drugiego składnika.",
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
            &hash_token(&auth.client_ip(peer, &headers)?.to_string()),
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
        tracing::error!(event="auth_mail_failed", reason=%reason);
        db.auth_invalidate(&id).await.map_err(|_| unavailable())?;
        return Err(unavailable());
    }
    let mut r = response(
        json!({"challenge_id":id}),
        &[set_cookie(attempt_name(), &browser, 600)],
    )?;
    *r.status_mut() = StatusCode::ACCEPTED;
    tracing::info!(event = "auth_code_sent");
    Ok(r)
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = AuthVerify)]
#[serde(deny_unknown_fields)]
struct Verify {
    challenge_id: String,
    code: String,
}
#[utoipa::path(
    operation_id = "auth_verify",
    post,
    path = "/api/auth/verify",
    tag = "Auth",
    summary = "Zweryfikuj kod i utwórz sesję.",
    description = "Rejestracja/migracja kont bez hasła i passkey. Wymaga cookie próby i tego samego Origin; zwraca ograniczoną sesję setup do ustawienia pierwszej metody.",
    request_body = Verify,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value))
)]
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
    let browser = cookie(&headers, attempt_name()).ok_or_else(|| {
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
            &hash_token(&auth.client_ip(peer, &headers)?.to_string()),
            session_cookie(&headers)?,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            tracing::warn!(
                event = "auth_verify_rejected",
                reason = "invalid_or_expired_code"
            );
            error(
                StatusCode::UNAUTHORIZED,
                "Kod jest nieprawidłowy lub wygasł.",
            )
        })?;
    tracing::info!(event="auth_verified", role=%session.user.role);
    response(
        auth.session_body(&token, &session)?,
        &[
            set_cookie(
                session_name(),
                &token,
                (session.expires_at - now()).clamp(0, 30 * 86400) as u32,
            ),
            set_cookie(attempt_name(), "", 0),
        ],
    )
}
#[utoipa::path(
    operation_id = "auth_me",
    get,
    path = "/api/auth/me",
    tag = "Auth",
    summary = "Bieżąca sesja, użytkownik i token CSRF.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn me(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let s = auth.pending(&headers).await?;
    Ok(Json(auth.session_body(
        session_cookie(&headers)?.ok_or_else(unavailable)?,
        &s,
    )?))
}
#[utoipa::path(
    operation_id = "auth_logout",
    post,
    path = "/api/auth/logout",
    tag = "Auth",
    summary = "Zakończ bieżącą sesję.",
    request_body = Value,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn logout(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_input): Json<Value>,
) -> Result<Response, Error> {
    auth.pending_mutation(&headers).await?;
    auth.database()?
        .auth_logout(session_cookie(&headers)?.ok_or_else(unavailable)?)
        .await
        .map_err(|_| unavailable())?;
    tracing::info!(event = "auth_logout", scope = "session");
    response(json!({"ok":true}), &[set_cookie(session_name(), "", 0)])
}
#[utoipa::path(
    operation_id = "auth_logout_all",
    post,
    path = "/api/auth/logout-all",
    tag = "Auth",
    summary = "Zakończ wszystkie sesje użytkownika.",
    request_body = Value,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn logout_all(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_input): Json<Value>,
) -> Result<Response, Error> {
    let s = auth.mutation(&headers).await?;
    auth.database()?
        .auth_logout_all(&s.user.id)
        .await
        .map_err(|_| unavailable())?;
    tracing::info!(event = "auth_logout", scope = "all_sessions");
    response(json!({"ok":true}), &[set_cookie(session_name(), "", 0)])
}
#[utoipa::path(
    operation_id = "auth_preferences",
    get,
    path = "/api/account/notifications",
    tag = "Konto",
    summary = "Pobierz preferencje powiadomień email.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn preferences(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let s = auth.current(&headers).await?;
    Ok(Json(
        json!({"email_order_updates":s.user.email_order_updates,"email_chat_messages":s.user.email_chat_messages}),
    ))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = NotificationPreferences)]
#[serde(deny_unknown_fields)]
struct Preferences {
    email_order_updates: bool,
    email_chat_messages: bool,
}
#[utoipa::path(
    operation_id = "auth_save_preferences",
    put,
    path = "/api/account/notifications",
    tag = "Konto",
    summary = "Zapisz preferencje powiadomień email.",
    request_body = Preferences,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_attempt_cookies_reject_ambiguous_or_malformed_values() {
        let token = "ab".repeat(32);
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, format!("attempt={token}").parse().unwrap());
        assert_eq!(cookie(&headers, "attempt"), Some(token.as_str()));
        headers.append(header::COOKIE, format!("attempt={token}").parse().unwrap());
        assert!(cookie(&headers, "attempt").is_none());
        for value in [
            format!("attempt={token}; attempt={token}"),
            format!("attempt=invalid; attempt={token}"),
            format!("attempt={token}; attempt=invalid"),
        ] {
            headers.insert(header::COOKIE, value.parse().unwrap());
            assert!(cookie(&headers, "attempt").is_none());
        }
    }
    #[test]
    fn session_cookies_are_opaque_mode_specific_and_unambiguous() {
        let token = "ab".repeat(32);
        let mut headers = HeaderMap::new();
        assert!(session_cookie(&headers).unwrap().is_none());
        headers.insert(
            header::COOKIE,
            format!("{}={token}", session_name()).parse().unwrap(),
        );
        assert_eq!(session_cookie(&headers).unwrap(), Some(token.as_str()));
        headers.append(
            header::COOKIE,
            format!("{}={token}", session_name()).parse().unwrap(),
        );
        assert!(session_cookie(&headers).is_err());
        headers.insert(
            header::COOKIE,
            format!("{}=bad", session_name()).parse().unwrap(),
        );
        assert!(session_cookie(&headers).is_err());
        let other = if session_name().starts_with("__Host") {
            "rapidmaker_session"
        } else {
            "__Host-rapidmaker_session"
        };
        headers.insert(header::COOKIE, format!("{other}={token}").parse().unwrap());
        assert!(session_cookie(&headers).is_err());
    }
}
