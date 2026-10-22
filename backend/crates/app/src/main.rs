mod api;

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
    axum::serve(listener, api::router()).await.expect("serve");
}
