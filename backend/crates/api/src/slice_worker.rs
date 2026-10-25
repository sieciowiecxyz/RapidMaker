//! PostgreSQL leases fence workers; slicing never runs in an HTTP handler.
use super::quotes::SliceInput;
use rapidmaker_pricing::calculate;
use rapidmaker_storage::{SlicerJob, Storage};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::{sync::watch, task::JoinHandle};
use tracing::Instrument;

pub struct SliceWorker {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}
impl SliceWorker {
    pub fn start(db: Storage) -> Self {
        let (stop, mut receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut cleanup = tokio::time::Instant::now() - Duration::from_secs(60);
            loop {
                if *receiver.borrow() {
                    break;
                }
                if cleanup.elapsed() >= Duration::from_secs(60) {
                    if super::files::cleanup_models(&db).await.is_err() {
                        tracing::error!(event = "model_cleanup_failed", reason = "database");
                    }
                    cleanup = tokio::time::Instant::now();
                }
                match db.claim_slice(super::cart::now()).await {
                    Ok(Some(job)) => {
                        process(&db, job).await;
                        continue;
                    }
                    Ok(None) => {}
                    Err(_) => tracing::error!(event = "slicer_claim_failed", reason = "database"),
                }
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                    _ = receiver.changed() => break,
                }
            }
        });
        Self { stop, task }
    }
    pub async fn shutdown(self) {
        let _ = self.stop.send(true);
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(155), &mut task).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => tracing::error!(event = "slicer_worker_stopped", reason = "task_failure"),
            Err(_) => {
                task.abort();
                tracing::error!(event = "slicer_worker_stopped", reason = "shutdown_timeout");
            }
        }
    }
}
async fn process(db: &Storage, job: SlicerJob) {
    async {
        let Some(token) = job.lease_token.as_deref() else {
            tracing::error!(event = "slicer_invalid_lease", reason = "missing_lease");
            return;
        };
        tracing::info!(event = "slicer_started");
        let started = std::time::Instant::now();
        let outcome = tokio::time::timeout(Duration::from_secs(150), measure(db, &job))
            .await
            .unwrap_or(Err((
                true,
                "Przekroczono czas wyceny. Spróbuj ponownie lub poproś o wycenę indywidualną.",
            )));
        let state = match &outcome {
            Ok(_) => "completed",
            Err((true, _)) => "retry",
            Err((false, _)) => "rejected",
        };
        let result = match outcome {
            Ok((request, result)) => {
                db.finish_slice(&job.id, token, &request, &result, super::cart::now())
                    .await
            }
            Err((retry, message)) => {
                db.fail_slice(&job.id, token, retry, message, super::cart::now())
                    .await
            }
        };
        match result {
            Ok(true) if state == "completed" => tracing::info!(
                event = "slicer_recorded",
                outcome = state,
                elapsed_ms = started.elapsed().as_millis() as u64
            ),
            Ok(true) => tracing::warn!(
                event = "slicer_recorded",
                outcome = state,
                elapsed_ms = started.elapsed().as_millis() as u64
            ),
            Ok(false) => tracing::warn!(event = "slicer_lease_lost", reason = "lease_lost"),
            Err(_) => tracing::error!(event = "slicer_persist_failed", reason = "database"),
        }
    }
    .instrument(tracing::info_span!("slicer_job", job_id = %job.id, attempt = job.attempts))
    .await
}
type Failure = (bool, &'static str);
async fn measure(db: &Storage, job: &SlicerJob) -> Result<(Value, Value), Failure> {
    let config = (
        false,
        "Konfiguracja wyceny zmieniła się. Przelicz model ponownie.",
    );
    let unavailable = (
        true,
        "Wycena jest chwilowo niedostępna. Spróbuj ponownie lub poproś o wycenę indywidualną.",
    );
    let input: SliceInput = serde_json::from_value(job.payload.clone()).map_err(|_| config)?;
    input.rates.validate().map_err(|_| config)?;
    input.options.validate().map_err(|_| config)?;
    if input.slicer_profile_version != rapidmaker_slicer::PROFILE_VERSION {
        return Err(config);
    }
    let record = db
        .slice_model(
            &job.id,
            job.lease_token.as_deref().ok_or(config)?,
            super::cart::now(),
        )
        .await
        .map_err(|_| unavailable)?
        .ok_or((false, "Model wygasł lub jest niedostępny."))?;
    let model = super::files::handle(record).map_err(|_| config)?;
    if model.record.sha256 != input.model_sha256
        || rapidmaker_slicer::check_model_dimensions(model.record.dimensions_mm).is_err()
    {
        return Err(config);
    }
    let material = db
        .materials()
        .await
        .map_err(|_| unavailable)?
        .into_iter()
        .find(|m| m.id == input.material_id)
        .ok_or(config)?;
    if super::quotes::approved_cost(&material) != Some(input.purchase_net_grosze_per_kg)
        || material.source["profiles_sha256"] != input.profiles_sha256
        || !super::quotes::known_color(&material, &input.color_code)
    {
        return Err(config);
    }
    if let Some(current) = db.pricing_config().await.map_err(|_| unavailable)?
        && current != serde_json::to_value(&input.rates).map_err(|_| config)?
    {
        return Err(config);
    }
    let measured = rapidmaker_slicer::slice_job(&model.mesh, &input.material_id, &input.options)
        .await
        .map_err(|e| match e {
            rapidmaker_slicer::Error::Busy | rapidmaker_slicer::Error::Failed(_) => unavailable,
            _ => (
                false,
                "Nie udało się przygotować modelu. Sprawdź plik lub poproś o wycenę indywidualną.",
            ),
        })?;
    let price = calculate(
        measured.time_seconds,
        (measured.filament_grams * 1000.0).ceil() as u64,
        measured.plates,
        input.purchase_net_grosze_per_kg,
        &input.rates,
    )
    .map_err(|_| (false, "Ta konfiguracja wymaga indywidualnej wyceny."))?;
    let expires_at = super::cart::now() + 600;
    let request = json!({"owner_hash":job.owner_hash,"user_id":model.record.user_id,
        "model_id":job.model_id,"expires_at":expires_at,"dimensions_mm":model.record.dimensions_mm,
        "model_sha256":input.model_sha256,"material_id":input.material_id,"color_code":input.color_code,
        "quantity":input.options.quantity,"slicer_profile_version":input.slicer_profile_version,
        "options":input.options,"purchase_net_grosze_per_kg":input.purchase_net_grosze_per_kg,
        "rates":input.rates,"profiles_sha256":input.profiles_sha256});
    let result = json!({"id":job.id,"model_id":job.model_id,"expires_at":expires_at,
        "status":"estimated_poc","currency":"PLN","quantity":input.options.quantity,
        "measurement":measured,"plates":measured.plates,"batch_strategy":"arranged_plate_batches",
        "options":input.options,"price":price,"pricing_version":input.rates.version,"payable":false,
        "notice":"Wycena z pomiaru slicera. Dostawa i opakowanie rozliczane osobno."});
    Ok((request, result))
}
