use super::auth::{Auth, Error, error, now, unavailable};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_storage::{InvoiceDraft, Production, ProductionError};
use serde_json::{Value, json};
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) fn router(auth: Arc<Auth>) -> OpenApiRouter {
    OpenApiRouter::new()
        .routes(routes!(get, save))
        .routes(routes!(invoice, save_invoice))
        .with_state(auth)
}
fn result<T>(value: Result<T, ProductionError>) -> Result<T, Error> {
    value.map_err(|v| match v {
        ProductionError::NotFound => error(StatusCode::NOT_FOUND, "Nie znaleziono zamówienia."),
        ProductionError::Conflict => error(
            StatusCode::CONFLICT,
            "Dane zmieniły się w innym oknie. Odśwież przed ponownym zapisem.",
        ),
        ProductionError::Invalid(message) => error(StatusCode::UNPROCESSABLE_ENTITY, message),
    })
}
#[utoipa::path(get,path="/api/admin/orders/{id}/production",tag="Admin",operation_id="admin_production",params(("id"=String,Path)),responses((status=200,body=Production),(status=404,body=Value)),security(("session"=[])))]
async fn get(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Production>, Error> {
    auth.administrator(&headers, false).await?;
    Ok(Json(
        auth.database()?
            .production(&id)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "Nie znaleziono zamówienia."))?,
    ))
}
#[utoipa::path(put,path="/api/admin/orders/{id}/production",tag="Admin",operation_id="admin_save_production",params(("id"=String,Path)),request_body=Production,responses((status=200,body=Production),(status=409,body=Value),(status=422,body=Value)),security(("session"=[],"csrf"=[])))]
async fn save(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Production>,
) -> Result<Json<Production>, Error> {
    let actor = auth.administrator(&headers, true).await?;
    Ok(Json(result(
        auth.database()?
            .save_production(&id, &actor.user.id, input, now())
            .await
            .map_err(|_| unavailable())?,
    )?))
}
#[utoipa::path(get,path="/api/admin/orders/{id}/invoice-draft",tag="Admin",operation_id="admin_invoice_draft",params(("id"=String,Path)),responses((status=200,body=Value),(status=404,body=Value)),security(("session"=[])))]
async fn invoice(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    auth.administrator(&headers, false).await?;
    Ok(Json(
        json!({"draft":auth.database()?.invoice_draft(&id).await.map_err(|_|unavailable())?.ok_or_else(||error(StatusCode::NOT_FOUND,"Nie znaleziono zamówienia."))?}),
    ))
}
#[utoipa::path(put,path="/api/admin/orders/{id}/invoice-draft",tag="Admin",operation_id="admin_save_invoice_draft",params(("id"=String,Path)),request_body=InvoiceDraft,responses((status=200,body=InvoiceDraft),(status=409,body=Value),(status=422,body=Value)),security(("session"=[],"csrf"=[])))]
async fn save_invoice(
    State(auth): State<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<InvoiceDraft>,
) -> Result<Json<InvoiceDraft>, Error> {
    let actor = auth.administrator(&headers, true).await?;
    Ok(Json(result(
        auth.database()?
            .save_invoice_draft(&id, &actor.user.id, input, now())
            .await
            .map_err(|_| unavailable())?,
    )?))
}
