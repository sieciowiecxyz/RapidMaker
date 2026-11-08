use super::checkout::{Error, Problem, Result, error, unavailable};
use axum::{Json, http::StatusCode};
use rapidmaker_packing::{Estimate, EstimateRequest, estimate};
use tokio::sync::Semaphore;
use utoipa_axum::{router::OpenApiRouter as Router, routes};

static BUSY: Semaphore = Semaphore::const_new(2);
// Public estimates cannot occupy both slots needed by owned checkout plans.
static PUBLIC: Semaphore = Semaphore::const_new(1);

fn busy() -> Error {
    error(
        StatusCode::TOO_MANY_REQUESTS,
        "packing_busy",
        "Trwa przeliczanie dostawy. Spróbuj ponownie za chwilę.",
    )
}

pub(super) async fn compute<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T> {
    let permit = BUSY.try_acquire().map_err(|_| busy())?;
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        span.in_scope(work)
    })
    .await
    .map_err(|_| unavailable())
}

#[utoipa::path(operation_id = "packing_estimate_handler", post, path = "/api/shipping/estimate", tag = "Dostawa",
    request_body = EstimateRequest, responses((status = 200, description = "Szacowany plan pakowania", body = Estimate), (status = 429, description = "Pakowanie jest zajęte", body = Problem), (status = 503, body = Problem)))]
async fn estimate_handler(Json(request): Json<EstimateRequest>) -> Result<Json<Estimate>> {
    let public = PUBLIC.try_acquire().map_err(|_| busy())?;
    Ok(Json(
        compute(move || {
            let _public = public;
            estimate(request)
        })
        .await?,
    ))
}
pub fn router() -> Router {
    Router::new()
        .routes(routes!(estimate_handler))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn packing_keeps_reactor_free_and_cancelled_work_keeps_its_slot() {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (pulse, heartbeat) = std::sync::mpsc::channel();
        let (release, held) = std::sync::mpsc::channel();
        let (finished, done) = tokio::sync::oneshot::channel();
        let watchdog = std::thread::spawn(move || {
            let responsive = heartbeat
                .recv_timeout(std::time::Duration::from_secs(1))
                .is_ok();
            release.send(()).unwrap();
            responsive
        });
        let job = tokio::spawn(compute(move || {
            started.send(()).unwrap();
            held.recv().unwrap();
            finished.send(()).unwrap();
        }));
        ready.await.unwrap();
        job.abort();
        let _ = job.await; // Observe cancellation before checking the still-running worker.
        let retained = BUSY.available_permits() < 2;
        let _ = pulse.send(());
        let responsive = watchdog.join().unwrap();
        done.await.unwrap();
        assert!(
            responsive,
            "Packing blocked the Tokio reactor until the watchdog released it"
        );
        assert!(
            retained,
            "Cancellation released a still-running packing job's slot"
        );
    }
}
