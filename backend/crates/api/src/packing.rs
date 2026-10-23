use axum::{Json, http::StatusCode};
use rapidmaker_packing::{Estimate, EstimateRequest, estimate};
use utoipa_axum::{router::OpenApiRouter as Router, routes};

#[utoipa::path(operation_id = "packing_estimate_handler", post, path = "/api/shipping/estimate", tag = "Dostawa",
    request_body = EstimateRequest, responses((status = 200, description = "Szacowany plan pakowania", body = Estimate)))]
async fn estimate_handler(Json(request): Json<EstimateRequest>) -> (StatusCode, Json<Estimate>) {
    (StatusCode::OK, Json(estimate(request)))
}
pub fn router() -> Router {
    Router::new()
        .routes(routes!(estimate_handler))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}
