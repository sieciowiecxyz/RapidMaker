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
    let router = api::router().await.unwrap_or_else(|_| {
        eprintln!(
            "Cannot initialize database, pricing or mail configuration; check local configuration."
        );
        std::process::exit(1);
    });
    axum::serve(listener, router).await.expect("serve");
}
