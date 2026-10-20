mod models;
mod providers;

fn main() {
    if models::worker() {
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
    let routes = rapidmaker_checkout_demo::router()
        .merge(providers::router())
        .merge(models::router());
    axum::serve(listener, providers::protect(routes))
        .await
        .expect("serve");
}
