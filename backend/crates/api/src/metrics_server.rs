//! A separate loopback-only scrape listener; never merge this router into the public API.
use crate::metrics::Metrics;
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, oneshot};

struct Scrape {
    metrics: Arc<Metrics>,
    token_hash: [u8; 32],
    last: Mutex<Option<Instant>>,
}
impl Scrape {
    fn new(token: &str) -> Result<Arc<Self>, &'static str> {
        if !rapidmaker_auth::valid_token(token) {
            return Err("METRICS_TOKEN must contain 64 hexadecimal characters");
        }
        Ok(Arc::new(Self {
            metrics: Arc::new(Metrics::new()),
            token_hash: rapidmaker_auth::hash_token(token),
            last: Mutex::new(None),
        }))
    }
    fn router(self: Arc<Self>) -> Router {
        Router::new()
            .route("/metrics", get(scrape))
            .with_state(self)
    }
}
async fn scrape(State(state): State<Arc<Scrape>>, headers: HeaderMap, uri: Uri) -> Response {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    if !token
        .filter(|t| rapidmaker_auth::valid_token(t))
        .is_some_and(|t| {
            rapidmaker_auth::constant_eq(&rapidmaker_auth::hash_token(t), &state.token_hash)
        })
    {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer")],
        )
            .into_response();
    }
    if headers.contains_key(header::ORIGIN)
        || uri.query().is_some()
        || !headers
            .get(header::HOST)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| matches!(h, "localhost:18435" | "127.0.0.1:18435"))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    // At most one render per second, no queued concurrent scrapes and no database queries.
    let Ok(mut last) = state.last.try_lock() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    if last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    *last = Some(Instant::now());
    match state.metrics.render() {
        Ok(text) => (
            [
                (
                    header::CONTENT_TYPE,
                    "application/openmetrics-text; version=1.0.0; charset=utf-8",
                ),
                (header::CACHE_CONTROL, "no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            text,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub(super) struct MetricsServer {
    pub(super) metrics: Arc<Metrics>,
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}
impl MetricsServer {
    pub(super) async fn from_env() -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>>
    {
        let token = match std::env::var("METRICS_TOKEN") {
            Ok(token) => token,
            Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => return Err("Invalid METRICS_TOKEN encoding".into()),
        };
        let state = Scrape::new(&token)?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:18435").await?;
        let metrics = state.metrics.clone();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            tracing::info!(event = "metrics_listening", address = "127.0.0.1:18435");
            if axum::serve(listener, state.router())
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .is_err()
            {
                tracing::error!(event = "metrics_server_failed");
            }
        });
        Ok(Some(Self {
            metrics,
            stop,
            task,
        }))
    }
    pub(super) async fn shutdown(self) {
        let _ = self.stop.send(());
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
            Ok(Ok(())) => (),
            Ok(Err(_)) => tracing::error!(event = "metrics_task_failed"),
            Err(_) => {
                task.abort();
                tracing::warn!(event = "metrics_shutdown_timeout");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn scrape_requires_private_token_and_limits_rendering() {
        let token = "a".repeat(64);
        assert!(Scrape::new("").is_err());
        assert!(Scrape::new("token_canary").is_err());
        let router = Scrape::new(&token).unwrap().router();
        for (auth, host, origin, uri, status) in [
            (None, "localhost:18435", false, "/metrics", 401),
            (
                Some("Bearer bad".into()),
                "localhost:18435",
                false,
                "/metrics",
                401,
            ),
            (
                Some(format!("Bearer {}", "b".repeat(64))),
                "localhost:18435",
                false,
                "/metrics",
                401,
            ),
            (
                Some(format!("Bearer {token}")),
                "attacker.invalid",
                false,
                "/metrics",
                403,
            ),
            (
                Some(format!("Bearer {token}")),
                "localhost:18435",
                true,
                "/metrics",
                403,
            ),
            (
                Some(format!("Bearer {token}")),
                "localhost:18435",
                false,
                "/metrics?token=canary",
                403,
            ),
            (
                Some(format!("Bearer {token}")),
                "localhost:18435",
                false,
                "/metrics",
                200,
            ),
            (
                Some(format!("Bearer {token}")),
                "localhost:18435",
                false,
                "/metrics",
                429,
            ),
        ] {
            let mut request = Request::builder().uri(uri).header(header::HOST, host);
            if let Some(auth) = auth {
                request = request.header(header::AUTHORIZATION, auth);
            }
            if origin {
                request = request.header(header::ORIGIN, "http://attacker.invalid");
            }
            let response = router
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status);
            if status == 200 {
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                let text = String::from_utf8(
                    to_bytes(response.into_body(), 1_000_000)
                        .await
                        .unwrap()
                        .to_vec(),
                )
                .unwrap();
                assert!(text.contains("# EOF") && text.contains("rapidmaker_uptime_seconds"));
                assert!(!text.contains(&token));
            }
        }
    }
}
