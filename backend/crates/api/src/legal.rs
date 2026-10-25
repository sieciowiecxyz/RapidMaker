use axum::Json;
use rapidmaker_notifications::legal;
use serde_json::{Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) fn router() -> OpenApiRouter {
    OpenApiRouter::new().routes(routes!(documents))
}

pub(super) fn validate(version: &str) -> Result<(), super::auth::Error> {
    if version != legal::VERSION {
        return Err(super::auth::error(
            axum::http::StatusCode::CONFLICT,
            "Regulamin zmienił się lub nie został zaakceptowany. Odśwież stronę i przeczytaj aktualne warunki.",
        ));
    }
    Ok(())
}

#[utoipa::path(get, path="/api/legal/documents", tag="Serwis", operation_id="legal_documents", responses((status=200, body=Value)))]
async fn documents() -> Json<Value> {
    Json(
        json!({"version":legal::VERSION,"production_days":legal::PRODUCTION_DAYS,"terms":legal::TERMS,"privacy":legal::PRIVACY}),
    )
}
