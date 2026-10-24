//! HTTP only: credential verification and atomic state changes live in auth/storage.
use super::auth::{
    Auth, Error, error, now, response, session_cookie, session_name, set_cookie, unavailable,
};
use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_auth::{
    passkeys::{PublicKeyCredential, RegisterPublicKeyCredential},
    random_token,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::Arc};
use utoipa_axum::router::OpenApiRouter as Router;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .routes(utoipa_axum::routes!(register_start))
        .routes(utoipa_axum::routes!(register_finish))
        .routes(utoipa_axum::routes!(login_start))
        .routes(utoipa_axum::routes!(login_finish))
        .routes(utoipa_axum::routes!(list))
        .routes(utoipa_axum::routes!(remove))
        .with_state(auth)
}
fn denied() -> Error {
    error(
        StatusCode::FORBIDDEN,
        "Wymagane świeże potwierdzenie passkey lub lokalne zezwolenie operatora.",
    )
}
fn token(headers: &HeaderMap) -> Result<&str, Error> {
    session_cookie(headers)?.ok_or_else(denied)
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = PasskeyRegistrationStart)]
#[serde(deny_unknown_fields)]
struct Start {
    bootstrap: Option<String>,
}
#[utoipa::path(
    operation_id = "passkeys_register_start",
    post,
    path = "/api/auth/passkey/register/start",
    tag = "Passkey",
    summary = "Rozpocznij rejestrację passkey.",
    description = "Rejestracja wymaga świeżego pełnego logowania lub zweryfikowanej emailowej sesji zakładania pierwszego klucza. Logowanie passkey jest pierwszym składnikiem; drugi składnik pozostaje obowiązkowy. Weryfikacja użytkownika WebAuthn jest obowiązkowa.",
    request_body = Start,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn register_start(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Start>,
) -> Result<Json<Value>, Error> {
    auth.pending_mutation(&headers).await?;
    if input
        .bootstrap
        .as_ref()
        .is_some_and(|v| !rapidmaker_auth::valid_token(v))
    {
        return Err(denied());
    }
    let start = auth
        .database()?
        .passkey_registration_start(
            auth.webauthn.as_ref().ok_or_else(unavailable)?,
            token(&headers)?,
            input.bootstrap.as_deref(),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    Ok(Json(
        json!({"challenge_id":start.id,"options":start.options}),
    ))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = PasskeyRegistration)]
#[serde(deny_unknown_fields)]
struct Register {
    challenge_id: String,
    #[serde(deserialize_with = "registration_credential")]
    #[schema(value_type = Object)]
    credential: RegisterPublicKeyCredential,
    name: String,
}
fn registration_credential<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<RegisterPublicKeyCredential, D::Error> {
    let mut value = Value::deserialize(deserializer)?;
    // Chromium may not know whether a key is resident. Absence is not an assertion of `rk=false`.
    // This extension is unsigned; signed authenticator data and UV remain verified by WebAuthn.
    for name in ["extensions", "clientExtensionResults"] {
        if let Some(extensions) = value.get_mut(name).and_then(Value::as_object_mut)
            && extensions
                .get("credProps")
                .and_then(Value::as_object)
                .is_some_and(|v| v.is_empty())
        {
            extensions.remove("credProps");
        }
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}
#[utoipa::path(
    operation_id = "passkeys_register_finish",
    post,
    path = "/api/auth/passkey/register/finish",
    tag = "Passkey",
    summary = "Zweryfikuj rejestrację passkey i obróć sesję.",
    description = "Rejestracja wymaga świeżego pełnego logowania lub zweryfikowanej emailowej sesji zakładania pierwszego klucza. Logowanie passkey jest pierwszym składnikiem; drugi składnik pozostaje obowiązkowy. Weryfikacja użytkownika WebAuthn jest obowiązkowa.",
    request_body = Register,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn register_finish(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<axum::response::Response, Error> {
    auth.pending_mutation(&headers).await?;
    let new_token = random_token().map_err(|_| unavailable())?;
    let session = auth
        .database()?
        .passkey_registration_finish(
            auth.webauthn.as_ref().ok_or_else(unavailable)?,
            token(&headers)?,
            &input.challenge_id,
            &input.credential,
            &input.name,
            &new_token,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    tracing::info!(event = "passkey_registered");
    finish(&auth, &new_token, session)
}
#[utoipa::path(
    operation_id = "passkeys_login_start",
    post,
    path = "/api/auth/passkey/login/start",
    tag = "Passkey",
    summary = "Rozpocznij logowanie passkey klienta lub administratora.",
    description = "Rejestracja wymaga świeżego pełnego logowania lub zweryfikowanej emailowej sesji zakładania pierwszego klucza. Logowanie passkey jest pierwszym składnikiem; drugi składnik pozostaje obowiązkowy. Weryfikacja użytkownika WebAuthn jest obowiązkowa.",
    request_body = Email,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn login_start(
    State(auth): State<Arc<Auth>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Email>,
) -> Result<axum::response::Response, Error> {
    auth.check_origin(&headers)?;
    auth.key()?;
    let email = input.email.trim().to_lowercase();
    if !email.is_ascii() || !rapidmaker_notifications::valid_address(&email) {
        return Err(denied());
    }
    if auth
        .database()?
        .auth_login_record(
            &email,
            &rapidmaker_auth::hash_token(&auth.client_ip(peer, &headers)?.to_string()),
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .is_none()
    {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Spróbuj ponownie później.",
        ));
    }
    let pending = random_token().map_err(|_| unavailable())?;
    let session = auth
        .database()?
        .auth_primary(&email, None, &pending, session_cookie(&headers)?, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    let start = auth
        .database()?
        .passkey_login_start(
            auth.webauthn.as_ref().ok_or_else(unavailable)?,
            &pending,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    let mut body = auth.session_body(&pending, &session)?;
    body["challenge_id"] = json!(start.id);
    body["options"] = start.options;
    response(body, &[set_cookie(session_name(), &pending, 600)])
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Email {
    email: String,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = PasskeyLogin)]
#[serde(deny_unknown_fields)]
struct Login {
    challenge_id: String,
    #[schema(value_type = Object)]
    credential: PublicKeyCredential,
}
#[utoipa::path(
    operation_id = "passkeys_login_finish",
    post,
    path = "/api/auth/passkey/login/finish",
    tag = "Passkey",
    summary = "Zweryfikuj passkey i rozpocznij obowiązkowy drugi składnik.",
    description = "Rejestracja wymaga świeżego pełnego logowania lub zweryfikowanej emailowej sesji zakładania pierwszego klucza. Logowanie passkey jest pierwszym składnikiem; drugi składnik pozostaje obowiązkowy. Weryfikacja użytkownika WebAuthn jest obowiązkowa.",
    request_body = Login,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn login_finish(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<axum::response::Response, Error> {
    auth.pending_mutation(&headers).await?;
    let new_token = random_token().map_err(|_| unavailable())?;
    let session = auth
        .database()?
        .passkey_login_finish(
            auth.webauthn.as_ref().ok_or_else(unavailable)?,
            token(&headers)?,
            &input.challenge_id,
            &input.credential,
            &new_token,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    if let Err(e) = auth.send_factor(&new_token, &session).await {
        auth.database()?
            .auth_logout(&new_token)
            .await
            .map_err(|_| unavailable())?;
        return Err(e);
    }
    tracing::info!(event = "passkey_verified");
    finish(&auth, &new_token, session)
}
fn finish(
    auth: &Auth,
    token: &str,
    session: rapidmaker_storage::AuthSession,
) -> Result<axum::response::Response, Error> {
    auth.finish_session(token, &session)
}

#[utoipa::path(
    operation_id = "passkeys_list",
    get,
    path = "/api/auth/passkeys",
    tag = "Passkey",
    summary = "Lista własnych passkeys po świeżym pełnym logowaniu.",
    description = "Wymaga własnej sesji i pełnego logowania z ostatnich 5 minut.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn list(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    auth.current(&headers).await?;
    let keys = auth
        .database()?
        .passkeys_list(token(&headers)?, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(denied)?;
    Ok(Json(json!({"passkeys":keys})))
}
#[utoipa::path(
    operation_id = "passkeys_remove",
    delete,
    path = "/api/auth/passkeys/{id}",
    tag = "Passkey",
    summary = "Usuń passkey; ostatni klucz jest chroniony.",
    description = "Wymaga własnej sesji i pełnego logowania z ostatnich 5 minut.",
    params(("id" = String, Path, description = "Identyfikator passkey.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn remove(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    auth.mutation(&headers).await?;
    if !auth
        .database()?
        .passkey_remove(token(&headers)?, &id, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(denied());
    }
    Ok(Json(json!({"ok":true})))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chromium_empty_optional_credprops_is_accepted() {
        let input = json!({"challenge_id":"test","name":"test","credential":{"id":"test","rawId":"AA","type":"public-key","response":{"attestationObject":"AA","clientDataJSON":"AA"},"clientExtensionResults":{"credProps":{}}}});
        assert!(serde_json::from_value::<Register>(input).is_ok());
    }
}
