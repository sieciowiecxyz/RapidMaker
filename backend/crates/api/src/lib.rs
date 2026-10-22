mod checkout;
mod models;
mod packing;
mod quotes;

pub use models::worker;

pub async fn router() -> Result<axum::Router, Box<dyn std::error::Error + Send + Sync>> {
    Ok(checkout::protect(
        packing::router()
            .merge(checkout::router())
            .merge(models::router())
            .merge(quotes::router().await?),
    ))
}
