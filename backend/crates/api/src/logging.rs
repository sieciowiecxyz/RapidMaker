//! Log route templates, never raw URLs, request headers, bodies or response contents.
use axum::{
    extract::{MatchedPath, Request},
    http::{HeaderValue, Method},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::Instrument;

pub(super) async fn request(request: Request, next: Next) -> Response {
    // Always generate a server ID: caller-controlled IDs can contain secrets or forged context.
    let request_id = uuid::Uuid::new_v4().to_string();
    let header = HeaderValue::from_str(&request_id).expect("generated UUID is ASCII");
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or("unmatched", MatchedPath::as_str)
        .to_owned();
    let method = match *request.method() {
        Method::GET => "GET",
        Method::HEAD => "HEAD",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    };
    let span = tracing::info_span!("http", %request_id, method, route);
    async move {
        let start = Instant::now();
        let mut response = next.run(request).await;
        response.headers_mut().insert("x-request-id", header);
        let status = response.status().as_u16();
        let elapsed_ms = start.elapsed().as_millis() as u64;
        if status >= 500 {
            tracing::error!(event = "http_completed", status, elapsed_ms);
        } else if status >= 400 {
            tracing::warn!(event = "http_completed", status, elapsed_ms);
        } else if matches!(method, "GET" | "HEAD" | "OPTIONS") {
            tracing::debug!(event = "http_completed", status, elapsed_ms);
        } else {
            tracing::info!(event = "http_completed", status, elapsed_ms);
        }
        response
    }
    .instrument(span)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, middleware, routing::get};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;
    use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt};

    #[derive(Clone)]
    struct Output(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl<'a> MakeWriter<'a> for Output {
        type Writer = Self;
        fn make_writer(&'a self) -> Self {
            self.clone()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logs_are_correlated_json_without_request_or_response_secrets() {
        let out = Output(Arc::default());
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(out.clone()),
        );
        let _default = tracing::subscriber::set_default(subscriber);
        let app = Router::new()
            .route("/private/{id}", get(|| async { "response_canary" }))
            .layer(middleware::from_fn(request));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/private/path_canary?secret=query_canary")
                    .header("authorization", "Bearer authorization_canary")
                    .header("cookie", "session=cookie_canary")
                    .header("x-request-id", "caller_canary")
                    .body(Body::from("body_canary"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let id = response.headers()["x-request-id"].to_str().unwrap();
        assert!(uuid::Uuid::parse_str(id).is_ok());
        let output = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
        assert!(output.contains(id) && output.contains("/private/{id}"));
        for canary in [
            "path_canary",
            "query_canary",
            "authorization_canary",
            "cookie_canary",
            "caller_canary",
            "body_canary",
            "response_canary",
        ] {
            assert!(!output.contains(canary), "leaked {canary}");
        }
        let event: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(event["fields"]["status"], 200);
        assert!(event["fields"]["elapsed_ms"].is_number());
    }
}
