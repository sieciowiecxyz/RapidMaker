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

async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("signal handler");
    tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
}
