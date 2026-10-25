mod account_security;
mod admin;
mod auth;
mod cart;
mod checkout;
mod checkout_legacy;
mod checkout_worker;
mod docs;
mod email;
mod files;
mod logging;
mod mail_inbound;
mod manual_quotes;
mod metrics;
mod metrics_server;
mod models;
pub use mail_inbound::receive_once as mail_receive_once;
mod orders;
mod packing;
mod passkeys;
mod quotes;
mod refunds;
mod shipment_action_worker;
mod shipment_actions;
mod shipment_worker;
mod shipments;
mod slice_worker;

pub use checkout_worker::operator_status as checkout_operator;
pub use models::worker;

pub use slice_worker::SliceWorker;
pub struct Workers {
    metrics: Option<metrics_server::MetricsServer>,
    mail: Option<email::MailWorker>,
    inbound: Option<mail_inbound::InboundWorker>,
    slicer: Option<SliceWorker>,
    checkout: Option<checkout_worker::CheckoutWorker>,
    shipping: Option<shipment_worker::ShipmentWorker>,
}
impl Workers {
    pub async fn shutdown(self) {
        tokio::join!(
            async {
                if let Some(server) = self.metrics {
                    server.shutdown().await;
                }
            },
            async {
                if let Some(worker) = self.shipping {
                    worker.shutdown().await;
                }
            },
            async {
                if let Some(worker) = self.inbound {
                    worker.shutdown().await;
                }
            },
            async {
                if let Some(worker) = self.mail {
                    worker.shutdown().await;
                }
            },
            async {
                if let Some(worker) = self.checkout {
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
    let imap = rapidmaker_notifications::Imap::from_env()?;
    let auth = auth::Auth::new(storage.clone(), mailer.clone())?;
    let (checkout, webhook, checkout_worker, shipping_worker) = checkout::router(storage.clone())?;
    let api = utoipa_axum::router::OpenApiRouter::new()
        .merge(packing::router())
        .merge(checkout)
        .merge(models::router())
        .merge(quotes::router(storage.clone())?)
        .merge(auth::router(auth.clone()))
        .merge(account_security::router(auth.clone()))
        .merge(passkeys::router(auth.clone()))
        .merge(orders::router(auth.clone()));
    let (router, mut document) = api.split_for_parts();
    let (webhook, webhook_document) = webhook.split_for_parts();
    document.merge(webhook_document);
    let metrics = metrics_server::MetricsServer::from_env().await?;
    let mut router = checkout::protect(router.merge(docs::router(document)), auth.clone())
        .merge(webhook)
        .layer(axum::Extension(mailer.clone()))
        .layer(axum::Extension(auth))
        .layer(axum::middleware::from_fn(logging::request));
    if let Some(server) = &metrics {
        router = router.layer(axum::middleware::from_fn_with_state(
            server.metrics.clone(),
            metrics::request,
        ));
    }
    let worker = storage
        .as_ref()
        .filter(|_| mailer.enabled())
        .map(|db| email::MailWorker::start(db.clone(), mailer));
    let inbound = imap
        .map(|imap| {
            storage
                .clone()
                .ok_or("IMAP requires database")
                .map(|db| mail_inbound::InboundWorker::start(db, imap))
        })
        .transpose()?;
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
            metrics,
            mail: worker,
            inbound,
            slicer,
            checkout: checkout_worker,
            shipping: shipping_worker,
        },
    ))
}
