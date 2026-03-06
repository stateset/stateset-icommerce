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

use crate::state::AppState;

/// Build the full API router with all route groups mounted.
///
/// This is the main entry point for route assembly. The returned [`Router`]
/// can be used directly or composed into a larger application.
pub fn api_router() -> Router<AppState> {
    Router::new().merge(health::router()).nest("/api/v1", v1_router())
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
