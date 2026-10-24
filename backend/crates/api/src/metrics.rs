//! Bounded HTTP labels contain only server route templates and normalized method/status.
//! Durations cover response preparation, not streaming the response body.
use axum::{
    extract::{MatchedPath, Request, State},
    http::Method,
    middleware::Next,
    response::Response,
};
use prometheus_client::{
    encoding::{EncodeLabelSet, text::encode},
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::Registry,
};
use std::{sync::Arc, time::Instant};

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RouteLabels {
    method: &'static str,
    route: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct RequestLabels {
    method: &'static str,
    route: String,
    status_class: &'static str,
}

pub(super) struct Metrics {
    registry: Registry,
    started: Instant,
    uptime: Gauge,
    in_flight: Gauge,
    requests: Family<RequestLabels, Counter>,
    duration: Family<RouteLabels, Histogram, fn() -> Histogram>,
}

impl Metrics {
    pub(super) fn new() -> Self {
        let mut registry = Registry::default();
        let uptime = Gauge::default();
        let in_flight = Gauge::default();
        let requests = Family::<RequestLabels, Counter>::default();
        let duration =
            Family::<RouteLabels, Histogram, fn() -> Histogram>::new_with_constructor(|| {
                Histogram::new([0.005, 0.025, 0.1, 0.5, 1.0, 5.0, 15.0, 60.0])
            });
        registry.register(
            "rapidmaker_uptime_seconds",
            "Seconds since HTTP metrics initialization",
            uptime.clone(),
        );
        registry.register(
            "rapidmaker_http_in_flight",
            "Requests preparing a response",
            in_flight.clone(),
        );
        registry.register(
            "rapidmaker_http_requests",
            "Completed HTTP responses",
            requests.clone(),
        );
        registry.register(
            "rapidmaker_http_duration_seconds",
            "Response preparation duration in seconds",
            duration.clone(),
        );
        Self {
            registry,
            started: Instant::now(),
            uptime,
            in_flight,
            requests,
            duration,
        }
    }

    pub(super) fn render(&self) -> Result<String, std::fmt::Error> {
        self.uptime
            .set(self.started.elapsed().as_secs().min(i64::MAX as u64) as i64);
        let mut output = String::new();
        encode(&mut output, &self.registry)?;
        Ok(output)
    }
}

struct InFlight(Gauge);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub(super) async fn request(
    State(metrics): State<Arc<Metrics>>,
    request: Request,
    next: Next,
) -> Response {
    let labels = RouteLabels {
        route: request
            .extensions()
            .get::<MatchedPath>()
            .map_or("unmatched", MatchedPath::as_str)
            .to_owned(),
        method: match *request.method() {
            Method::GET => "GET",
            Method::HEAD => "HEAD",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::PATCH => "PATCH",
            Method::DELETE => "DELETE",
            Method::OPTIONS => "OPTIONS",
            Method::CONNECT => "CONNECT",
            Method::TRACE => "TRACE",
            _ => "OTHER",
        },
    };
    metrics.in_flight.inc();
    let _in_flight = InFlight(metrics.in_flight.clone());
    let start = Instant::now();
    let response = next.run(request).await;
    metrics
        .duration
        .get_or_create(&labels)
        .observe(start.elapsed().as_secs_f64());
    let status_class = match response.status().as_u16() {
        100..=199 => "1xx",
        200..=299 => "2xx",
        300..=399 => "3xx",
        400..=499 => "4xx",
        500..=599 => "5xx",
        _ => "other",
    };
    metrics
        .requests
        .get_or_create(&RequestLabels {
            method: labels.method,
            route: labels.route,
            status_class,
        })
        .inc();
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, middleware, routing::any};
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_use_bounded_labels_without_request_data() {
        let metrics = Arc::new(Metrics::new());
        let router = Router::new()
            .route("/models/{id}", any(|| async { "response_canary" }))
            .layer(middleware::from_fn_with_state(metrics.clone(), request));
        for (method, uri) in [
            ("GET", "/models/path_canary?secret=query_canary"),
            ("method_canary", "/models/another_canary"),
            ("GET", "/unmatched_canary"),
        ] {
            router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("authorization", "Bearer header_canary")
                        .header("cookie", "session=cookie_canary")
                        .body(Body::from("body_canary"))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
        let text = metrics.render().unwrap();
        assert!(!text.contains("canary"));
        assert!(text.contains("route=\"/models/{id}\""));
        assert!(text.contains("route=\"unmatched\""));
        assert!(text.contains("method=\"OTHER\""));
        assert!(text.contains("status_class=\"4xx\""));
        assert!(text.contains("rapidmaker_http_requests_total"));
        assert!(text.contains("rapidmaker_http_duration_seconds_count"));
        assert_eq!(metrics.in_flight.get(), 0);
        assert_eq!(
            metrics
                .requests
                .get_or_create(&RequestLabels {
                    method: "GET",
                    route: "/models/{id}".into(),
                    status_class: "2xx",
                })
                .get(),
            1
        );
    }

    #[tokio::test]
    async fn cancelled_requests_release_in_flight_gauge() {
        let metrics = Arc::new(Metrics::new());
        let started = Arc::new(tokio::sync::Notify::new());
        let handler_started = started.clone();
        let router = Router::new()
            .route(
                "/wait",
                any(move || async move {
                    handler_started.notify_one();
                    std::future::pending::<()>().await;
                }),
            )
            .layer(middleware::from_fn_with_state(metrics.clone(), request));
        let task = tokio::spawn(
            router.oneshot(Request::builder().uri("/wait").body(Body::empty()).unwrap()),
        );
        started.notified().await;
        assert_eq!(metrics.in_flight.get(), 1);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(metrics.in_flight.get(), 0);
    }
}
