use axum::{Json, Router, http::StatusCode, routing::post};
use rapidmaker_packing::{Estimate, EstimateRequest, estimate};

async fn estimate_handler(Json(request): Json<EstimateRequest>) -> (StatusCode, Json<Estimate>) {
    (StatusCode::OK, Json(estimate(request)))
}
pub fn router() -> Router {
    Router::new()
        .route("/api/shipping/estimate", post(estimate_handler))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}
