use rapidmaker_api as api;

fn main() {
    if rapidmaker_observability::init().is_err() {
        eprintln!(
            r#"{{"level":"ERROR","fields":{{"event":"logging_init_failed","reason":"check_LOG_LEVEL_or_subscriber"}}}}"#
        );
        std::process::exit(1);
    }
    if api::worker() {
        return;
    }
    tracing::info!(
        event = "application_starting",
        version = env!("CARGO_PKG_VERSION")
    );
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(serve());
}

async fn serve() {
    if std::env::args().nth(1).as_deref() == Some("--mail-receive") {
        match api::mail_receive_once().await {
            Ok(value) => println!("{value}"),
            Err(_) => {
                tracing::error!(
                    event = "mail_receive_failed",
                    reason = "configuration_or_transport"
                );
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--mail-operator") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let result = async {
            if args.len() != 2 || !matches!(args[1].as_str(), "on" | "off") {
                return Err("Provide admin email and on/off".into());
            }
            let db = rapidmaker_storage::Storage::connect(&std::env::var("DATABASE_URL")?).await?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64;
            if !db
                .chat_operator_email(&args[0], args[1] == "on", now)
                .await?
            {
                return Err("No eligible verified administrator".into());
            }
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        }
        .await;
        match result {
            Ok(()) => println!("Operator email reply permission updated."),
            Err(_) => {
                tracing::error!(
                    event = "operator_permission_refused",
                    reason = "arguments_or_eligibility"
                );
                std::process::exit(1);
            }
        }
        return;
    }
    if let Some(mode) = std::env::args()
        .nth(1)
        .filter(|m| matches!(m.as_str(), "--checkout-status" | "--checkout-close"))
    {
        let args: Vec<String> = std::env::args().skip(2).collect();
        if args.is_empty() || args.len() > 2 {
            tracing::error!(event = "checkout_cli_refused", reason = "arguments");
            std::process::exit(1);
        }
        match api::checkout_operator(
            &args[0],
            args.get(1).map(String::as_str),
            mode == "--checkout-close",
        )
        .await
        {
            Ok(value) => println!("{value}"),
            Err(_) => {
                tracing::error!(
                    event = "checkout_recovery_refused",
                    reason = "stored_attempt_or_provider_reference"
                );
                std::process::exit(1);
            }
        }
        return;
    }

    if std::env::args().nth(1).as_deref() == Some("--slicer-worker") {
        let worker = api::slicer_worker().await.unwrap_or_else(|_| {
            tracing::error!(
                event = "slicer_worker_init_failed",
                reason = "configuration_or_database"
            );
            std::process::exit(1);
        });
        shutdown().await;
        worker.shutdown().await;
        return;
    }
    if let Some(mode) = std::env::args()
        .nth(1)
        .filter(|s| matches!(s.as_str(), "--auth-bootstrap" | "--auth-recover"))
    {
        let result = admin_grant(&mode).await;
        match result {
            Ok(secret) => println!("{secret}"),
            Err(_) => {
                tracing::error!(
                    event = "admin_grant_refused",
                    reason = "eligibility_or_configuration"
                );
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--mail-test") {
        let result = match rapidmaker_notifications::Mailer::from_env() {
            Ok(mailer) => mailer.send_test().await,
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                println!("SMTP accepted the test message; check the inbox to confirm delivery.")
            }
            Err(error) => {
                tracing::error!(event="mail_test_failed", reason=%error);
                std::process::exit(1);
            }
        }
        return;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18432")
        .await
        .unwrap_or_else(|_| {
            tracing::error!(event = "server_bind_failed", reason = "localhost_listener");
            std::process::exit(1);
        });
    let (router, worker) = api::router().await.unwrap_or_else(|_| {
        tracing::error!(
            event = "server_init_failed",
            reason = "database_pricing_or_mail_configuration"
        );
        std::process::exit(1);
    });
    tracing::info!(event = "server_listening", address = "127.0.0.1:18432");
    let result = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await;
    worker.shutdown().await;
    if result.is_err() {
        tracing::error!(event = "server_stopped", reason = "serve_failed");
        std::process::exit(1);
    }
    tracing::info!(event = "server_stopped", reason = "shutdown");
}

async fn admin_grant(mode: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let email = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().len() == 3)
        .ok_or("provide exactly one verified email")?
        .trim()
        .to_lowercase();
    if !email.is_ascii() || !rapidmaker_notifications::valid_address(&email) {
        return Err("invalid email".into());
    }
    let db = rapidmaker_storage::Storage::connect(&std::env::var("DATABASE_URL")?).await?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    let grant = if mode == "--auth-bootstrap" {
        db.grant_admin(&email, now).await?
    } else {
        db.recover_admin(&email, now).await?
    };
    grant.ok_or_else(|| "no eligible verified session or invalid admin state".into())
}

async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("signal handler");
    tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
    tracing::info!(event = "shutdown_requested");
}
