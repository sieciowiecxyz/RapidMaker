mod admin;
mod cart;
mod checkout;
mod files;
mod models;
mod packing;
mod quotes;

pub use models::worker;

pub async fn router() -> Result<axum::Router, Box<dyn std::error::Error + Send + Sync>> {
    let storage = if let Ok(url) = std::env::var("DATABASE_URL") {
        let storage = rapidmaker_storage::Storage::connect(&url).await?;
        storage.import_bambu().await?;
        Some(storage)
    } else {
        None
    };
    Ok(checkout::protect(
        packing::router()
            .merge(checkout::router(storage.clone()))
            .merge(models::router())
            .merge(quotes::router(storage)?),
    ))
}
