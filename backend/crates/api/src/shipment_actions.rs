//! Admin-only logistics commands; external writes start only after PostgreSQL commit.
use super::{
    auth::Auth,
    checkout::{App, Problem, Result, error, now, unavailable},
    shipments::{domain, provider_error, stored},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use rapidmaker_shipping::PickupDate;
use rapidmaker_storage::{Shipment, ShipmentAction, ShipmentError};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Proposals {
    package_ids: Vec<String>,
    /// YYYY-MM-DD, in the sender's local calendar.
    ready_date: String,
    #[serde(default)]
    repeat: bool,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Pickup {
    version: i64,
    request_id: String,
    package_ids: Vec<String>,
    pickup_date: PickupDate,
    #[serde(default)]
    repeat: bool,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Cancel {
    version: i64,
    request_id: String,
    package_ids: Vec<String>,
}
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Reconcile {
    version: i64,
}

pub(super) fn router() -> OpenApiRouter<Arc<App>> {
    OpenApiRouter::new()
        .routes(routes!(history))
        .routes(routes!(proposals))
        .routes(routes!(pickup))
        .routes(routes!(cancel))
        .routes(routes!(reconcile))
}
fn selected(s: &Shipment, local: &[String]) -> Result<Vec<String>> {
    if local.is_empty()
        || local.len() > 30
        || local
            .iter()
            .enumerate()
            .any(|(i, id)| local[..i].contains(id))
    {
        return Err(domain(ShipmentError::Invalid));
    }
    local
        .iter()
        .map(|id| {
            s.packages
                .iter()
                .find(|p| &p.id == id)
                .and_then(|p| p.provider_id.clone())
                .ok_or_else(|| domain(ShipmentError::Invalid))
        })
        .collect()
}
pub(super) async fn available(app: &App, ids: &[String], kind: &str, repeat: bool) -> Result<()> {
    let api = app.shipping.as_ref().ok_or_else(unavailable)?;
    for id in ids {
        let p = api.get_shipment(id).await.map_err(provider_error)?;
        let flag = match (kind, repeat) {
            ("cancel", _) => "cancel_available",
            (_, true) => "repickup",
            _ => "pickup_available",
        };
        if p.body[flag] != true || !matches!(p.state.as_str(), "ordered" | "tracking") {
            return Err(error(
                StatusCode::CONFLICT,
                "shipping_operation_unavailable",
                "Przewoźnik nie pozwala teraz na tę operację. Odśwież stan przesyłki.",
            ));
        }
    }
    Ok(())
}
fn provider_id(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::to_owned)
        .or_else(|| v.as_u64().map(|id| id.to_string()))
}
/// A common window must be offered for every selected consignment, not just the first.
pub(super) fn offered(value: &Value, ids: &[String], date: &PickupDate) -> bool {
    ids.iter().all(|id| {
        value["packages"].as_array().is_some_and(|groups| {
            groups.iter().any(|g| {
                provider_id(&g["package_id"]).as_ref() == Some(id)
                    && g["proposals"].as_array().is_some_and(|ps| {
                        ps.iter().any(|p| {
                            p["available"] == true
                                && p["date"] == date.date
                                && p["min_time"] == date.min_time
                                && p["max_time"] == date.max_time
                        })
                    })
            })
        })
    })
}
async fn replay(app: &App, order: &str, request: &str) -> Result<bool> {
    let request = uuid::Uuid::parse_str(request)
        .map_err(|_| domain(ShipmentError::Invalid))?
        .to_string();
    Ok(app
        .db()?
        .shipment_actions(order)
        .await
        .map_err(|_| unavailable())?
        .iter()
        .any(|a| a.request_id == request))
}
#[utoipa::path(get,path="/api/admin/orders/{id}/shipment/actions",operation_id="shipment_actions",tag="Admin",
 summary="Historia podjazdów i anulowań; UUID i stan każdej trwałej operacji.",params(("id"=String,Path)),
 responses((status=200,body=Vec<ShipmentAction>),(status=403,body=Problem)),security(("session"=[])))]
async fn history(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<ShipmentAction>>> {
    auth.administrator(&headers, false).await?;
    stored(&app, &id).await?;
    Ok(Json(
        app.db()?
            .shipment_actions(&id)
            .await
            .map_err(|_| unavailable())?,
    ))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/pickup/proposals",operation_id="shipment_pickup_proposals",tag="Admin",
 summary="Dostępne terminy podjazdu dla wybranych przesyłek zamówienia.",params(("id"=String,Path)),request_body=Proposals,
 responses((status=200,body=Value),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn proposals(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Proposals>,
) -> Result<Json<Value>> {
    auth.administrator(&headers, true).await?;
    let s = stored(&app, &id).await?;
    let ids = selected(&s, &input.package_ids)?;
    let _permit = app.permit()?;
    available(&app, &ids, "pickup", input.repeat).await?;
    Ok(Json(
        app.shipping
            .as_ref()
            .ok_or_else(unavailable)?
            .pickup_proposals(&ids, &input.ready_date, input.repeat)
            .await
            .map_err(provider_error)?,
    ))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/pickup",operation_id="shipment_pickup",tag="Admin",
 summary="Zleć podjazd w dostępnym terminie; 202 nie potwierdza jeszcze rezerwacji.",params(("id"=String,Path)),request_body=Pickup,
 responses((status=202,body=ShipmentAction),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn pickup(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Pickup>,
) -> Result<(StatusCode, Json<ShipmentAction>)> {
    let user = auth.administrator(&headers, true).await?.user;
    let s = stored(&app, &id).await?;
    let ids = selected(&s, &input.package_ids)?;
    let value = json!({"package_ids":input.package_ids,"provider_ids":ids,"pickup_date":input.pickup_date,"repeat":input.repeat});
    if !replay(&app, &id, &input.request_id).await? {
        let _permit = app.permit()?;
        available(&app, &ids, "pickup", input.repeat).await?;
        let ps = app
            .shipping
            .as_ref()
            .ok_or_else(unavailable)?
            .pickup_proposals(&ids, &input.pickup_date.date, input.repeat)
            .await
            .map_err(provider_error)?;
        if !offered(&ps, &ids, &input.pickup_date) {
            return Err(error(
                StatusCode::CONFLICT,
                "pickup_window_unavailable",
                "Wybrany termin nie jest dostępny dla wszystkich przesyłek. Pobierz aktualne terminy.",
            ));
        }
    }
    let a = app
        .db()?
        .shipment_action_enqueue(
            &id,
            input.version,
            "pickup",
            &input.request_id,
            &value,
            &user.id,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .map_err(domain)?;
    Ok((StatusCode::ACCEPTED, Json(a)))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/cancel",operation_id="shipment_cancel",tag="Admin",
 summary="Zleć anulowanie wybranych przesyłek; nie anuluje zamówienia ani płatności.",params(("id"=String,Path)),request_body=Cancel,
 responses((status=202,body=ShipmentAction),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn cancel(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Cancel>,
) -> Result<(StatusCode, Json<ShipmentAction>)> {
    let user = auth.administrator(&headers, true).await?.user;
    let s = stored(&app, &id).await?;
    let ids = selected(&s, &input.package_ids)?;
    let value = json!({"package_ids":input.package_ids,"provider_ids":ids});
    if !replay(&app, &id, &input.request_id).await? {
        let _permit = app.permit()?;
        available(&app, &ids, "cancel", false).await?;
    }
    let a = app
        .db()?
        .shipment_action_enqueue(
            &id,
            input.version,
            "cancel",
            &input.request_id,
            &value,
            &user.id,
            now(),
        )
        .await
        .map_err(|_| unavailable())?
        .map_err(domain)?;
    Ok((StatusCode::ACCEPTED, Json(a)))
}
#[utoipa::path(post,path="/api/admin/orders/{id}/shipment/actions/{action_id}/reconcile",operation_id="shipment_action_reconcile",tag="Admin",
 summary="Ponownie sprawdź niepewną operację po UUID; nie wysyła nowego zlecenia.",params(("id"=String,Path),("action_id"=String,Path)),request_body=Reconcile,
 responses((status=202),(status=409,body=Problem)),security(("session"=[],"csrf"=[])))]
async fn reconcile(
    State(app): State<Arc<App>>,
    Extension(auth): Extension<Arc<Auth>>,
    headers: HeaderMap,
    Path((id, action)): Path<(String, String)>,
    Json(input): Json<Reconcile>,
) -> Result<StatusCode> {
    let user = auth.administrator(&headers, true).await?.user;
    if !app
        .db()?
        .shipment_action_resume(&id, input.version, &action, &user.id, now())
        .await
        .map_err(|_| unavailable())?
    {
        return Err(domain(ShipmentError::Conflict));
    }
    Ok(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pickup_requires_the_same_available_window_for_every_package() {
        let d = PickupDate {
            date: "2026-10-26".into(),
            min_time: "10:00".into(),
            max_time: "14:00".into(),
        };
        let mut value = json!({"packages":[{"package_id":1,"proposals":[{"date":d.date,"min_time":d.min_time,"max_time":d.max_time,"available":true}]}]});
        assert!(offered(&value, &["1".into()], &d));
        assert!(!offered(&value, &["1".into(), "2".into()], &d));
        value["packages"][0]["proposals"][0]["available"] = json!(false);
        assert!(!offered(&value, &["1".into()], &d));
    }
}
