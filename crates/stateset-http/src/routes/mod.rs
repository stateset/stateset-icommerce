//! Route modules and router assembly.

pub mod customers;
pub mod events;
pub mod health;
pub mod inventory;
pub mod invoices;
pub mod orders;
pub mod payments;
pub mod products;
pub mod returns;
pub mod shipments;

use axum::Router;
use std::time::Duration;
use tower_http::timeout::TimeoutLayer;

use crate::state::AppState;

/// Default request timeout for all API endpoints (30 seconds).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build the full API router with all route groups mounted.
///
/// This is the main entry point for route assembly. The returned [`Router`]
/// includes a 30-second request timeout on all API endpoints.
pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1", v1_router())
        .layer(TimeoutLayer::new(REQUEST_TIMEOUT))
}

/// Build the v1 API sub-router.
fn v1_router() -> Router<AppState> {
    Router::new()
        .merge(orders::router())
        .merge(customers::router())
        .merge(products::router())
        .merge(inventory::router())
        .merge(returns::router())
        .merge(shipments::router())
        .merge(payments::router())
        .merge(invoices::router())
        .merge(events::router())
        .merge(crate::openapi::router())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_router_builds() {
        let _router: Router<AppState> = api_router();
    }

    #[test]
    fn v1_router_builds() {
        let _router: Router<AppState> = v1_router();
    }
}
