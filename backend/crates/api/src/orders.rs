use super::auth::{Auth, Error, error, now, unavailable};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use rapidmaker_storage::Storage;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub fn router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route("/api/account/orders", get(list))
        .route("/api/account/orders/{id}", get(detail))
        .route("/api/account/orders/{id}/files", get(files))
        .route("/api/account/orders/{id}/files/{model}/{kind}", get(file))
        .route(
            "/api/account/orders/{id}/messages",
            get(messages).post(message),
        )
        .route(
            "/api/account/orders/{id}/payment-status",
            post(payment_status),
        )
        .route("/api/auth/claim-guest", post(claim))
        .with_state(auth)
}
pub fn admin_router<S: Clone + Send + Sync + 'static>(db: Option<Storage>) -> Router<S> {
    Router::new()
        .route("/api/admin/orders", get(admin_list))
        .route("/api/admin/orders/{id}/files", get(admin_files))
        .route(
            "/api/admin/orders/{id}/files/{model}/{kind}",
            get(admin_file),
        )
        .route("/api/admin/orders/{id}/status", post(status))
        .route(
            "/api/admin/orders/{id}/messages",
            get(admin_messages).post(admin_message),
        )
        .route("/api/admin/notifications", get(stats))
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
async fn list(State(auth): State<Arc<Auth>>, headers: HeaderMap) -> Result<Json<Value>, Error> {
    let (user, owner) = scope(&auth, &headers, false).await?;
    Ok(Json(
        json!({"orders":auth.database()?.orders(user.as_deref(),owner.as_deref()).await.map_err(|_|unavailable())?}),
    ))
}
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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    body: String,
}
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
async fn claim(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Json(_): Json<Value>,
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
async fn payment_status(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(_): Json<Value>,
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
async fn admin_list(State(db): State<Option<Storage>>) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"orders":db.as_ref().ok_or_else(unavailable)?.admin_orders().await.map_err(|_|unavailable())?}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Status {
    status: String,
}
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
async fn admin_messages(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(Json(
        json!({"messages":db.as_ref().ok_or_else(unavailable)?.order_messages(&id,None,None,true).await.map_err(|_|unavailable())?.ok_or_else(not_found)?}),
    ))
}
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
async fn files(
    State(auth): State<Arc<Auth>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Error> {
    file_list(owned_order_db(&auth, &headers, &id).await?, &id, false).await
}
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
async fn admin_files(
    State(db): State<Option<Storage>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    file_list(db.as_ref().ok_or_else(unavailable)?, &id, true).await
}
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
