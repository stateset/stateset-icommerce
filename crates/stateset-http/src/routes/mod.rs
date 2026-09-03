//! Route modules and router assembly.

pub mod a2a_credit;
pub mod a2a_messaging;
pub mod accounts_payable;
pub mod accounts_receivable;
pub mod activity_logs;
pub mod backorders;
pub mod bom;
pub mod carts;
pub mod channels;
pub mod companies;
pub mod currency;
pub mod customers;
pub mod edi_documents;
pub mod events;
pub mod fixed_assets;
pub mod fulfillment;
pub mod general_ledger;
pub mod gift_cards;
pub mod health;
pub mod inbound_shipments;
pub mod integration_field_mappings;
pub mod integration_mappings;
pub mod inventory;
pub mod invoices;
pub mod kernel;
pub mod lots;
pub mod loyalty;
pub mod negotiations;
pub mod orders;
pub mod payment_obligations;
pub mod payments;
pub mod prepayments;
pub mod price_levels;
pub mod price_schedules;
pub mod print_stations;
pub mod production_batches;
pub mod products;
pub mod promotions;
pub mod purchase_orders;
pub mod purgatory;
pub mod quality;
pub mod receiving;
pub mod reports;
pub mod returns;
pub mod revenue_recognition;
pub mod reviews;
pub mod segments;
pub mod serials;
pub mod shipments;
pub mod shipping_zones;
pub mod stock_snapshots;
pub mod store_credits;
pub mod subscriptions;
pub mod supplier_skus;
pub mod tax;
pub mod topology_snapshots;
pub mod transfer_orders;
pub mod units_of_measure;
pub mod vendor_credits;
pub mod vendor_returns;
pub mod warehouse;
pub mod warranties;
pub mod wishlists;
pub mod work_orders;
pub mod x402;

use axum::{Router, extract::DefaultBodyLimit, middleware::from_fn_with_state};
use std::time::Duration;
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;

use crate::idempotency::{IdempotencyLayer, idempotency};
use crate::state::AppState;

/// Default request timeout for all API endpoints (30 seconds).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Default maximum accepted request body size for extractor-based endpoints (1 `MiB`).
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
    api_router_with_idempotency(max_body_bytes, IdempotencyLayer::new())
}

/// Build the full API router with an explicit `IdempotencyLayer`.
///
/// Used by [`crate::server::ServerBuilder`] to wire a durable, database-backed
/// idempotency store and the required-key (HTTP 428) gate; the plain
/// [`api_router`] variants use an in-memory, optional-key layer.
#[allow(deprecated)]
pub fn api_router_with_idempotency(
    max_body_bytes: usize,
    idempotency_layer: IdempotencyLayer,
) -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1", v1_router())
        // Idempotency-Key handling for POST create endpoints. Applied before the
        // body limit so oversized bodies are still rejected by `DefaultBodyLimit`.
        .layer(from_fn_with_state(idempotency_layer, idempotency))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(CompressionLayer::new())
        .layer(TimeoutLayer::new(REQUEST_TIMEOUT))
        .layer(axum::middleware::from_fn(crate::middleware::track_http_metrics))
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
        .merge(shipping_zones::router())
        .merge(channels::router())
        .merge(companies::router())
        .merge(transfer_orders::router())
        .merge(units_of_measure::router())
        .merge(production_batches::router())
        .merge(supplier_skus::router())
        .merge(vendor_returns::router())
        .merge(vendor_credits::router())
        .merge(payment_obligations::router())
        .merge(price_levels::router())
        .merge(price_schedules::router())
        .merge(activity_logs::router())
        .merge(integration_mappings::router())
        .merge(integration_field_mappings::router())
        .merge(inbound_shipments::router())
        .merge(purgatory::router())
        .merge(print_stations::router())
        .merge(edi_documents::router())
        .merge(topology_snapshots::router())
        .merge(stock_snapshots::router())
        .merge(reports::router())
        .merge(kernel::router())
        .merge(x402::router())
        .merge(prepayments::router())
        .merge(purchase_orders::router())
        .merge(general_ledger::router())
        .merge(fixed_assets::router())
        .merge(revenue_recognition::router())
        .merge(accounts_payable::router())
        .merge(accounts_receivable::router())
        .merge(warehouse::router())
        .merge(fulfillment::router())
        .merge(receiving::router())
        .merge(work_orders::router())
        .merge(quality::router())
        .merge(bom::router())
        .merge(lots::router())
        .merge(serials::router())
        .merge(carts::router())
        .merge(tax::router())
        .merge(backorders::router())
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
