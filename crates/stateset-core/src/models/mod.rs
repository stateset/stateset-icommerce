//! Domain models for commerce operations

pub mod a2a;
pub mod a2a_skill;
pub mod accounts_payable;
pub mod accounts_receivable;
pub mod activity_log;
pub mod agent_card;
pub mod analytics;
pub mod backorder;
pub mod cart;
pub mod channel;
pub mod close_books_report;
pub mod cogs_report;
pub mod company;
pub mod cost_accounting;
pub mod credit;
pub mod currency;
pub mod custom_object;
pub mod customer;
pub mod edi_document;
pub mod erc8004;
pub mod forecasting;
pub mod fraud;
pub mod fulfillment;
pub mod general_ledger;
pub mod gift_card;
pub mod inbound_shipment;
pub mod integration_field_mapping;
pub mod integration_mapping;
pub mod inventory;
pub mod inventory_aging;
pub mod invoice;
pub mod lot;
pub mod loyalty;
pub mod manufacturing;
pub mod metrics;
pub mod order;
pub mod payment;
pub mod payment_obligation;
pub mod prepayment;
pub mod price_level;
pub mod price_schedule;
pub mod print_station;
pub mod product;
pub mod production_batch;
pub mod promotion;
pub mod purchase_order;
pub mod purgatory;
pub mod quality;
pub mod receiving;
pub mod returns;
pub mod review;
pub mod sales_report;
pub mod search_config;
pub mod segment;
pub mod serial;
pub mod shipment;
pub mod shipping_zone;
pub mod stock_snapshot;
pub mod store_credit;
pub mod subscription;
pub mod supplier_sku;
pub mod tax;
pub mod topology_snapshot;
pub mod transfer_order;
pub mod unit_of_measure;
pub mod vector;
pub mod vendor_credit;
pub mod vendor_return;
pub mod warehouse;
pub mod warranty;
pub mod wishlist;
pub mod x402;

pub use a2a::*;
// Re-export a2a_skill items individually to avoid name collisions with the a2a
// module (both define `A2AQuote` and `A2AQuoteFilter` with different schemas).
// Use the module path `a2a_skill::A2AQuote` when you need the skill-commerce variant.
pub use a2a_skill::{
    A2APurchase, A2APurchaseFilter, A2AQuote as SkillQuote, A2AQuoteFilter as SkillQuoteFilter,
    ConfirmDeliveryInput, ConfirmDeliveryOutput, CreateA2APurchase, CreateA2AQuote,
    DiscoverSellersInput, DiscoverSellersOutput, InitiatePurchaseInput, InitiatePurchaseOutput,
    ItemAvailability, PurchaseStatus, QuoteItem, QuoteStatus, QuotedItem, RequestQuoteInput,
    RequestQuoteOutput, SellerInfo,
};
pub use accounts_payable::*;
pub use accounts_receivable::*;
pub use activity_log::*;
pub use agent_card::*;
pub use analytics::*;
pub use backorder::*;
pub use cart::*;
pub use channel::*;
pub use close_books_report::*;
pub use cogs_report::*;
pub use company::*;
pub use cost_accounting::*;
pub use credit::*;
pub use currency::*;
pub use custom_object::*;
pub use customer::*;
pub use edi_document::*;
pub use erc8004::*;
pub use forecasting::*;
pub use fraud::*;
pub use fulfillment::*;
pub use general_ledger::*;
pub use gift_card::*;
pub use inbound_shipment::*;
pub use integration_field_mapping::*;
pub use integration_mapping::*;
pub use inventory::*;
pub use inventory_aging::*;
pub use invoice::*;
pub use lot::*;
pub use loyalty::*;
pub use manufacturing::*;
pub use metrics::*;
pub use order::*;
pub use payment::*;
pub use payment_obligation::*;
pub use prepayment::*;
pub use price_level::*;
pub use price_schedule::*;
pub use print_station::*;
pub use product::*;
pub use production_batch::*;
pub use promotion::*;
pub use purchase_order::*;
pub use purgatory::*;
pub use quality::*;
pub use receiving::*;
pub use returns::*;
pub use review::*;
pub use sales_report::*;
pub use search_config::*;
pub use segment::*;
pub use serial::*;
pub use shipment::*;
pub use shipping_zone::*;
pub use stock_snapshot::*;
pub use store_credit::*;
pub use subscription::*;
pub use supplier_sku::*;
pub use tax::*;
pub use topology_snapshot::*;
pub use transfer_order::*;
pub use unit_of_measure::*;
pub use vector::*;
pub use vendor_credit::*;
pub use vendor_return::*;
pub use warehouse::*;
pub use warranty::*;
pub use wishlist::*;
pub use x402::*;

/// Common ID type alias
pub type Id = uuid::Uuid;

/// Decimal amount type (for backward compatibility)
/// Use `Money` struct from currency module for proper currency handling
pub type Amount = rust_decimal::Decimal;
