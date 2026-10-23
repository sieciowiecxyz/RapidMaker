use rapidmaker_api as api;

fn main() {
    if api::worker() {
        return;
    }
    tokio::runtime::Runtime::new()
        .expect("runtime")
        .block_on(serve());
}

async fn serve() {
    if let Some(mode) = std::env::args()
        .nth(1)
        .filter(|s| matches!(s.as_str(), "--auth-bootstrap" | "--auth-recover"))
    {
        let result = admin_grant(&mode).await;
        match result {
            Ok(secret) => println!("{secret}"),
            Err(_) => {
                eprintln!(
                    "Local admin grant refused; verify email login/session and database configuration."
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
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18432")
        .await
        .expect("bind localhost");
    let (router, worker) = api::router().await.unwrap_or_else(|_| {
        eprintln!(
            "Cannot initialize database, pricing or mail configuration; check local configuration."
        );
        std::process::exit(1);
    });
    let result = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await;
    if let Some(worker) = worker {
        worker.shutdown().await;
    }
    result.expect("serve");
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
}
