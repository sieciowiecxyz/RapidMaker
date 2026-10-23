//! HTTP only: credential verification and atomic state changes live in auth/storage.
use super::auth::{
    Auth, Error, error, now, response, session_cookie, session_name, set_cookie, unavailable,
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_auth::{
    passkeys::{PublicKeyCredential, RegisterPublicKeyCredential},
    random_token,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
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
    description = "Wymaga sesji email i CSRF/Origin. Rejestracja wymaga świeżego MFA albo lokalnego zezwolenia bootstrap. Odpowiedzi opcji i credential są standardowymi obiektami WebAuthn; weryfikacja użytkownika jest obowiązkowa.",
    request_body = Start,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn register_start(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Start>,
) -> Result<Json<Value>, Error> {
    auth.mutation(&headers).await?;
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
    description = "Wymaga sesji email i CSRF/Origin. Rejestracja wymaga świeżego MFA albo lokalnego zezwolenia bootstrap. Odpowiedzi opcji i credential są standardowymi obiektami WebAuthn; weryfikacja użytkownika jest obowiązkowa.",
    request_body = Register,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn register_finish(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<axum::response::Response, Error> {
    auth.mutation(&headers).await?;
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
    finish(&auth, &new_token, session)
}
#[utoipa::path(
    operation_id = "passkeys_login_start",
    post,
    path = "/api/auth/passkey/login/start",
    tag = "Passkey",
    summary = "Rozpocznij potwierdzenie administratora passkey.",
    description = "Wymaga sesji email i CSRF/Origin. Rejestracja wymaga świeżego MFA albo lokalnego zezwolenia bootstrap. Odpowiedzi opcji i credential są standardowymi obiektami WebAuthn; weryfikacja użytkownika jest obowiązkowa.",
    request_body = Value,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn login_start(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_input): Json<Value>,
) -> Result<Json<Value>, Error> {
    auth.mutation(&headers).await?;
    let start = auth
        .database()?
        .passkey_login_start(
            auth.webauthn.as_ref().ok_or_else(unavailable)?,
            token(&headers)?,
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
    summary = "Zweryfikuj passkey administratora i obróć sesję.",
    description = "Wymaga sesji email i CSRF/Origin. Rejestracja wymaga świeżego MFA albo lokalnego zezwolenia bootstrap. Odpowiedzi opcji i credential są standardowymi obiektami WebAuthn; weryfikacja użytkownika jest obowiązkowa.",
    request_body = Login,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn login_finish(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<axum::response::Response, Error> {
    auth.mutation(&headers).await?;
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
    finish(&auth, &new_token, session)
}
fn finish(
    auth: &Auth,
    token: &str,
    session: rapidmaker_storage::AuthSession,
) -> Result<axum::response::Response, Error> {
    response(
        json!({"user":session.user,"admin_verified":true,"csrf_token":auth.csrf(token)?}),
        &[set_cookie(
            session_name(),
            token,
            (session.expires_at - now()).clamp(0, 8 * 3600) as u32,
        )],
    )
}
#[utoipa::path(
    operation_id = "passkeys_list",
    get,
    path = "/api/auth/passkeys",
    tag = "Passkey",
    summary = "Lista passkeys administratora.",
    description = "Wymaga administratora i świeżego potwierdzenia MFA.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn list(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    auth.administrator(&headers, false).await?;
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
    description = "Wymaga administratora i świeżego potwierdzenia MFA.",
    params(("id" = String, Path, description = "Identyfikator passkey.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn remove(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    auth.administrator(&headers, true).await?;
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
