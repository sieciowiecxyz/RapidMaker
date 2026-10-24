//! Uses the shipment lease and existing worker; uncertain PUTs are recovered with GET only.
use super::{
    checkout::{App, now},
    shipment_actions::{available, offered},
};
use rapidmaker_shipping::{PickupDate, ShipmentError};
use rapidmaker_storage::{Shipment, ShipmentAction, StorageError};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Input {
    package_ids: Vec<String>,
    provider_ids: Vec<String>,
    pickup_date: Option<PickupDate>,
    #[serde(default)]
    repeat: bool,
}
async fn save(
    app: &App,
    s: &Shipment,
    a: &ShipmentAction,
    state: &str,
    result: &Value,
    canceled: &[String],
) -> Result<(), StorageError> {
    if !app
        .db
        .as_ref()
        .ok_or("database unavailable")?
        .shipment_action_update(s, &a.id, state, result, canceled, now())
        .await?
    {
        return Err("shipment action lease lost".into());
    }
    Ok(())
}
pub(super) async fn process(app: &App, s: &Shipment) -> Result<bool, StorageError> {
    let db = app.db.as_ref().ok_or("database unavailable")?;
    let Some(a) = db.shipment_action_active(s).await? else {
        if db.shipment_action_busy(&s.order_id).await? {
            db.shipment_retry(s, 900, now()).await?;
            return Ok(true);
        }
        return Ok(false);
    };
    let input: Input = serde_json::from_value(a.input.clone())?;
    let api = app.shipping.as_ref().ok_or("shipping unavailable")?;
    if s.attempts >= 999 {
        save(
            app,
            s,
            &a,
            "manual_review",
            &json!({"error":"retry_exhausted"}),
            &[],
        )
        .await?;
        db.shipment_retry(s, 900, now()).await?;
        return Ok(true);
    }
    if a.state == "queued" {
        if !db.shipment_action_authorized(&a.id).await? {
            save(
                app,
                s,
                &a,
                "failed",
                &json!({"error":"operator_disabled"}),
                &[],
            )
            .await?;
            db.shipment_retry(s, 1, now()).await?;
            return Ok(true);
        }
        // Availability may have changed between admin approval and the worker lease.
        let check = available(app, &input.provider_ids, &a.kind, input.repeat).await;
        if let Err((status, _)) = check {
            if status.is_client_error() {
                save(
                    app,
                    s,
                    &a,
                    "failed",
                    &json!({"error":"operation_unavailable"}),
                    &[],
                )
                .await?;
                db.shipment_retry(s, 1, now()).await?;
                return Ok(true);
            }
            return Err("logistics availability unavailable".into());
        }
        if a.kind == "pickup" {
            let date = input.pickup_date.as_ref().ok_or("missing pickup date")?;
            let proposals = api
                .pickup_proposals(&input.provider_ids, &date.date, input.repeat)
                .await
                .map_err(|_| "pickup proposals unavailable")?;
            if !offered(&proposals, &input.provider_ids, date) {
                save(
                    app,
                    s,
                    &a,
                    "failed",
                    &json!({"error":"pickup_window_unavailable"}),
                    &[],
                )
                .await?;
                db.shipment_retry(s, 1, now()).await?;
                return Ok(true);
            }
        }
        // This durable transition must happen before the only mutating HTTP attempt.
        save(app, s, &a, "submitting", &json!({}), &[]).await?;
        let submitted = if a.kind == "pickup" {
            api.request_pickup(
                &a.id,
                &input.provider_ids,
                input.pickup_date.as_ref().ok_or("missing date")?,
            )
            .await
        } else {
            api.cancel_shipments(&a.id, &input.provider_ids).await
        };
        match submitted {
            Ok(()) => save(app, s, &a, "pending", &json!({}), &[]).await?,
            Err(
                ShipmentError::Invalid(_) | ShipmentError::Unauthorized | ShipmentError::NotFound,
            ) => {
                save(
                    app,
                    s,
                    &a,
                    "failed",
                    &json!({"error":"provider_rejected"}),
                    &[],
                )
                .await?;
                db.shipment_retry(s, 1, now()).await?;
                return Ok(true);
            }
            Err(_) => {
                db.shipment_retry(s, 30, now()).await?;
                return Ok(true);
            }
        }
        db.shipment_retry(s, 3, now()).await?;
        return Ok(true);
    }
    let command = match api.logistics_command(&a.kind, &a.id).await {
        Ok(v) => v,
        Err(ShipmentError::NotFound) => {
            // A lost response is not permission to send another PUT, even with the same UUID.
            let misses = a.result["not_found_count"].as_u64().unwrap_or(0) + 1;
            save(
                app,
                s,
                &a,
                if misses >= 3 {
                    "manual_review"
                } else {
                    "pending"
                },
                &json!({"error":"command_not_found","not_found_count":misses}),
                &[],
            )
            .await?;
            db.shipment_retry(s, 30, now()).await?;
            return Ok(true);
        }
        Err(_) => return Err("logistics command unavailable".into()),
    };
    if command["uuid"].as_str() != Some(a.id.as_str())
        || !matches!(
            command["status"].as_str(),
            Some("queueing" | "running" | "successful" | "partial_success" | "error")
        )
    {
        save(
            app,
            s,
            &a,
            "manual_review",
            &json!({"error":"invalid_provider_response"}),
            &[],
        )
        .await?;
        db.shipment_retry(s, 900, now()).await?;
        return Ok(true);
    }
    let running = matches!(command["status"].as_str(), Some("queueing" | "running"));
    let mut result = json!({"command":command});
    let (state, canceled) = if a.kind == "pickup" {
        let state = pickup_result(&result["command"], &input.provider_ids);
        (state, vec![])
    } else {
        let mut canceled = vec![];
        for (local, provider) in input.package_ids.iter().zip(&input.provider_ids) {
            let p = api
                .get_shipment(provider)
                .await
                .map_err(|_| "cancellation confirmation unavailable")?;
            if p.state == "cancelled" {
                canceled.push(local.clone());
            }
        }
        let state = if canceled.len() == input.package_ids.len() {
            "succeeded"
        } else if running || deferred(&result["command"]) {
            "pending"
        } else if result["command"]["status"] == "successful" {
            "manual_review"
        } else if canceled.is_empty() {
            "failed"
        } else {
            "partial"
        };
        (state, canceled)
    };
    if a.kind == "cancel" {
        result["confirmed_canceled_package_ids"] = json!(canceled);
    }
    save(app, s, &a, state, &result, &canceled).await?;
    let delay = if state != "pending" {
        1
    } else if deferred(&result["command"]) {
        900
    } else {
        60
    };
    db.shipment_retry(s, delay, now()).await?;
    Ok(true)
}
fn deferred(command: &Value) -> bool {
    command["cancel_command_details"]
        .as_array()
        .is_some_and(|rows| {
            rows.iter().any(|r| {
                r["scheduled_cancel_date"]
                    .as_str()
                    .is_some_and(|v| !v.is_empty())
            })
        })
}
fn pickup_result(command: &Value, selected: &[String]) -> &'static str {
    if matches!(command["status"].as_str(), Some("queueing" | "running")) {
        return "pending";
    }
    let Some(details) = command["pickup_details"].as_array() else {
        return if command["status"] == "error" {
            "failed"
        } else {
            "manual_review"
        };
    };
    let mut confirmed = std::collections::HashSet::new();
    for row in details {
        if !row["pickup_id"]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 128)
        {
            return "manual_review";
        }
        let Some(ids) = row["package_ids"].as_array() else {
            return "manual_review";
        };
        for id in ids {
            let id = id
                .as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|n| n.to_string()));
            let Some(id) = id.filter(|id| selected.contains(id)) else {
                return "manual_review";
            };
            confirmed.insert(id);
        }
    }
    if confirmed.len() == selected.len() {
        "succeeded"
    } else if command["status"] == "successful" {
        "manual_review"
    } else if confirmed.is_empty() {
        "failed"
    } else {
        "partial"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pickup_acceptance_is_not_confirmation_and_foreign_ids_are_rejected() {
        assert_eq!(
            pickup_result(&json!({"status":"running"}), &["1".into()]),
            "pending"
        );
        assert_eq!(
            pickup_result(&json!({"status":"successful"}), &["1".into()]),
            "manual_review"
        );
        let c = json!({"status":"successful","pickup_details":[{"pickup_id":"P123","package_ids":[1]}]});
        assert_eq!(pickup_result(&c, &["1".into()]), "succeeded");
        assert_eq!(
            pickup_result(&c, &["1".into(), "2".into()]),
            "manual_review"
        );
        let mut partial = c.clone();
        partial["status"] = json!("partial_success");
        assert_eq!(
            pickup_result(&partial, &["1".into(), "2".into()]),
            "partial"
        );
        assert_eq!(pickup_result(&c, &["2".into()]), "manual_review");
    }
    #[test]
    fn scheduled_cancellation_is_still_pending() {
        assert!(deferred(
            &json!({"cancel_command_details":[{"cancel_success":true,"scheduled_cancel_date":"2026-10-26T00:00:00+01:00"}]})
        ));
        assert!(!deferred(
            &json!({"cancel_command_details":[{"cancel_success":true}]})
        ));
    }
}
