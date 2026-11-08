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
mod legal;
mod logging;
mod mail_inbound;
mod manual_quotes;
mod metrics;
mod metrics_server;
mod model_limits;
mod models;
pub use mail_inbound::receive_once as mail_receive_once;
mod orders;
mod packing;
mod passkeys;
mod production;
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
    model_limits::ModelProfile::from_env()?;
    Ok(SliceWorker::start(
        rapidmaker_storage::Storage::connect(&std::env::var("DATABASE_URL")?).await?,
    ))
}
pub async fn router() -> Result<(axum::Router, Workers), Box<dyn std::error::Error + Send + Sync>> {
    let model_profile = model_limits::ModelProfile::from_env()?;
    let mailer = std::sync::Arc::new(rapidmaker_notifications::Mailer::from_env()?);
    let storage = if let Ok(url) = std::env::var("DATABASE_URL") {
        let storage = rapidmaker_storage::Storage::connect(&url).await?;
        storage.import_bambu().await?;
        if std::env::var("AUTH_MODE").as_deref() == Ok("production")
            && storage
                .pricing_config()
                .await?
                .is_some_and(|v| v["version"].as_str().is_some_and(|s| s.starts_with("poc-")))
        {
            return Err(
                "POC pricing cannot run in production; configure verified costs and rates".into(),
            );
        }
        Some(storage)
    } else {
        None
    };
    // Complete the cold bundle hash before serving quotes; other APIs can run without a slicer.
    if storage.is_some() && rapidmaker_slicer::runtime_sha256().await.is_err() {
        tracing::warn!(
            event = "slicer_runtime_unavailable",
            reason = "bundle_missing_or_changed"
        );
    }
    let imap = rapidmaker_notifications::Imap::from_env()?;
    let auth = auth::Auth::new(storage.clone(), mailer.clone())?;
    let (checkout, webhook, checkout_worker, shipping_worker) = checkout::router(storage.clone())?;
    let api = utoipa_axum::router::OpenApiRouter::new()
        .merge(legal::router())
        .merge(packing::router())
        .merge(checkout)
        .merge(models::router(model_profile))
        .merge(quotes::router(storage.clone())?)
        .merge(auth::router(auth.clone()))
        .merge(account_security::router(auth.clone()))
        .merge(passkeys::router(auth.clone()))
        .merge(orders::router(auth.clone()))
        .merge(production::router(auth.clone()));
    let (router, mut document) = api.split_for_parts();
    let (webhook, webhook_document) = webhook.split_for_parts();
    document.merge(webhook_document);
    let metrics = metrics_server::MetricsServer::from_env().await?;
    let mut router = checkout::protect(router.merge(docs::router(document)), auth.clone())
        .merge(webhook)
        .layer(axum::Extension(mailer.clone()))
        .layer(axum::Extension(auth))
        .layer(axum::middleware::from_fn(logging::request))
        .layer(tower_http::timeout::RequestBodyTimeoutLayer::new(
            std::time::Duration::from_secs(30),
        ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test(start_paused = true)]
    async fn idle_json_and_webhook_bodies_do_not_wait_forever() {
        assert!(
            std::env::var_os("DATABASE_URL").is_none(),
            "This test must not use a database"
        );
        let (app, workers) = router().await.unwrap();
        let request = |path: &str, body: Body| {
            Request::post(path)
                .extension(axum::extract::ConnectInfo(
                    "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
                .header("host", "localhost:18430")
                .header("origin", "http://localhost:18430")
                .header("content-type", "application/json")
                .body(body)
                .unwrap()
        };
        assert_eq!(
            app.clone()
                .oneshot(request(
                    "/api/shipping/estimate",
                    Body::from(r#"{"parts":[],"delivery":"pickup"}"#)
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        for path in [
            "/api/shipping/estimate",
            "/api/auth/password/login",
            "/api/webhooks/stripe",
        ] {
            let body = Body::from_stream(futures_util::stream::pending::<
                Result<axum::body::Bytes, std::io::Error>,
            >());
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(31),
                app.clone().oneshot(request(path, body)),
            )
            .await
            .expect("An idle body must not retain its request indefinitely")
            .unwrap();
            assert!(
                response.status().is_client_error(),
                "{path}: {}",
                response.status()
            );
        }
        use futures_util::StreamExt;
        let progress =
            futures_util::stream::iter(["{", "\"parts\":[],", "\"delivery\":\"pickup\"}"]).then(
                |chunk| async move {
                    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                    Ok::<_, std::io::Error>(axum::body::Bytes::from_static(chunk.as_bytes()))
                },
            );
        let response = app
            .oneshot(request(
                "/api/shipping/estimate",
                Body::from_stream(progress),
            ))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Progressing bodies must not get a whole-handler timeout"
        );
        workers.shutdown().await;
    }
}
