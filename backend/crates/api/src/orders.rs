use super::auth::{Auth, Error, error, now, unavailable};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use rapidmaker_storage::Storage;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use utoipa_axum::router::OpenApiRouter as Router;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .routes(utoipa_axum::routes!(list))
        .routes(utoipa_axum::routes!(detail))
        .routes(utoipa_axum::routes!(files))
        .routes(utoipa_axum::routes!(file))
        .routes(utoipa_axum::routes!(messages, message))
        .routes(utoipa_axum::routes!(payment_status))
        .routes(utoipa_axum::routes!(claim))
        .with_state(auth)
}
pub fn admin_router<S: Clone + Send + Sync + 'static>(db: Option<Storage>) -> Router<S> {
    Router::new()
        .routes(utoipa_axum::routes!(admin_list))
        .routes(utoipa_axum::routes!(admin_files))
        .routes(utoipa_axum::routes!(admin_file))
        .routes(utoipa_axum::routes!(status))
        .routes(utoipa_axum::routes!(admin_messages, admin_message))
        .routes(utoipa_axum::routes!(stats))
        .with_state(db)
}
async fn scope(
    auth: &Auth,
    headers: &HeaderMap,
    mutate: bool,
) -> Result<(Option<String>, Option<String>), Error> {
    let user = if super::auth::session_cookie(headers)?.is_some() {
        Some(
            if mutate {
                auth.mutation(headers).await?
            } else {
                auth.current(headers).await?
            }
            .user
            .id,
        )
    } else {
        None
    };
    if mutate {
        auth.check_origin(headers)?;
    }
    let owner = super::files::owner(headers);
    if user.is_none() && owner.is_none() {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "Zaloguj się lub użyj przeglądarki zamówienia.",
        ));
    }
    Ok((user, owner))
}
#[utoipa::path(
    operation_id = "orders_list",
    get,
    path = "/api/account/orders",
    tag = "Zamówienia",
    summary = "Lista własnych zamówień.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []), ("guest" = []))
)]
async fn list(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, false).await?;
    Ok(Json(
        json!({"orders":auth.database()?.orders(user.as_deref(),owner.as_deref()).await.map_err(|_|unavailable())?}),
    ))
}
#[utoipa::path(
    operation_id = "orders_detail",
    get,
    path = "/api/account/orders/{id}",
    tag = "Zamówienia",
    summary = "Szczegóły własnego zamówienia.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []), ("guest" = []))
)]
async fn detail(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, false).await?;
    Ok(Json(
        auth.database()?
            .order(&id, user.as_deref(), owner.as_deref())
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(not_found)?,
    ))
}
fn not_found() -> Error {
    error(StatusCode::NOT_FOUND, "Nie znaleziono zamówienia.")
}
#[utoipa::path(
    operation_id = "orders_messages",
    get,
    path = "/api/account/orders/{id}/messages",
    tag = "Zamówienia",
    summary = "Rozmowa własnego zamówienia.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []), ("guest" = []))
)]
async fn messages(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, false).await?;
    let messages = auth
        .database()?
        .order_messages(&id, user.as_deref(), owner.as_deref(), false)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({"messages":messages})))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = OrderMessage)]
#[serde(deny_unknown_fields)]
struct Message {
    body: String,
}
#[utoipa::path(
    operation_id = "orders_message",
    post,
    path = "/api/account/orders/{id}/messages",
    tag = "Zamówienia",
    summary = "Wyślij wiadomość do obsługi.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    request_body = Message,
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []), ("guest" = []))
)]
async fn message(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(m): Json<Message>,
) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, true).await?;
    valid_body(&m.body)?;
    let to = std::env::var("MAIL_REPLY_TO").ok();
    if !auth
        .database()?
        .order_message(
            &id,
            user.as_deref(),
            owner.as_deref(),
            false,
            &m.body,
            to.as_deref(),
            now(),
        )
        .await
        .map_err(|_| {
            error(
                StatusCode::TOO_MANY_REQUESTS,
                "Nie można dodać wiadomości. Spróbuj później.",
            )
        })?
    {
        return Err(not_found());
    }
    Ok(Json(json!({"ok":true})))
}
fn valid_body(body: &str) -> Result<(), Error> {
    if body.trim().is_empty()
        || body.len() > 4000
        || body
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        Err(error(
            StatusCode::BAD_REQUEST,
            "Wiadomość musi mieć od 1 do 4000 bajtów.",
        ))
    } else {
        Ok(())
    }
}
#[utoipa::path(
    operation_id = "orders_claim",
    post,
    path = "/api/auth/claim-guest",
    tag = "Zamówienia",
    summary = "Przypisz zasoby gościa do zalogowanego konta.",
    description = "Wymaga obu cookies: zalogowanej sesji konta i sesji gościa. Atomowo przypisuje własne modele, wyceny i zamówienia gościa. Wymaga CSRF i dozwolonego Origin.",
    request_body = Value,
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "guest" = [], "csrf" = []))
)]
async fn claim(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_input): Json<Value>,
) -> Result<Json<Value>, Error> {
    let user = auth.mutation(&headers).await?.user;
    let owner = super::files::owner(&headers)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "Brak sesji gościa."))?;
    let count = auth
        .database()?
        .claim_guest(&user.id, &owner, now())
        .await
        .map_err(|_| unavailable())?;
    Ok(Json(json!({"resources_claimed":count})))
}
#[utoipa::path(
    operation_id = "orders_payment_status",
    post,
    path = "/api/account/orders/{id}/payment-status",
    tag = "Zamówienia",
    summary = "Sprawdź płatność zamówienia u operatora.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    request_body = Value,
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []), ("guest" = []))
)]
async fn payment_status(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(_input): Json<Value>,
) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, true).await?;
    let db = auth.database()?;
    let order = db
        .order(&id, user.as_deref(), owner.as_deref())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(not_found)?;
    super::checkout::refresh_order(db, &id, &order).await?;
    Ok(Json(
        db.order(&id, user.as_deref(), owner.as_deref())
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(not_found)?,
    ))
}
#[utoipa::path(
    operation_id = "orders_admin_list",
    get,
    path = "/api/admin/orders",
    tag = "Admin",
    summary = "Lista zamówień operatora.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn admin_list(State(db): State<Option<Storage>>) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"orders":db.as_ref().ok_or_else(unavailable)?.admin_orders().await.map_err(|_|unavailable())?}),
    ))
}
#[derive(Deserialize, utoipa::ToSchema)]
#[schema(as = OrderStatus)]
#[serde(deny_unknown_fields)]
struct Status {
    status: String,
}
#[utoipa::path(
    operation_id = "orders_status",
    post,
    path = "/api/admin/orders/{id}/status",
    tag = "Admin",
    summary = "Zmień status realizacji zamówienia.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    request_body = Status,
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn status(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
    Json(s): Json<Status>,
) -> Result<Json<Value>, Error> {
    if !db
        .as_ref()
        .ok_or_else(unavailable)?
        .order_status(&id, &s.status, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(error(
            StatusCode::CONFLICT,
            "Nieprawidłowa zmiana statusu lub brak zamówienia.",
        ));
    }
    Ok(Json(json!({"ok":true})))
}
#[utoipa::path(
    operation_id = "orders_admin_messages",
    get,
    path = "/api/admin/orders/{id}/messages",
    tag = "Admin",
    summary = "Pobierz rozmowę zamówienia.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn admin_messages(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"messages":db.as_ref().ok_or_else(unavailable)?.order_messages(&id,None,None,true).await.map_err(|_|unavailable())?.ok_or_else(not_found)?}),
    ))
}
#[utoipa::path(
    operation_id = "orders_admin_message",
    post,
    path = "/api/admin/orders/{id}/messages",
    tag = "Admin",
    summary = "Wyślij wiadomość do klienta.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    request_body = Message,
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = [], "csrf" = []))
)]
async fn admin_message(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
    Json(m): Json<Message>,
) -> Result<Json<Value>, Error> {
    valid_body(&m.body)?;
    if !db
        .as_ref()
        .ok_or_else(unavailable)?
        .order_message(&id, None, None, true, &m.body, None, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(not_found());
    }
    Ok(Json(json!({"ok":true})))
}
#[utoipa::path(
    operation_id = "orders_stats",
    get,
    path = "/api/admin/notifications",
    tag = "Admin",
    summary = "Statystyki kolejki powiadomień email.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn stats(State(db): State<Option<Storage>>) -> Result<Json<Value>, Error> {
    Ok(Json(
        serde_json::to_value(
            db.as_ref()
                .ok_or_else(unavailable)?
                .email_stats()
                .await
                .map_err(|_| unavailable())?,
        )
        .map_err(|_| unavailable())?,
    ))
}

async fn owned_order_db<'a>(
    auth: &'a Auth,
    headers: &HeaderMap,
    id: &str,
) -> Result<&'a Storage, Error> {
    let (user, owner) = scope(auth, headers, false).await?;
    let db = auth.database()?;
    db.order(id, user.as_deref(), owner.as_deref())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(not_found)?;
    Ok(db)
}
async fn file_list(db: &Storage, id: &str, admin: bool) -> Result<Json<Value>, Error> {
    let mut files = db.order_files(id, now()).await.map_err(|_| unavailable())?;
    let prefix = if admin { "admin" } else { "account" };
    for file in &mut files {
        if file["available"] == true {
            let model = file["model_id"].as_str().ok_or_else(unavailable)?;
            let base = format!("/api/{prefix}/orders/{id}/files/{model}");
            file["source_url"] = json!(format!("{base}/source"));
            if file["preview_bytes"].as_i64().is_some_and(|size| size > 0) {
                file["preview_url"] = json!(format!("{base}/preview.glb"));
            }
            if admin {
                file["mesh_url"] = json!(format!("{base}/mesh"));
            }
        }
    }
    Ok(Json(json!({"files":files})))
}
async fn order_artifact(
    db: &Storage,
    id: &str,
    model: &str,
    kind: &str,
    admin: bool,
) -> Result<Response, Error> {
    let record = db
        .order_model(id, model, now())
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| {
            error(
                StatusCode::GONE,
                "Plik zamówienia jest niedostępny lub minął czas jego przechowywania.",
            )
        })?;
    super::files::order_artifact(record, kind, admin)
}
#[utoipa::path(
    operation_id = "orders_files",
    get,
    path = "/api/account/orders/{id}/files",
    tag = "Zamówienia",
    summary = "Lista dostępnych plików zamówienia.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 429, description = "Przekroczono limit prób.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []), ("guest" = []))
)]
async fn files(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    file_list(owned_order_db(&auth, &headers, &id).await?, &id, false).await
}
#[utoipa::path(
    operation_id = "orders_file",
    get,
    path = "/api/account/orders/{id}/files/{model}/{kind}",
    tag = "Zamówienia",
    summary = "Pobierz własny plik zamówienia.",
    description = "Własność konta lub cookie gościa jest sprawdzana po stronie serwera. Mutacje wymagają dozwolonego Origin; sesja konta wymaga także CSRF.",
    params(("id" = String, Path, description = "Identyfikator zamówienia."), ("model" = String, Path, description = "Identyfikator modelu."), ("kind" = String, Path, description = "source albo preview.glb; mesh dostępne tylko administratorowi.")),
    responses((status = 200, description = "Powodzenie.", body = Vec<u8>, content_type = "application/octet-stream"), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 410, description = "Plik wygasł lub jest niedostępny.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []), ("guest" = []))
)]
async fn file(
    State(auth): State<Arc<Auth>>,
    Path((id, model, kind)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    order_artifact(
        owned_order_db(&auth, &headers, &id).await?,
        &id,
        &model,
        &kind,
        false,
    )
    .await
}
#[utoipa::path(
    operation_id = "orders_admin_files",
    get,
    path = "/api/admin/orders/{id}/files",
    tag = "Admin",
    summary = "Lista plików zamówienia dla operatora.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    params(("id" = String, Path, description = "Identyfikator zamówienia.")),
    responses((status = 200, description = "Powodzenie.", body = Value), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn admin_files(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    file_list(db.as_ref().ok_or_else(unavailable)?, &id, true).await
}
#[utoipa::path(
    operation_id = "orders_admin_file",
    get,
    path = "/api/admin/orders/{id}/files/{model}/{kind}",
    tag = "Admin",
    summary = "Pobierz plik zamówienia jako operator.",
    description = "Wymaga sesji administratora potwierdzonej passkey (MFA). Mutacje wymagają CSRF i dozwolonego Origin.",
    params(("id" = String, Path, description = "Identyfikator zamówienia."), ("model" = String, Path, description = "Identyfikator modelu."), ("kind" = String, Path, description = "source, preview.glb albo mesh.")),
    responses((status = 200, description = "Powodzenie.", body = Vec<u8>, content_type = "application/octet-stream"), (status = 400, description = "Nieprawidłowe dane.", body = Value), (status = 401, description = "Brak lub wygaśnięcie sesji.", body = Value), (status = 403, description = "Brak uprawnień, MFA lub nieprawidłowy CSRF/Origin.", body = Value), (status = 404, description = "Nie znaleziono zasobu właściciela.", body = Value), (status = 409, description = "Niedozwolona zmiana stanu.", body = Value), (status = 410, description = "Plik wygasł lub jest niedostępny.", body = Value), (status = 503, description = "Usługa chwilowo niedostępna.", body = Value)),
    security(("session" = []))
)]
async fn admin_file(
    State(db): State<Option<Storage>>,
    Path((id, model, kind)): Path<(String, String, String)>,
) -> Result<Response, Error> {
    order_artifact(
        db.as_ref().ok_or_else(unavailable)?,
        &id,
        &model,
        &kind,
        true,
    )
    .await
}
