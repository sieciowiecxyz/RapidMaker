mod checkout;
mod models;
mod packing;

pub use models::worker;

pub fn router() -> axum::Router {
    checkout::protect(
        packing::router()
            .merge(checkout::router())
            .merge(models::router()),
    )
}
