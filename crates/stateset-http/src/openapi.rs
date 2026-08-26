//! `OpenAPI` specification generation.
//!
//! Serves the auto-generated `OpenAPI` 3.1 spec at `GET /api/v1/openapi.json`.

use axum::response::Html;
use axum::{Json, Router, routing::get};
use utoipa::OpenApi;

use crate::dto::{
    AddressDto, CreateCustomerRequest, CreateInvoiceRequest, CreateOrderItemRequest,
    CreateOrderRequest, CreatePaymentRequest, CreateProductRequest, CreateRefundRequest,
    CreateReturnItemRequest, CreateReturnRequest, CreateShipmentRequest, CustomerListResponse,
    CustomerResponse, HealthResponse, InventoryAdjustRequest, InventoryItemResponse,
    InventoryListResponse, InventoryResponse, InvoiceListResponse, InvoiceResponse,
    OrderItemResponse, OrderListResponse, OrderResponse, PaymentListResponse, PaymentResponse,
    ProductListResponse, ProductResponse, ReadyResponse, RecordInvoicePaymentRequest,
    ReturnListResponse, ReturnResponse, ShipOrderLineRequest, ShipOrderRequest,
    ShipmentListResponse, ShipmentResponse, StockPolicyDto, TenantCacheResponse,
    UpdateCustomerRequest, UpdateProductRequest, VersionResponse,
};
use crate::error::ErrorBody;
use crate::routes::a2a_credit::{
    CreateCreditTermsRequest, CreditAmountRequest, CreditTermsResponse,
};
use crate::routes::a2a_messaging::{MessageResponse, SendMessageRequest};
use crate::routes::currency::{
    ConversionResponse, ConvertCurrencyRequest, ExchangeRateListResponse, ExchangeRateResponse,
    SetExchangeRateRequest,
};
use crate::routes::gift_cards::{CreateGiftCardRequest, GiftCardListResponse, GiftCardResponse};
use crate::routes::loyalty::{
    CreateLoyaltyProgramRequest, EnrollCustomerRequest, LoyaltyAccountResponse,
    LoyaltyProgramListResponse, LoyaltyProgramResponse,
};
use crate::routes::negotiations::{
    CounterOfferRequest, CreateNegotiationRequest, NegotiationResponse,
};
use crate::routes::promotions::{CreatePromotionRequest, PromotionListResponse, PromotionResponse};
use crate::routes::reviews::{CreateReviewRequest, ReviewListResponse, ReviewResponse};
use crate::routes::segments::{CreateSegmentRequest, SegmentListResponse, SegmentResponse};
use crate::routes::shipping_zones::{
    CreateShippingZoneRequest, ShippingZoneListResponse, ShippingZoneResponse,
};
use crate::routes::store_credits::{
    AdjustStoreCreditRequest, CreateStoreCreditRequest, StoreCreditListResponse,
    StoreCreditResponse,
};
use crate::routes::subscriptions::{
    CreateSubscriptionRequest, SubscriptionListResponse, SubscriptionResponse,
};
use crate::routes::warranties::{CreateWarrantyRequest, WarrantyListResponse, WarrantyResponse};
use crate::routes::wishlists::{
    AddWishlistItemRequest, CreateWishlistRequest, WishlistItemResponse, WishlistListResponse,
    WishlistResponse,
};
use crate::state::AppState;

/// Auto-generated `OpenAPI` 3.1 specification for the StateSet Commerce API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "StateSet Commerce API",
        description = "REST API for the StateSet embedded commerce engine.",
        version = "1.0.4",
        contact(name = "StateSet", url = "https://stateset.io"),
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
    ),
    paths(
        // Health
        crate::routes::health::health,
        crate::routes::health::readiness,
        crate::routes::health::deep_health,
        crate::routes::health::metrics,
        crate::routes::health::version,
        // Orders
        crate::routes::orders::create_order,
        crate::routes::orders::get_order,
        crate::routes::orders::list_orders,
        crate::routes::orders::cancel_order,
        crate::routes::orders::ship_order,
        // Customers
        crate::routes::customers::create_customer,
        crate::routes::customers::get_customer,
        crate::routes::customers::list_customers,
        crate::routes::customers::update_customer,
        crate::routes::customers::delete_customer,
        // Products
        crate::routes::products::create_product,
        crate::routes::products::get_product,
        crate::routes::products::list_products,
        crate::routes::products::update_product,
        crate::routes::products::delete_product,
        // Inventory
        crate::routes::inventory::list_inventory,
        crate::routes::inventory::get_stock,
        crate::routes::inventory::adjust_stock,
        // Returns
        crate::routes::returns::create_return,
        crate::routes::returns::get_return,
        crate::routes::returns::list_returns,
        crate::routes::returns::approve_return,
        crate::routes::returns::set_item_disposition,
        // Shipments
        crate::routes::shipments::create_shipment,
        crate::routes::shipments::list_shipments,
        crate::routes::shipments::get_shipment,
        crate::routes::shipments::deliver_shipment,
        // Payments
        crate::routes::payments::create_payment,
        crate::routes::payments::list_payments,
        crate::routes::payments::get_payment,
        crate::routes::payments::complete_payment,
        crate::routes::payments::create_refund,
        // Invoices
        crate::routes::invoices::create_invoice,
        crate::routes::invoices::list_invoices,
        crate::routes::invoices::get_invoice,
        crate::routes::invoices::send_invoice,
        crate::routes::invoices::record_invoice_payment,
        // Reviews
        crate::routes::reviews::create_review,
        crate::routes::reviews::get_review,
        crate::routes::reviews::list_reviews,
        crate::routes::reviews::delete_review,
        // Wishlists
        crate::routes::wishlists::create_wishlist,
        crate::routes::wishlists::get_wishlist,
        crate::routes::wishlists::list_wishlists,
        crate::routes::wishlists::delete_wishlist,
        crate::routes::wishlists::add_item,
        crate::routes::wishlists::remove_item,
        // Gift Cards
        crate::routes::gift_cards::create_gift_card,
        crate::routes::gift_cards::get_gift_card,
        crate::routes::gift_cards::list_gift_cards,
        crate::routes::gift_cards::charge_gift_card,
        crate::routes::gift_cards::refund_gift_card,
        crate::routes::gift_cards::disable_gift_card,
        // Loyalty
        crate::routes::loyalty::create_program,
        crate::routes::loyalty::list_programs,
        crate::routes::loyalty::enroll_customer,
        crate::routes::loyalty::get_account,
        // Negotiations
        crate::routes::negotiations::create_negotiation,
        crate::routes::negotiations::get_negotiation,
        crate::routes::negotiations::counter_offer,
        crate::routes::negotiations::accept_negotiation,
        crate::routes::negotiations::reject_negotiation,
        // A2A Messaging
        crate::routes::a2a_messaging::send_message,
        crate::routes::a2a_messaging::list_messages,
        crate::routes::a2a_messaging::acknowledge_message,
        // A2A Credit
        crate::routes::a2a_credit::create_terms,
        crate::routes::a2a_credit::list_terms,
        crate::routes::a2a_credit::get_terms,
        crate::routes::a2a_credit::charge_credit,
        crate::routes::a2a_credit::record_payment,
        // Subscriptions
        crate::routes::subscriptions::create_subscription,
        crate::routes::subscriptions::list_subscriptions,
        crate::routes::subscriptions::get_subscription,
        crate::routes::subscriptions::pause_subscription,
        crate::routes::subscriptions::resume_subscription,
        crate::routes::subscriptions::cancel_subscription,
        // Store Credits
        crate::routes::store_credits::create_store_credit,
        crate::routes::store_credits::list_store_credits,
        crate::routes::store_credits::get_store_credit,
        crate::routes::store_credits::adjust_store_credit,
        crate::routes::store_credits::apply_store_credit,
        // Promotions
        crate::routes::promotions::create_promotion,
        crate::routes::promotions::list_promotions,
        crate::routes::promotions::get_promotion,
        crate::routes::promotions::activate_promotion,
        crate::routes::promotions::deactivate_promotion,
        // Currency
        crate::routes::currency::list_rates,
        crate::routes::currency::set_rate,
        crate::routes::currency::convert_currency,
        // Warranties
        crate::routes::warranties::create_warranty,
        crate::routes::warranties::list_warranties,
        crate::routes::warranties::get_warranty,
        // Segments
        crate::routes::segments::create_segment,
        crate::routes::segments::list_segments,
        crate::routes::segments::get_segment,
        crate::routes::segments::delete_segment,
        crate::routes::segments::add_member,
        crate::routes::segments::remove_member,
        // Shipping Zones
        crate::routes::shipping_zones::create_zone,
        crate::routes::shipping_zones::list_zones,
        crate::routes::shipping_zones::get_zone,
        crate::routes::shipping_zones::delete_zone,
        // Events
        crate::routes::events::event_stream,
        // --- B2B / ERP-ops domain objects + reports ---
        // channels
        crate::routes::channels::create,
        crate::routes::channels::list,
        crate::routes::channels::get_one,
        crate::routes::channels::update,
        crate::routes::channels::delete_one,
        crate::routes::channels::set_lock,
        // companies
        crate::routes::companies::create,
        crate::routes::companies::list,
        crate::routes::companies::get_one,
        crate::routes::companies::delete_one,
        crate::routes::companies::create_contact,
        crate::routes::companies::list_contacts,
        // transfer_orders
        crate::routes::transfer_orders::create,
        crate::routes::transfer_orders::list,
        crate::routes::transfer_orders::get_one,
        crate::routes::transfer_orders::ship,
        crate::routes::transfer_orders::receive,
        crate::routes::transfer_orders::cancel,
        // units_of_measure
        crate::routes::units_of_measure::create_class,
        crate::routes::units_of_measure::list_classes,
        crate::routes::units_of_measure::create_uom,
        crate::routes::units_of_measure::list_uoms,
        crate::routes::units_of_measure::create_rule,
        crate::routes::units_of_measure::list_rules,
        // production_batches
        crate::routes::production_batches::create,
        crate::routes::production_batches::list,
        crate::routes::production_batches::get_one,
        crate::routes::production_batches::delete_one,
        crate::routes::production_batches::add_work_orders,
        // supplier_skus
        crate::routes::supplier_skus::create,
        crate::routes::supplier_skus::list,
        crate::routes::supplier_skus::get_one,
        crate::routes::supplier_skus::delete_one,
        // vendor_returns
        crate::routes::vendor_returns::create,
        crate::routes::vendor_returns::list,
        crate::routes::vendor_returns::get_one,
        crate::routes::vendor_returns::submit,
        crate::routes::vendor_returns::process,
        crate::routes::vendor_returns::cancel,
        // vendor_credits
        crate::routes::vendor_credits::create,
        crate::routes::vendor_credits::list,
        crate::routes::vendor_credits::get_one,
        crate::routes::vendor_credits::apply,
        crate::routes::vendor_credits::cancel,
        // payment_obligations
        crate::routes::payment_obligations::create,
        crate::routes::payment_obligations::list,
        crate::routes::payment_obligations::dashboard,
        crate::routes::payment_obligations::get_one,
        crate::routes::payment_obligations::record_payment,
        crate::routes::payment_obligations::set_status,
        crate::routes::payment_obligations::link_bill,
        // prepayments
        crate::routes::prepayments::create,
        crate::routes::prepayments::list,
        crate::routes::prepayments::get_one,
        crate::routes::prepayments::apply,
        crate::routes::prepayments::refund,
        // price_levels
        crate::routes::price_levels::create,
        crate::routes::price_levels::list,
        crate::routes::price_levels::get_one,
        crate::routes::price_levels::update,
        crate::routes::price_levels::delete_one,
        crate::routes::price_levels::set_entry,
        crate::routes::price_levels::list_entries,
        // price_schedules
        crate::routes::price_schedules::create,
        crate::routes::price_schedules::list,
        crate::routes::price_schedules::resolve,
        crate::routes::price_schedules::get_one,
        crate::routes::price_schedules::delete_one,
        crate::routes::price_schedules::set_entry,
        crate::routes::price_schedules::list_entries,
        // activity_logs
        crate::routes::activity_logs::record,
        crate::routes::activity_logs::list,
        crate::routes::activity_logs::get_one,
        crate::routes::activity_logs::history,
        // integration_mappings
        crate::routes::integration_mappings::create,
        crate::routes::integration_mappings::bulk_create,
        crate::routes::integration_mappings::list,
        crate::routes::integration_mappings::resolve,
        crate::routes::integration_mappings::get_one,
        crate::routes::integration_mappings::update,
        crate::routes::integration_mappings::delete_one,
        // integration_field_mappings
        crate::routes::integration_field_mappings::create,
        crate::routes::integration_field_mappings::bulk_create,
        crate::routes::integration_field_mappings::bulk_delete,
        crate::routes::integration_field_mappings::list,
        crate::routes::integration_field_mappings::groups,
        crate::routes::integration_field_mappings::get_one,
        crate::routes::integration_field_mappings::update,
        crate::routes::integration_field_mappings::delete_one,
        // inbound_shipments
        crate::routes::inbound_shipments::create,
        crate::routes::inbound_shipments::list,
        crate::routes::inbound_shipments::get_one,
        crate::routes::inbound_shipments::mark_in_transit,
        crate::routes::inbound_shipments::mark_arrived,
        crate::routes::inbound_shipments::receive,
        crate::routes::inbound_shipments::cancel,
        // purgatory
        crate::routes::purgatory::ingest,
        crate::routes::purgatory::list,
        crate::routes::purgatory::get_one,
        crate::routes::purgatory::post_order,
        crate::routes::purgatory::map_line,
        crate::routes::purgatory::delete_one,
        // print_stations
        crate::routes::print_stations::pair,
        crate::routes::print_stations::list_stations,
        crate::routes::print_stations::revoke,
        crate::routes::print_stations::enqueue,
        crate::routes::print_stations::next_job,
        crate::routes::print_stations::list_jobs,
        crate::routes::print_stations::complete_job,
        // edi_documents
        crate::routes::edi_documents::create,
        crate::routes::edi_documents::list,
        crate::routes::edi_documents::summary,
        crate::routes::edi_documents::get_one,
        crate::routes::edi_documents::set_status,
        // topology_snapshots
        crate::routes::topology_snapshots::capture,
        crate::routes::topology_snapshots::list,
        crate::routes::topology_snapshots::latest,
        crate::routes::topology_snapshots::get_one,
        crate::routes::topology_snapshots::delete_one,
        // stock_snapshots
        crate::routes::stock_snapshots::capture,
        crate::routes::stock_snapshots::list,
        crate::routes::stock_snapshots::latest,
        crate::routes::stock_snapshots::get_one,
        crate::routes::stock_snapshots::delete_one,
        // reports
        crate::routes::reports::inventory_aging,
        crate::routes::reports::sales_by_channel,
        crate::routes::reports::transaction_cogs,
        crate::routes::reports::close_the_books,
        crate::routes::reports::consumption,
        // carts
        crate::routes::carts::create,
        crate::routes::carts::list,
        crate::routes::carts::get_one,
        crate::routes::carts::add_item,
        crate::routes::carts::update_item,
        crate::routes::carts::remove_item,
        crate::routes::carts::set_shipping,
        crate::routes::carts::set_payment,
        crate::routes::carts::complete,
        crate::routes::carts::cancel,
        // backorders
        crate::routes::backorders::create,
        crate::routes::backorders::list,
        crate::routes::backorders::get_one,
        crate::routes::backorders::fulfill,
        crate::routes::backorders::cancel,
        // purchase_orders
        crate::routes::purchase_orders::create,
        crate::routes::purchase_orders::list,
        crate::routes::purchase_orders::get_one,
        crate::routes::purchase_orders::update,
        crate::routes::purchase_orders::submit,
        crate::routes::purchase_orders::approve,
        crate::routes::purchase_orders::send,
        crate::routes::purchase_orders::acknowledge,
        crate::routes::purchase_orders::hold,
        crate::routes::purchase_orders::complete,
        crate::routes::purchase_orders::cancel,
        crate::routes::purchase_orders::receive,
        crate::routes::purchase_orders::get_items,
        crate::routes::purchase_orders::create_supplier,
        crate::routes::purchase_orders::list_suppliers,
        crate::routes::purchase_orders::get_supplier,
        crate::routes::purchase_orders::update_supplier,
        crate::routes::purchase_orders::delete_supplier,
        // general_ledger
        crate::routes::general_ledger::create_account,
        crate::routes::general_ledger::list_accounts,
        crate::routes::general_ledger::get_account,
        crate::routes::general_ledger::create_journal_entry,
        crate::routes::general_ledger::list_journal_entries,
        crate::routes::general_ledger::get_journal_entry,
        crate::routes::general_ledger::post_journal_entry,
        crate::routes::general_ledger::void_journal_entry,
        crate::routes::general_ledger::reverse_journal_entry,
        crate::routes::general_ledger::trial_balance,
        crate::routes::general_ledger::balance_sheet,
        crate::routes::general_ledger::income_statement,
        crate::routes::general_ledger::create_period,
        crate::routes::general_ledger::list_periods,
        crate::routes::general_ledger::open_period,
        crate::routes::general_ledger::close_period,
        crate::routes::general_ledger::lock_period,
        crate::routes::general_ledger::reopen_period,
        crate::routes::general_ledger::revalue,
        crate::routes::general_ledger::close_month,
        // fixed_assets
        crate::routes::fixed_assets::create,
        crate::routes::fixed_assets::list,
        crate::routes::fixed_assets::get_one,
        crate::routes::fixed_assets::update,
        crate::routes::fixed_assets::place_in_service,
        crate::routes::fixed_assets::dispose,
        crate::routes::fixed_assets::write_off,
        crate::routes::fixed_assets::generate_schedule,
        crate::routes::fixed_assets::get_schedule,
        crate::routes::fixed_assets::post_depreciation,
        // revenue_recognition
        crate::routes::revenue_recognition::create_contract,
        crate::routes::revenue_recognition::list_contracts,
        crate::routes::revenue_recognition::get_contract,
        crate::routes::revenue_recognition::update_contract,
        crate::routes::revenue_recognition::list_obligations,
        crate::routes::revenue_recognition::generate_schedule,
        crate::routes::revenue_recognition::get_schedule,
        crate::routes::revenue_recognition::recognize,
        // accounts_payable
        crate::routes::accounts_payable::create_bill,
        crate::routes::accounts_payable::list_bills,
        crate::routes::accounts_payable::get_bill,
        crate::routes::accounts_payable::approve_bill,
        crate::routes::accounts_payable::cancel_bill,
        crate::routes::accounts_payable::dispute_bill,
        crate::routes::accounts_payable::create_payment,
        crate::routes::accounts_payable::void_payment,
        crate::routes::accounts_payable::create_payment_run,
        crate::routes::accounts_payable::approve_payment_run,
        crate::routes::accounts_payable::process_payment_run,
        crate::routes::accounts_payable::cancel_payment_run,
        crate::routes::accounts_payable::aging,
        crate::routes::accounts_payable::three_way_match_bill,
        // accounts_receivable
        crate::routes::accounts_receivable::aging_summary,
        crate::routes::accounts_receivable::aging_report,
        crate::routes::accounts_receivable::customer_aging,
        crate::routes::accounts_receivable::apply_payment,
        crate::routes::accounts_receivable::unapply_payment,
        crate::routes::accounts_receivable::create_credit_memo,
        crate::routes::accounts_receivable::list_credit_memos,
        crate::routes::accounts_receivable::apply_credit_memo,
        crate::routes::accounts_receivable::create_write_off,
        crate::routes::accounts_receivable::reverse_write_off,
        crate::routes::accounts_receivable::send_dunning,
        crate::routes::accounts_receivable::customer_statement,
        crate::routes::accounts_receivable::record_collection_activity,
        crate::routes::accounts_receivable::list_collection_activities,
        crate::routes::accounts_receivable::invoices_due_for_dunning,
        // warehouse
        crate::routes::warehouse::create_warehouse,
        crate::routes::warehouse::list_warehouses,
        crate::routes::warehouse::get_warehouse,
        crate::routes::warehouse::update_warehouse,
        crate::routes::warehouse::delete_warehouse,
        crate::routes::warehouse::create_location,
        crate::routes::warehouse::list_locations,
        crate::routes::warehouse::get_location,
        crate::routes::warehouse::get_location_inventory,
        crate::routes::warehouse::adjust_inventory,
        crate::routes::warehouse::move_inventory,
        crate::routes::warehouse::create_bin,
        crate::routes::warehouse::list_bins,
        crate::routes::warehouse::get_bin,
        crate::routes::warehouse::update_bin,
        crate::routes::warehouse::delete_bin,
        crate::routes::warehouse::get_bin_levels,
        crate::routes::warehouse::adjust_bin_level,
        crate::routes::warehouse::move_between_bins,
        crate::routes::warehouse::reconcile_bins,
        crate::routes::warehouse::create_cycle_count,
        crate::routes::warehouse::list_cycle_counts,
        crate::routes::warehouse::get_cycle_count,
        crate::routes::warehouse::start_cycle_count,
        crate::routes::warehouse::record_cycle_counts,
        crate::routes::warehouse::complete_cycle_count,
        crate::routes::warehouse::cancel_cycle_count,
        // fulfillment
        crate::routes::fulfillment::create_wave,
        crate::routes::fulfillment::list_waves,
        crate::routes::fulfillment::get_wave,
        crate::routes::fulfillment::release_wave,
        crate::routes::fulfillment::list_picks,
        crate::routes::fulfillment::assign_pick,
        crate::routes::fulfillment::complete_pick,
        crate::routes::fulfillment::list_packs,
        crate::routes::fulfillment::complete_pack,
        crate::routes::fulfillment::add_carton,
        crate::routes::fulfillment::list_cartons,
        crate::routes::fulfillment::list_ships,
        crate::routes::fulfillment::complete_ship,
        // receiving
        crate::routes::receiving::create_receipt,
        crate::routes::receiving::list_receipts,
        crate::routes::receiving::get_receipt,
        crate::routes::receiving::list_receipt_items,
        crate::routes::receiving::start_receiving,
        crate::routes::receiving::receive_items,
        crate::routes::receiving::complete_receiving,
        crate::routes::receiving::cancel_receipt,
        crate::routes::receiving::create_put_away,
        crate::routes::receiving::list_put_aways,
        crate::routes::receiving::get_put_away,
        crate::routes::receiving::complete_put_away,
        // work_orders
        crate::routes::work_orders::create,
        crate::routes::work_orders::list,
        crate::routes::work_orders::get_one,
        crate::routes::work_orders::update,
        crate::routes::work_orders::start,
        crate::routes::work_orders::complete,
        crate::routes::work_orders::hold,
        crate::routes::work_orders::resume,
        crate::routes::work_orders::cancel,
        crate::routes::work_orders::add_task,
        crate::routes::work_orders::list_tasks,
        crate::routes::work_orders::start_task,
        crate::routes::work_orders::complete_task,
        // quality
        crate::routes::quality::create_inspection,
        crate::routes::quality::list_inspections,
        crate::routes::quality::get_inspection,
        crate::routes::quality::start_inspection,
        crate::routes::quality::record_inspection_result,
        crate::routes::quality::complete_inspection,
        crate::routes::quality::create_ncr,
        crate::routes::quality::list_ncrs,
        crate::routes::quality::get_ncr,
        crate::routes::quality::disposition_ncr,
        crate::routes::quality::close_ncr,
        crate::routes::quality::create_hold,
        crate::routes::quality::list_holds,
        crate::routes::quality::get_hold,
        crate::routes::quality::release_hold,
        // bom
        crate::routes::bom::create,
        crate::routes::bom::list,
        crate::routes::bom::get_one,
        crate::routes::bom::update,
        crate::routes::bom::delete_one,
        crate::routes::bom::activate,
        crate::routes::bom::add_component,
        crate::routes::bom::list_components,
        // lots
        crate::routes::lots::create,
        crate::routes::lots::list,
        crate::routes::lots::expiring,
        crate::routes::lots::get_one,
        crate::routes::lots::consume,
        crate::routes::lots::reserve,
        crate::routes::lots::release_reservation,
        crate::routes::lots::quarantine,
        crate::routes::lots::release_quarantine,
        // serials
        crate::routes::serials::create,
        crate::routes::serials::list,
        crate::routes::serials::get_one,
        crate::routes::serials::reserve,
        crate::routes::serials::release_reservation,
        crate::routes::serials::ship,
        crate::routes::serials::mark_returned,
        crate::routes::serials::scrap,
    ),
    components(schemas(
        // Request DTOs
        CreateOrderRequest,
        CreateOrderItemRequest,
        StockPolicyDto,
        AddressDto,
        CreateCustomerRequest,
        CreateProductRequest,
        InventoryAdjustRequest,
        CreateReturnRequest,
        CreateReturnItemRequest,
        UpdateCustomerRequest,
        UpdateProductRequest,
        CreateShipmentRequest,
        CreatePaymentRequest,
        CreateRefundRequest,
        CreateInvoiceRequest,
        RecordInvoicePaymentRequest,
        ShipOrderRequest,
        ShipOrderLineRequest,
        // Response DTOs
        OrderResponse,
        OrderItemResponse,
        OrderListResponse,
        CustomerResponse,
        CustomerListResponse,
        ProductResponse,
        ProductListResponse,
        InventoryResponse,
        InventoryItemResponse,
        InventoryListResponse,
        ShipmentResponse,
        ShipmentListResponse,
        PaymentResponse,
        PaymentListResponse,
        InvoiceResponse,
        InvoiceListResponse,
        ReturnResponse,
        ReturnListResponse,
        HealthResponse,
        ReadyResponse,
        VersionResponse,
        TenantCacheResponse,
        // Reviews
        CreateReviewRequest,
        ReviewResponse,
        ReviewListResponse,
        // Wishlists
        CreateWishlistRequest,
        AddWishlistItemRequest,
        WishlistResponse,
        WishlistItemResponse,
        WishlistListResponse,
        // Gift Cards
        CreateGiftCardRequest,
        GiftCardResponse,
        GiftCardListResponse,
        // Loyalty
        CreateLoyaltyProgramRequest,
        EnrollCustomerRequest,
        LoyaltyProgramResponse,
        LoyaltyProgramListResponse,
        LoyaltyAccountResponse,
        // Negotiations
        CreateNegotiationRequest,
        CounterOfferRequest,
        NegotiationResponse,
        // A2A Messaging
        SendMessageRequest,
        MessageResponse,
        // A2A Credit
        CreateCreditTermsRequest,
        CreditAmountRequest,
        CreditTermsResponse,
        // Subscriptions
        CreateSubscriptionRequest,
        SubscriptionResponse,
        SubscriptionListResponse,
        // Store Credits
        CreateStoreCreditRequest,
        AdjustStoreCreditRequest,
        StoreCreditResponse,
        StoreCreditListResponse,
        // Promotions
        CreatePromotionRequest,
        PromotionResponse,
        PromotionListResponse,
        // Currency
        SetExchangeRateRequest,
        ConvertCurrencyRequest,
        ExchangeRateResponse,
        ExchangeRateListResponse,
        ConversionResponse,
        // Warranties
        CreateWarrantyRequest,
        WarrantyResponse,
        WarrantyListResponse,
        // Segments
        CreateSegmentRequest,
        SegmentResponse,
        SegmentListResponse,
        // Shipping Zones
        CreateShippingZoneRequest,
        ShippingZoneResponse,
        ShippingZoneListResponse,
        // Error
        ErrorBody,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "orders", description = "Order lifecycle management"),
        (name = "customers", description = "Customer management"),
        (name = "products", description = "Product catalog"),
        (name = "inventory", description = "Stock and inventory management"),
        (name = "returns", description = "Return request processing"),
        (name = "shipments", description = "Shipment tracking and management"),
        (name = "payments", description = "Payment transaction management"),
        (name = "invoices", description = "Invoice management"),
        (name = "reviews", description = "Product review management"),
        (name = "wishlists", description = "Customer wishlist management"),
        (name = "gift_cards", description = "Gift card management"),
        (name = "loyalty", description = "Loyalty program management"),
        (name = "negotiations", description = "Agent-to-agent price negotiation"),
        (name = "a2a", description = "Agent-to-agent messaging and credit terms"),
        (name = "subscriptions", description = "Recurring subscription management"),
        (name = "store_credits", description = "Store credit management"),
        (name = "promotions", description = "Promotion and discount management"),
        (name = "currency", description = "Exchange rates and currency conversion"),
        (name = "warranties", description = "Product warranty management"),
        (name = "segments", description = "Customer segment management"),
        (name = "shipping", description = "Shipping zone management"),
        (name = "events", description = "Real-time event streaming"),
        (name = "activity_logs", description = "Record-level activity history"),
        (name = "channels", description = "Sales channel management"),
        (name = "companies", description = "B2B company account management"),
        (name = "edi_documents", description = "EDI document exchange"),
        (name = "inbound_shipments", description = "Inbound shipment receiving"),
        (name = "integration_field_mappings", description = "Integration field-level mappings"),
        (name = "integration_mappings", description = "External integration record mappings"),
        (name = "payment_obligations", description = "Payment obligation tracking"),
        (name = "prepayments", description = "Customer and vendor prepayments"),
        (name = "price_levels", description = "Customer price level management"),
        (name = "price_schedules", description = "Scheduled pricing management"),
        (name = "print_stations", description = "Warehouse print station management"),
        (name = "production_batches", description = "Production batch tracking"),
        (name = "purgatory", description = "Quarantined record review"),
        (name = "reports", description = "Computed business reports"),
        (name = "stock_snapshots", description = "Point-in-time inventory snapshots"),
        (name = "supplier_skus", description = "Supplier SKU catalog"),
        (name = "topology_snapshots", description = "Network topology snapshots"),
        (name = "transfer_orders", description = "Inter-warehouse transfer orders"),
        (name = "units_of_measure", description = "Unit of measure definitions"),
        (name = "vendor_credits", description = "Vendor credit management"),
        (name = "vendor_returns", description = "Return-to-vendor processing"),
        (name = "purchase_orders", description = "Supplier procurement: purchase order lifecycle (draft → approval → send → acknowledge → receive → complete) and supplier management"),
        (name = "general_ledger", description = "Chart of accounts, journal entries, accounting periods, and financial reports (trial balance, balance sheet, income statement)"),
        (name = "accounts_payable", description = "Supplier bills, AP payments and allocations, payment runs, and AP aging"),
        (name = "accounts_receivable", description = "AR aging, payment application, credit memos, write-offs, dunning, and customer statements"),
        (name = "warehouse", description = "Warehouses, storage locations, and location-level inventory (adjust/move)"),
        (name = "fulfillment", description = "Outbound fulfillment: waves, pick tasks, pack tasks and cartons, ship tasks"),
        (name = "receiving", description = "Inbound receiving: goods receipts, item receipt, and put-away tasks"),
        (name = "work_orders", description = "Manufacturing work order lifecycle and shop-floor tasks"),
        (name = "quality", description = "Quality control: inspections, non-conformance reports, and quality holds"),
        (name = "bom", description = "Bills of materials: BOM revisions, components, and status control"),
        (name = "lots", description = "Lot/batch tracking: creation, consumption, reservations, quarantine, and expiry queries"),
        (name = "serials", description = "Serial number tracking: creation, lookup, reservations, and lifecycle transitions"),
        (name = "fixed_assets", description = "Fixed-asset register: acquisition, depreciation schedules, disposal, and write-off"),
        (name = "revenue_recognition", description = "Revenue contracts, performance obligations, and recognition schedules (ASC 606)"),
        (name = "carts", description = "Cart and checkout sessions: items, shipping, payment, completion"),
        (name = "backorders", description = "Backorder creation, fulfillment, and cancellation"),
    )
)]
#[derive(Debug)]
pub struct ApiDoc;

/// Build the `OpenAPI` spec router.
pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json)).route("/docs", get(docs_ui))
}

/// `GET /api/v1/openapi.json` — returns the `OpenAPI` spec as JSON.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

/// `GET /api/v1/docs` — interactive API documentation UI (Scalar).
async fn docs_ui() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html>
<head>
  <title>StateSet Commerce API</title>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
</head>
<body>
  <script id="api-reference" data-url="/api/v1/openapi.json"></script>
  <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body>
</html>"#,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_serializes_to_json() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["info"]["title"], "StateSet Commerce API");
        assert_eq!(json["info"]["version"], "1.0.4");
    }

    #[test]
    fn openapi_spec_has_all_paths() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let paths = json["paths"].as_object().unwrap();

        assert!(paths.contains_key("/health"), "missing /health");
        assert!(paths.contains_key("/health/ready"), "missing /health/ready");
        assert!(paths.contains_key("/metrics"), "missing /metrics");
        assert!(paths.contains_key("/version"), "missing /version");
        assert!(paths.contains_key("/api/v1/orders"), "missing /api/v1/orders");
        assert!(paths.contains_key("/api/v1/orders/{id}"), "missing /api/v1/orders/{{id}}");
        assert!(paths.contains_key("/api/v1/customers"), "missing /api/v1/customers");
        assert!(paths.contains_key("/api/v1/products"), "missing /api/v1/products");
        assert!(paths.contains_key("/api/v1/inventory"), "missing /api/v1/inventory");
        assert!(paths.contains_key("/api/v1/inventory/{sku}"), "missing /api/v1/inventory/{{sku}}");
        assert!(paths.contains_key("/api/v1/returns"), "missing /api/v1/returns");
        assert!(
            paths.contains_key("/api/v1/returns/{id}/approve"),
            "missing /api/v1/returns/{{id}}/approve"
        );
        assert!(paths.contains_key("/api/v1/shipments"), "missing /api/v1/shipments");
        assert!(paths.contains_key("/api/v1/shipments/{id}"), "missing /api/v1/shipments/{{id}}");
        assert!(paths.contains_key("/api/v1/payments"), "missing /api/v1/payments");
        assert!(paths.contains_key("/api/v1/payments/{id}"), "missing /api/v1/payments/{{id}}");
        assert!(paths.contains_key("/api/v1/invoices"), "missing /api/v1/invoices");
        assert!(paths.contains_key("/api/v1/invoices/{id}"), "missing /api/v1/invoices/{{id}}");

        // B2B / ERP-ops domain objects + reports must also be in the spec.
        for path in [
            "/api/v1/channels",
            "/api/v1/companies",
            "/api/v1/transfer-orders",
            "/api/v1/unit-classes",
            "/api/v1/production-batches",
            "/api/v1/supplier-skus",
            "/api/v1/vendor-returns",
            "/api/v1/vendor-credits",
            "/api/v1/payment-obligations",
            "/api/v1/prepayments",
            "/api/v1/price-levels",
            "/api/v1/price-schedules",
            "/api/v1/activity-logs",
            "/api/v1/integration-mappings",
            "/api/v1/integration-field-mappings",
            "/api/v1/inbound-shipments",
            "/api/v1/purgatory/orders",
            "/api/v1/print-stations",
            "/api/v1/edi-documents",
            "/api/v1/topology-snapshots",
            "/api/v1/stock-snapshots",
            "/api/v1/reports/inventory-aging",
            "/api/v1/reports/consumption",
        ] {
            assert!(paths.contains_key(path), "missing {path}");
        }
    }

    #[test]
    fn openapi_spec_has_all_schemas() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let schemas = json["components"]["schemas"].as_object().unwrap();

        let expected = [
            "CreateOrderRequest",
            "OrderResponse",
            "CustomerResponse",
            "ProductResponse",
            "InventoryResponse",
            "InventoryItemResponse",
            "ShipmentResponse",
            "PaymentResponse",
            "InvoiceResponse",
            "ReturnResponse",
            "HealthResponse",
            "TenantCacheResponse",
            "ErrorBody",
        ];
        for name in expected {
            assert!(schemas.contains_key(name), "missing schema: {name}");
        }
    }

    /// The error schema must document the optional stable commerce-invariant
    /// code, so generated clients can branch on it. See
    /// `docs/src/advanced/invariants.md`.
    #[test]
    fn error_schema_documents_invariant_code() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let schemas = json["components"]["schemas"].to_string();
        assert!(schemas.contains("invariant"), "ErrorBody schema must expose `invariant`");
    }

    #[test]
    fn openapi_spec_has_tags() {
        let spec = ApiDoc::openapi();
        let json = serde_json::to_value(&spec).unwrap();
        let tags: Vec<String> = json["tags"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(tags.contains(&"orders".to_string()));
        assert!(tags.contains(&"customers".to_string()));
        assert!(tags.contains(&"health".to_string()));
    }
}
