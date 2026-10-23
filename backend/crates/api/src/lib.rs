mod admin;
mod auth;
mod cart;
mod checkout;
mod docs;
mod email;
mod files;
mod models;
mod orders;
mod packing;
mod passkeys;
mod quotes;
mod slice_worker;

pub use models::worker;

pub use slice_worker::SliceWorker;
pub struct Workers {
    mail: Option<email::MailWorker>,
    slicer: Option<SliceWorker>,
}
impl Workers {
    pub async fn shutdown(self) {
        tokio::join!(
            async {
                if let Some(worker) = self.mail {
                    worker.shutdown().await;
                }
            },
            async {
                if let Some(worker) = self.slicer {
                    worker.shutdown().await;
                }
            }
        );
    }
}
pub async fn slicer_worker() -> Result<SliceWorker, Box<dyn std::error::Error + Send + Sync>> {
    Ok(SliceWorker::start(
        rapidmaker_storage::Storage::connect(&std::env::var("DATABASE_URL")?).await?,
    ))
}
pub async fn router() -> Result<(axum::Router, Workers), Box<dyn std::error::Error + Send + Sync>> {
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
            .merge(docs::router())
            .merge(checkout::router(storage.clone()))
            .merge(models::router())
            .merge(quotes::router(storage.clone())?)
            .merge(auth::router(auth.clone()))
            .merge(passkeys::router(auth.clone()))
            .merge(orders::router(auth.clone())),
        auth.clone(),
    )
    .layer(axum::Extension(mailer.clone()))
    .layer(axum::Extension(auth));
    let worker = storage
        .as_ref()
        .filter(|_| mailer.enabled())
        .map(|db| email::MailWorker::start(db.clone(), mailer));
    let slicer = match std::env::var("SLICER_WORKER_MODE")
        .as_deref()
        .unwrap_or("embedded")
    {
        "embedded" => storage.map(SliceWorker::start),
        "external" => None,
        _ => return Err("SLICER_WORKER_MODE must be embedded or external".into()),
    };
    Ok((
        router,
        Workers {
            mail: worker,
            slicer,
        },
    ))
}
