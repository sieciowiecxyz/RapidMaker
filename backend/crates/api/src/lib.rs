mod admin;
mod auth;
mod cart;
mod checkout;
mod email;
mod files;
mod models;
mod orders;
mod packing;
mod quotes;

pub use models::worker;

pub use email::MailWorker;
pub async fn router()
-> Result<(axum::Router, Option<MailWorker>), Box<dyn std::error::Error + Send + Sync>> {
    let mailer = std::sync::Arc::new(rapidmaker_notifications::Mailer::from_env()?);
    let storage = if let Ok(url) = std::env::var("DATABASE_URL") {
        let storage = rapidmaker_storage::Storage::connect(&url).await?;
        storage.import_bambu().await?;
        Some(storage)
    } else {
        None
    };
    let auth = auth::Auth::new(storage.clone(), mailer.clone())?;
    let router = checkout::protect(
        packing::router()
            .merge(checkout::router(storage.clone()))
            .merge(models::router())
            .merge(quotes::router(storage.clone())?)
            .merge(auth::router(auth.clone()))
            .merge(orders::router(auth)),
    )
    .layer(axum::Extension(mailer.clone()));
    let worker = storage
        .as_ref()
        .filter(|_| mailer.enabled())
        .map(|db| MailWorker::start(db.clone(), mailer));
    Ok((router, worker))
}
