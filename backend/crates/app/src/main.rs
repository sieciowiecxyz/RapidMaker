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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:18432")
        .await
        .expect("bind localhost");
    let router = api::router().await.unwrap_or_else(|_| {
        eprintln!(
            "Cannot initialize database or pricing configuration; check local configuration."
        );
        std::process::exit(1);
    });
    axum::serve(listener, router).await.expect("serve");
}
