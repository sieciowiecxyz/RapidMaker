//! Password/TOTP HTTP boundaries; PostgreSQL owns transitions, limits and replay protection.
use super::auth::{Auth, Error, error, now, session_cookie, unavailable};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use rapidmaker_auth::{credentials::*, hash_token, random_token};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::Arc};
use utoipa_axum::{router::OpenApiRouter as Router, routes};

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .routes(routes!(password_login))
        .routes(routes!(factor_verify))
        .routes(routes!(factor_resend))
        .routes(routes!(security))
        .routes(routes!(password_set))
        .routes(routes!(totp_start))
        .routes(routes!(totp_confirm))
        .routes(routes!(totp_disable))
        .with_state(auth)
}
pub(super) fn denied() -> Error {
    error(
        StatusCode::FORBIDDEN,
        "Potwierdź hasło lub passkey i drugi składnik. Zmiana zabezpieczeń wymaga pełnego logowania z ostatnich 5 minut.",
    )
}
pub(super) fn token(headers: &HeaderMap) -> Result<&str, Error> {
    session_cookie(headers)?.ok_or_else(denied)
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PasswordLogin {
    email: String,
    password: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Password {
    password: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Code {
    code: String,
}
#[utoipa::path(post,path="/api/auth/password/login",tag="Auth",request_body=PasswordLogin,responses((status=200,body=Value),(status=401,body=Value),(status=429,body=Value)))]
async fn password_login(
    State(auth): State<Arc<Auth>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<PasswordLogin>,
) -> Result<Response, Error> {
    auth.check_origin(&headers)?;
    auth.key()?;
    let email = input.email.trim().to_lowercase();
    if !email.is_ascii()
        || !rapidmaker_notifications::valid_address(&email)
        || validate_password(&input.password).is_err()
    {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Nieprawidłowe dane logowania.",
        ));
    }
    let record = auth
        .database()?
        .auth_login_record(
            &email,
            &hash_token(&auth.client_ip(peer, &headers)?.to_string()),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| error(StatusCode::TOO_MANY_REQUESTS, "Spróbuj ponownie później."))?;
    let encoded = record
        .clone()
        .or_else(|| auth.dummy_password.clone())
        .ok_or_else(unavailable)?;
    let permit = auth
        .password_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "Spróbuj ponownie za chwilę."))?;
    let valid = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        verify_password(&input.password, &encoded)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    if !valid || record.is_none() {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Nieprawidłowe dane logowania.",
        ));
    }
    let token = random_token().map_err(|_| unavailable())?;
    let s = auth
        .database()?
        .auth_primary(
            &email,
            record.as_deref(),
            &token,
            session_cookie(&headers)?,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    if let Err(e) = auth.send_factor(&token, &s).await {
        auth.database()?
            .auth_logout(&token)
            .await
            .map_err(|_| unavailable())?;
        return Err(e);
    }
    auth.finish_session(&token, &s)
}
#[utoipa::path(post,path="/api/auth/factor/verify",tag="Auth",request_body=Code,responses((status=200,body=Value),(status=401,body=Value)),security(("session"=[],"csrf"=[])))]
async fn factor_verify(
    State(auth): State<Arc<Auth>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Code>,
) -> Result<Response, Error> {
    auth.pending_mutation(&headers).await?;
    if input.code.len() > 64 {
        return Err(denied());
    }
    let new = random_token().map_err(|_| unavailable())?;
    let s = auth
        .database()?
        .auth_factor_verify(
            auth.key()?,
            token(&headers)?,
            &input.code,
            &new,
            &hash_token(&auth.client_ip(peer, &headers)?.to_string()),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::UNAUTHORIZED,
                "Kod jest nieprawidłowy, zużyty lub wygasł.",
            )
        })?;
    auth.finish_session(&new, &s)
}
#[utoipa::path(post,path="/api/auth/factor/resend",tag="Auth",responses((status=200,body=Value),(status=403,body=Value),(status=429,body=Value)),security(("session"=[],"csrf"=[])))]
async fn factor_resend(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let s = auth.pending_mutation(&headers).await?;
    if s.stage != "second_factor" || s.second_factor != "email" {
        return Err(denied());
    }
    auth.send_factor(token(&headers)?, &s).await?;
    Ok(Json(json!({"ok":true})))
}
#[utoipa::path(get,path="/api/account/security",tag="Konto",responses((status=200,body=Value),(status=403,body=Value)),security(("session"=[])))]
async fn security(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    auth.pending(&headers).await?;
    Ok(Json(
        auth.database()?
            .auth_security(token(&headers)?, now())
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(denied)?,
    ))
}
#[utoipa::path(put,path="/api/account/password",tag="Konto",request_body=Password,responses((status=200,body=Value),(status=403,body=Value)),security(("session"=[],"csrf"=[])))]
async fn password_set(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Password>,
) -> Result<Response, Error> {
    let s = auth.pending_mutation(&headers).await?;
    if s.stage != "setup"
        && (s.stage != "authenticated" || s.factor_verified_at.is_none_or(|t| t <= now() - 300))
    {
        return Err(denied());
    }
    validate_password(&input.password).map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "Hasło musi mieć 12–128 znaków i maksymalnie 512 bajtów.",
        )
    })?;
    if !auth
        .database()?
        .auth_password_admit(token(&headers)?, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Odczekaj przed kolejną zmianą hasła lub zaloguj się ponownie.",
        ));
    }
    let permit = auth
        .password_work
        .clone()
        .try_acquire_owned()
        .map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "Spróbuj ponownie za chwilę."))?;
    let hash = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_password(&input.password)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let new = random_token().map_err(|_| unavailable())?;
    let s = auth
        .database()?
        .auth_password_set(token(&headers)?, &hash, &new, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    auth.finish_session(&new, &s)
}
#[utoipa::path(post,path="/api/account/totp/start",tag="Konto",responses((status=200,body=Value),(status=403,body=Value)),security(("session"=[],"csrf"=[])))]
async fn totp_start(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let s = auth.mutation(&headers).await?;
    let secret = new_totp_secret().map_err(|_| unavailable())?;
    let sealed = seal_totp(auth.key()?, &s.user.id, &secret).map_err(|_| unavailable())?;
    if !auth
        .database()?
        .auth_totp_start(token(&headers)?, &sealed, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(denied());
    }
    let uri = totp_uri(&secret, &s.user.email).map_err(|_| unavailable())?;
    let encoded = rapidmaker_auth::passkeys::Url::parse(&uri)
        .map_err(|_| unavailable())?
        .query_pairs()
        .find(|(k, _)| k == "secret")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(unavailable)?;
    Ok(Json(json!({"secret":encoded,"otpauth_uri":uri})))
}
#[utoipa::path(post,path="/api/account/totp/confirm",tag="Konto",request_body=Code,responses((status=200,body=Value),(status=403,body=Value)),security(("session"=[],"csrf"=[])))]
async fn totp_confirm(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Code>,
) -> Result<Response, Error> {
    auth.mutation(&headers).await?;
    let codes = (0..8)
        .map(|_| random_token().map(|v| v[..32].to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unavailable())?;
    let new = random_token().map_err(|_| unavailable())?;
    let s = auth
        .database()?
        .auth_totp_confirm(
            auth.key()?,
            token(&headers)?,
            &input.code,
            &new,
            &codes,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    let mut body = auth.session_body(&new, &s)?;
    body["recovery_codes"] = json!(codes);
    super::auth::response(
        body,
        &[super::auth::set_cookie(
            super::auth::session_name(),
            &new,
            (s.expires_at - now()).clamp(0, 30 * 86400) as u32,
        )],
    )
}
#[utoipa::path(delete,path="/api/account/totp",tag="Konto",responses((status=200,body=Value),(status=403,body=Value)),security(("session"=[],"csrf"=[])))]
async fn totp_disable(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    auth.mutation(&headers).await?;
    let new = random_token().map_err(|_| unavailable())?;
    let s = auth
        .database()?
        .auth_totp_disable(token(&headers)?, &new, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    auth.finish_session(&new, &s)
}
