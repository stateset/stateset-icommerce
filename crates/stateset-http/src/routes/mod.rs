//! Route modules and router assembly.

pub mod a2a_credit;
pub mod a2a_messaging;
pub mod currency;
pub mod customers;
pub mod events;
pub mod gift_cards;
pub mod health;
pub mod inventory;
pub mod invoices;
pub mod loyalty;
pub mod negotiations;
pub mod orders;
pub mod payments;
pub mod products;
pub mod promotions;
pub mod returns;
pub mod reviews;
pub mod segments;
pub mod shipments;
pub mod store_credits;
pub mod subscriptions;
pub mod warranties;
pub mod wishlists;

use axum::{Router, extract::DefaultBodyLimit};
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;

use crate::state::AppState;

/// Default request timeout for all API endpoints (30 seconds).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Default maximum accepted request body size for extractor-based endpoints (1 MiB).
pub const DEFAULT_REQUEST_BODY_LIMIT_BYTES: usize = 1024 * 1024;

/// Build the full API router with all route groups mounted.
///
/// This is the main entry point for route assembly. The returned [`Router`]
/// includes gzip response compression and a 30-second request timeout.
#[allow(deprecated)]
pub fn api_router() -> Router<AppState> {
    api_router_with_body_limit(DEFAULT_REQUEST_BODY_LIMIT_BYTES)
}

/// Build the full API router with a custom extractor body-size limit.
#[allow(deprecated)]
pub fn api_router_with_body_limit(max_body_bytes: usize) -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1", v1_router())
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(CompressionLayer::new())
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
        .merge(reviews::router())
        .merge(wishlists::router())
        .merge(gift_cards::router())
        .merge(loyalty::router())
        .merge(negotiations::router())
        .merge(a2a_messaging::router())
        .merge(a2a_credit::router())
        .merge(subscriptions::router())
        .merge(store_credits::router())
        .merge(promotions::router())
        .merge(currency::router())
        .merge(warranties::router())
        .merge(segments::router())
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
    fn api_router_with_body_limit_builds() {
        let _router: Router<AppState> = api_router_with_body_limit(1024);
    }

    #[test]
    fn v1_router_builds() {
        let _router: Router<AppState> = v1_router();
    }
}
