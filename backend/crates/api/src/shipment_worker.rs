//! One bounded shipment worker; uncertain writes are reconciled, never recreated.
use super::{
    checkout::{App, now},
    shipments::{current_quote, provider_error},
};
use rapidmaker_shipping::{ShipmentError, matches_prepared};
use rapidmaker_storage::{Shipment, ShipmentPackage};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::watch,
    task::{JoinHandle, JoinSet},
};
use tracing::Instrument;
type Failure = rapidmaker_storage::StorageError;
type PollResult = (
    ShipmentPackage,
    Result<
        (
            rapidmaker_shipping::ProviderPackage,
            rapidmaker_shipping::Tracking,
        ),
        ShipmentError,
    >,
);

pub(super) struct ShipmentWorker {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl ShipmentWorker {
    pub(super) fn start(app: Arc<App>) -> Self {
        let (stop, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            loop {
                if *receiver.borrow() {
                    break;
                }
                tokio::select! {
                    _=run_once(&app)=>{},
                    _=receiver.changed()=>break,
                }
                tokio::select! {_=tokio::time::sleep(Duration::from_secs(3))=>{},_=receiver.changed()=>break}
            }
        });
        Self { stop, task }
    }
    pub(super) async fn shutdown(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(25), &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => {
                tracing::error!(event = "shipment_worker_failed", reason = "task_failure")
            }
            Err(_) => {
                task.abort();
                tracing::error!(
                    event = "shipment_worker_shutdown_timeout",
                    reason = "shutdown_timeout"
                );
            }
        }
    }
}
async fn run_once(app: &Arc<App>) {
    let Some(db) = &app.db else { return };
    let s = match db.shipment_lease(now()).await {
        Ok(Some(s)) => s,
        Ok(None) => return,
        Err(_) => {
            tracing::error!(event = "shipment_claim_failed", reason = "database");
            return;
        }
    };
    let span = tracing::info_span!("shipment_job", shipment_id = %s.id, order_id = %s.order_id, attempt = s.attempts);
    async {
        let started = std::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(90), process(app, &s)).await;
        if !matches!(result, Ok(Ok(()))) {
            let reason = if result.is_err() {
                "timeout"
            } else {
                "processing_failed"
            };
            tracing::error!(
                event = "shipment_process_failed",
                reason,
                elapsed_ms = started.elapsed().as_millis() as u64
            );
            if db
                .shipment_retry(&s, backoff(s.attempts), now())
                .await
                .is_err()
            {
                tracing::error!(event = "shipment_retry_failed", reason = "database");
            }
        } else {
            tracing::debug!(
                event = "shipment_processed",
                elapsed_ms = started.elapsed().as_millis() as u64
            );
        }
    }
    .instrument(span)
    .await
}
fn backoff(attempts: i32) -> i64 {
    (5_i64 << attempts.clamp(0, 7)).min(900)
}
async fn process(app: &Arc<App>, s: &Shipment) -> Result<(), Failure> {
    if super::shipment_action_worker::process(app, s).await? {
        return Ok(());
    }
    let db = app.db.as_ref().ok_or("missing shipment database")?;
    let api = app.shipping.as_ref().ok_or("missing shipment provider")?;
    match s.state.as_str() {
        "ordering" => {
            let command = match api.shipment_command(&s.command_uuid).await {
                Ok(c) => c,
                Err(ShipmentError::Unavailable | ShipmentError::Unknown) => {
                    db.shipment_retry(s, backoff(s.attempts), now()).await?;
                    return Ok(());
                }
                Err(_) => {
                    db.shipment_problem(s, "command_unknown", now()).await?;
                    tracing::warn!(event = "shipment_manual_review", reason = "command_unknown");
                    return Ok(());
                }
            };
            if command.state == "pending" {
                tracing::debug!(event = "shipment_booking_pending");
                db.shipment_retry(s, backoff(s.attempts), now()).await?;
                return Ok(());
            }
            if command.successful_ids.iter().any(|id| {
                !s.packages
                    .iter()
                    .any(|p| p.provider_id.as_ref() == Some(id))
            }) {
                db.shipment_problem(s, "command_mismatch", now()).await?;
                tracing::warn!(
                    event = "shipment_manual_review",
                    reason = "command_mismatch"
                );
                return Ok(());
            }
            let mut failed = false;
            for p in &s.packages {
                if p.state != "ordering" {
                    continue;
                }
                let success = p
                    .provider_id
                    .as_ref()
                    .is_some_and(|id| command.successful_ids.contains(id));
                failed |= !success;
                db.shipment_package_transition(
                    s,
                    &p.id,
                    "ordering",
                    if success { "booked" } else { "manual_review" },
                    None,
                    now(),
                )
                .await?;
            }
            if failed || command.state != "succeeded" {
                db.shipment_problem(s, "partial_or_failed_booking", now())
                    .await?;
                tracing::warn!(
                    event = "shipment_manual_review",
                    reason = "partial_or_failed_booking"
                );
            } else {
                db.shipment_state(s, "booked", 0, now()).await?;
                tracing::info!(event = "shipment_booked", consignments = s.packages.len());
            }
        }
        "queued" | "creating" => {
            if let Some(p) = s.packages.iter().find(|p| p.state == "creating") {
                match api.find_shipment_reference(&p.reference).await {
                    Ok(Some(actual))
                        if actual.state == "waiting" && matches_prepared(&actual, &p.payload) =>
                    {
                        db.shipment_package_transition(
                            s,
                            &p.id,
                            "creating",
                            "created",
                            Some(&actual.provider_id),
                            now(),
                        )
                        .await?;
                        db.shipment_state(s, "queued", 0, now()).await?;
                    }
                    Err(ShipmentError::Unavailable | ShipmentError::Unknown) if s.attempts < 10 => {
                        db.shipment_retry(s, backoff(s.attempts), now()).await?;
                    }
                    _ => {
                        db.shipment_problem(s, "package_unknown", now()).await?;
                        tracing::warn!(
                            event = "shipment_manual_review",
                            reason = "package_unknown"
                        );
                    }
                }
            } else if let Some(p) = s.packages.iter().find(|p| p.state == "pending") {
                if !db
                    .shipment_package_transition(s, &p.id, "pending", "creating", None, now())
                    .await?
                {
                    return Ok(());
                }
                match api.add_shipment(&p.payload).await {
                    Ok(actual) => {
                        db.shipment_package_transition(
                            s,
                            &p.id,
                            "creating",
                            "created",
                            Some(&actual.provider_id),
                            now(),
                        )
                        .await?;
                        db.shipment_state(s, "queued", 0, now()).await?;
                    }
                    Err(ShipmentError::Unknown) => {
                        tracing::warn!(
                            event = "shipment_creation_uncertain",
                            reason = "provider_unknown"
                        );
                        db.shipment_retry(s, 5, now()).await?;
                    }
                    Err(e) => {
                        db.shipment_problem(s, error_code(&e), now()).await?;
                        tracing::warn!(event = "shipment_manual_review", reason = error_code(&e));
                    }
                }
            } else if !s.packages.is_empty() && s.packages.iter().all(|p| p.state == "created") {
                // Reprice immediately before the paid command; drafts themselves buy nothing.
                let current = match current_quote(app, s).await {
                    Ok(q) => q,
                    Err(_) => {
                        db.shipment_problem(s, "price_unavailable", now()).await?;
                        tracing::warn!(
                            event = "shipment_manual_review",
                            reason = "price_unavailable"
                        );
                        return Ok(());
                    }
                };
                if s.quote.as_ref().and_then(|q| q["price_grosze"].as_u64())
                    != Some(u64::from(current.price_grosze))
                    || current.packages.len() != s.packages.len()
                    || !current
                        .packages
                        .iter()
                        .zip(&s.packages)
                        .all(|(new, old)| same_body(&new.body, &old.payload))
                {
                    db.shipment_problem(s, "price_or_data_changed", now())
                        .await?;
                    tracing::warn!(
                        event = "shipment_manual_review",
                        reason = "price_or_data_changed"
                    );
                    return Ok(());
                }
                if !db.shipment_order_start(s, now()).await? {
                    db.shipment_problem(s, "order_or_refund_changed", now())
                        .await?;
                    tracing::warn!(
                        event = "shipment_manual_review",
                        reason = "order_or_refund_changed"
                    );
                    return Ok(());
                }
                let ids = s
                    .packages
                    .iter()
                    .map(|p| p.provider_id.clone().ok_or("missing provider id"))
                    .collect::<Result<Vec<_>, _>>()?;
                match api.order_shipments(&s.command_uuid, &ids).await {
                    Ok(()) | Err(ShipmentError::Unknown) => {
                        tracing::debug!(event = "shipment_booking_reconcile_scheduled");
                        db.shipment_state(s, "ordering", 3, now()).await?;
                    }
                    Err(e) => {
                        db.shipment_problem(s, error_code(&e), now()).await?;
                        tracing::warn!(event = "shipment_manual_review", reason = error_code(&e));
                    }
                }
            } else {
                db.shipment_problem(s, "package_review", now()).await?;
                tracing::warn!(event = "shipment_manual_review", reason = "package_review");
            }
        }
        "booked" | "dispatched" | "manual_review" => poll(app, s).await?,
        _ => {}
    }
    Ok(())
}
fn error_code(e: &ShipmentError) -> &'static str {
    match e {
        ShipmentError::Unauthorized => "shipping_account",
        ShipmentError::Invalid(message) if message.contains("regulamin") => "provider_terms",
        ShipmentError::Invalid(_) => "provider_rejected",
        ShipmentError::NotFound => "provider_not_found",
        _ => "provider_unknown",
    }
}
fn same_body(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    let (mut a, mut b) = (a.clone(), b.clone());
    for value in [&mut a, &mut b] {
        if let Some(v) = value.as_object_mut() {
            v.remove("user_reference_number");
        }
    }
    a == b
}
fn start_poll(tasks: &mut JoinSet<PollResult>, app: &Arc<App>, p: ShipmentPackage) {
    let api = app.shipping.clone();
    tasks.spawn(
        async move {
            let result = async {
                let api = api.ok_or(ShipmentError::Unavailable)?;
                let id = p.provider_id.as_deref().ok_or(ShipmentError::NotFound)?;
                let actual = api.get_shipment(id).await?;
                if !matches_prepared(&actual, &p.payload) {
                    return Err(ShipmentError::Invalid(
                        "Nieprawidłowe powiązanie przesyłki.".into(),
                    ));
                }
                let tracking = match api.shipment_tracking(id).await {
                    Ok(t) => t,
                    Err(ShipmentError::NotFound) => rapidmaker_shipping::Tracking {
                        state: "awaiting_handover".into(),
                        numbers: actual.tracking_numbers.clone(),
                        parcels: actual
                            .tracking_numbers
                            .iter()
                            .map(|n| (n.clone(), "awaiting_handover".into()))
                            .collect(),
                    },
                    Err(e) => return Err(e),
                };
                if !tracking_complete(&actual.tracking_numbers, &tracking.numbers) {
                    return Err(ShipmentError::Unavailable);
                }
                Ok((actual, tracking))
            }
            .await;
            (p, result)
        }
        .in_current_span(),
    );
}
async fn poll(app: &Arc<App>, s: &Shipment) -> Result<(), Failure> {
    let db = app.db.as_ref().ok_or("missing shipment database")?;
    let mut pending = s
        .packages
        .iter()
        .filter(|p| matches!(p.state.as_str(), "booked" | "dispatched"))
        .cloned();
    let mut tasks = JoinSet::new();
    for p in pending.by_ref().take(4) {
        start_poll(&mut tasks, app, p);
    }
    let mut failed = false;
    let mut returned = false;
    while let Some(item) = tasks.join_next().await {
        let (p, result) = item?;
        match result {
            Ok((actual, tracking)) => {
                let numbers = if tracking.numbers.is_empty() {
                    actual.tracking_numbers
                } else {
                    tracking.numbers
                };
                let carrier = s.input["fulfillment"]["offer"]["name"]
                    .as_str()
                    .unwrap_or("Kurier");
                let data = json!({"state":tracking.state,"numbers":numbers,"carrier":carrier,"parcel_states":tracking.parcels});
                let dispatched = matches!(
                    tracking.state.as_str(),
                    "in_transit" | "delivered" | "returned"
                );
                db.shipment_tracking(
                    s,
                    &p.id,
                    &data,
                    dispatched,
                    tracking.state == "delivered",
                    now(),
                )
                .await?;
                if p.tracking["state"].as_str() != Some(tracking.state.as_str()) {
                    let state = match tracking.state.as_str() {
                        "awaiting_handover" => "awaiting_handover",
                        "in_transit" => "in_transit",
                        "delivered" => "delivered",
                        "exception" => "exception",
                        "returned" => "returned",
                        _ => "unknown",
                    };
                    tracing::info!(event = "shipment_tracking_changed", package_id = %p.id, state);
                }
                if tracking.state == "returned" {
                    returned = true;
                    let from = if dispatched {
                        "dispatched"
                    } else {
                        p.state.as_str()
                    };
                    db.shipment_package_transition(s, &p.id, from, "manual_review", None, now())
                        .await?;
                }
            }
            Err(e) => {
                tracing::warn!(event = "shipment_tracking_retry", package_id = %p.id, reason = error_code(&e));
                let _ = provider_error(e);
                failed = true;
            }
        }
        if let Some(next) = pending.next() {
            start_poll(&mut tasks, app, next);
        }
    }
    if returned {
        db.shipment_problem(s, "shipment_returned", now()).await?;
        tracing::warn!(
            event = "shipment_manual_review",
            reason = "shipment_returned"
        );
    } else {
        db.shipment_retry(s, if failed { backoff(s.attempts) } else { 900 }, now())
            .await?;
    }
    Ok(())
}
fn tracking_complete(expected: &[String], actual: &[String]) -> bool {
    let (mut expected, mut actual) = (expected.to_vec(), actual.to_vec());
    expected.sort();
    actual.sort();
    !expected.is_empty() && expected == actual
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requote_ignores_only_our_reference_not_dimensions_or_service() {
        let a = json!({"service_id":1,"user_reference_number":"a","parcels":[{"width":10}]});
        let mut b = a.clone();
        b["user_reference_number"] = "b".into();
        assert!(same_body(&a, &b));
        b["parcels"][0]["width"] = 11.into();
        assert!(!same_body(&a, &b));
    }
    #[test]
    fn incomplete_or_foreign_parcel_tracking_cannot_complete_a_consignment() {
        let both = vec!["a".into(), "b".into()];
        assert!(tracking_complete(&both, &["b".into(), "a".into()]));
        assert!(!tracking_complete(&both, &["a".into()]));
        assert!(!tracking_complete(&both, &["a".into(), "foreign".into()]));
    }
}
