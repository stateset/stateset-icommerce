#![cfg(all(feature = "sqlite", feature = "events"))]

//! Real carrier labels through [`ShipmentProvider`]s.
//!
//! The mock loop proves the engine persists the buy (tracking number,
//! postage cost, `label_purchased` event, shipped status). The `EasyPost` case
//! runs only with `EASYPOST_TEST_KEY` set and stays read-only (rates).

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer, CreateOrder, CreateOrderItem, CreateShipment, CreateShipmentItem,
    CustomerId, MockShipmentProvider, OrderId, Parcel, PostalAddress, PurchasedLabel, RateQuote,
    ShipmentProvider, ShipmentStatus, ShippingCarrier,
};
use uuid::Uuid;

fn address() -> PostalAddress {
    PostalAddress {
        name: Some("Ada Lovelace".into()),
        street1: "1 Main St".into(),
        city: "Springfield".into(),
        state: "IL".into(),
        zip: "62701".into(),
        country: "US".into(),
        ..Default::default()
    }
}

const fn parcel() -> Parcel {
    Parcel { weight_oz: dec!(16), length_in: None, width_in: None, height_in: None }
}

fn order(commerce: &Commerce) -> OrderId {
    let customer: CustomerId = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("label-{}@example.com", Uuid::new_v4()),
            first_name: "Label".into(),
            last_name: "Buyer".into(),
            ..Default::default()
        })
        .expect("customer")
        .id;
    commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer,
            items: vec![CreateOrderItem {
                product_id: Uuid::new_v4().into(),
                sku: "LABEL-SKU".into(),
                name: "Label Widget".into(),
                quantity: 1,
                unit_price: dec!(25.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("order")
        .id
}

fn draft_shipment(commerce: &Commerce) -> stateset_embedded::Shipment {
    commerce
        .shipments()
        .create(CreateShipment {
            order_id: order(commerce),
            recipient_name: "Ada Lovelace".into(),
            shipping_address: "1 Main St, Springfield, IL 62701".into(),
            items: Some(vec![CreateShipmentItem {
                sku: "LABEL-SKU".into(),
                name: "Label Widget".into(),
                quantity: 1,
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect("shipment")
}

#[test]
fn mock_rates_then_buy_persists_label_and_ships() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let shipment = draft_shipment(&commerce);
    let mock = MockShipmentProvider::new(
        vec![
            RateQuote {
                id: "rate_slow".into(),
                carrier: "USPS".into(),
                service: "Ground".into(),
                amount: dec!(7.34),
                currency: "USD".into(),
                delivery_days: Some(4),
            },
            RateQuote {
                id: "rate_fast".into(),
                carrier: "UPS".into(),
                service: "NextDay".into(),
                amount: dec!(29.00),
                currency: "USD".into(),
                delivery_days: Some(1),
            },
        ],
        vec![PurchasedLabel {
            shipment_id: "shp_mock_1".into(),
            tracking_number: "1Z999MOCK".into(),
            label_url: Some("https://carrier.example/label.pdf".into()),
            carrier: "USPS".into(),
            service: "Ground".into(),
            rate_id: "rate_slow".into(),
            amount: dec!(7.34),
            currency: "USD".into(),
        }],
    );

    // No explicit rate: the cheapest quote wins and the choice is audited.
    let shipped = commerce
        .shipments()
        .buy_label_with_provider(shipment.id, &mock, &address(), &address(), &parcel(), None)
        .expect("buy label");

    assert_eq!(shipped.tracking_number.as_deref(), Some("1Z999MOCK"));
    assert_eq!(shipped.shipping_cost, Some(dec!(7.34)));
    assert_eq!(shipped.status, ShipmentStatus::Shipped);

    let events = commerce.shipments().get_events(shipment.id).expect("events");
    let bought = events.iter().find(|e| e.event_type == "label_purchased").expect("label event");
    let description = bought.description.as_deref().unwrap_or_default();
    assert!(description.contains("rate_slow"), "choice audited: {description}");
    assert!(
        description.contains("https://carrier.example/label.pdf"),
        "receipt kept: {description}"
    );

    assert_eq!(mock.calls(), vec!["rates", "buy:rate_slow"]);
}

#[test]
fn mock_explicit_rate_wins_over_cheapest() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let shipment = draft_shipment(&commerce);
    let mock = MockShipmentProvider::new(
        vec![RateQuote {
            id: "rate_fast".into(),
            carrier: "UPS".into(),
            service: "NextDay".into(),
            amount: dec!(29.00),
            currency: "USD".into(),
            delivery_days: Some(1),
        }],
        vec![PurchasedLabel {
            shipment_id: "shp_mock_2".into(),
            tracking_number: "1Z999FAST".into(),
            label_url: None,
            carrier: "UPS".into(),
            service: "NextDay".into(),
            rate_id: "rate_fast".into(),
            amount: dec!(29.00),
            currency: "USD".into(),
        }],
    );

    let shipped = commerce
        .shipments()
        .buy_label_with_provider(
            shipment.id,
            &mock,
            &address(),
            &address(),
            &parcel(),
            Some("rate_fast"),
        )
        .expect("buy label");
    assert_eq!(shipped.shipping_cost, Some(dec!(29.00)));
    assert_eq!(mock.calls(), vec!["buy:rate_fast"]);
}

#[test]
fn buy_label_from_partially_packed_shipment_still_ships() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let mock = MockShipmentProvider::new(
        vec![RateQuote {
            id: "rate_flat".into(),
            carrier: "USPS".into(),
            service: "Ground".into(),
            amount: dec!(5.00),
            currency: "USD".into(),
            delivery_days: None,
        }],
        vec![PurchasedLabel {
            shipment_id: "shp_mock_packed".into(),
            tracking_number: "1Z999PACKED".into(),
            label_url: None,
            carrier: "USPS".into(),
            service: "Ground".into(),
            rate_id: "rate_flat".into(),
            amount: dec!(5.00),
            currency: "USD".into(),
        }],
    );

    // Mid-lifecycle (`Processing`) and packed (`ReadyToShip`) shipments must
    // not trip the packing walk: buying the label finishes the handoff.
    let processing = draft_shipment(&commerce);
    commerce.shipments().mark_processing(processing.id).expect("processing");
    let shipped = commerce
        .shipments()
        .buy_label_with_provider(
            processing.id,
            &mock,
            &address(),
            &address(),
            &parcel(),
            Some("rate_flat"),
        )
        .expect("buy label from processing");
    assert_eq!(shipped.status, ShipmentStatus::Shipped);
    assert_eq!(shipped.tracking_number.as_deref(), Some("1Z999PACKED"));

    let packed = draft_shipment(&commerce);
    commerce.shipments().mark_processing(packed.id).expect("processing");
    commerce.shipments().mark_ready(packed.id).expect("ready");
    let shipped = commerce
        .shipments()
        .buy_label_with_provider(
            packed.id,
            &mock,
            &address(),
            &address(),
            &parcel(),
            Some("rate_flat"),
        )
        .expect("buy label from ready");
    assert_eq!(shipped.status, ShipmentStatus::Shipped);
    assert_eq!(shipped.tracking_number.as_deref(), Some("1Z999PACKED"));
}

fn flat_mock() -> MockShipmentProvider {
    MockShipmentProvider::new(
        vec![RateQuote {
            id: "rate_flat".into(),
            carrier: "USPS".into(),
            service: "Ground".into(),
            amount: dec!(5.00),
            currency: "USD".into(),
            delivery_days: None,
        }],
        vec![PurchasedLabel {
            shipment_id: "shp_flat".into(),
            tracking_number: "9400FLAT".into(),
            label_url: None,
            carrier: "USPS".into(),
            service: "Ground".into(),
            rate_id: "rate_flat".into(),
            amount: dec!(5.00),
            currency: "USD".into(),
        }],
    )
}

#[test]
fn retry_after_a_bought_label_never_buys_postage_twice() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let shipment = draft_shipment(&commerce);
    let mock = flat_mock();
    let shipped = commerce
        .shipments()
        .buy_label_with_provider(shipment.id, &mock, &address(), &address(), &parcel(), None)
        .expect("first buy");
    assert_eq!(shipped.carrier, ShippingCarrier::Usps, "carrier follows the label");

    let err = commerce
        .shipments()
        .buy_label_with_provider(shipment.id, &mock, &address(), &address(), &parcel(), None)
        .expect_err("a shipped shipment must not buy a second label");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));
    assert_eq!(mock.calls(), vec!["rates", "buy:rate_flat"], "no second rate or buy call");
    let after = commerce.shipments().get(shipment.id).expect("get").expect("row");
    assert_eq!(after.tracking_number.as_deref(), Some("9400FLAT"));
}

#[test]
fn unknown_or_closed_shipment_is_refused_before_buying() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let mock = flat_mock();
    let missing = commerce.shipments().buy_label_with_provider(
        stateset_embedded::ShipmentId::new(),
        &mock,
        &address(),
        &address(),
        &parcel(),
        Some("rate_flat"),
    );
    assert!(missing.is_err());

    let cancelled = draft_shipment(&commerce);
    commerce.shipments().cancel(cancelled.id).expect("cancel");
    let refused = commerce.shipments().buy_label_with_provider(
        cancelled.id,
        &mock,
        &address(),
        &address(),
        &parcel(),
        Some("rate_flat"),
    );
    assert!(refused.is_err());
    assert!(mock.calls().is_empty(), "no postage bought: {:?}", mock.calls());
}

/// Live `EasyPost` test mode. Requires `EASYPOST_TEST_KEY`; otherwise skipped.
/// Rates only — no label is bought, so no money moves.
#[test]
fn easypost_test_mode_returns_rates() {
    let Some(key) = std::env::var("EASYPOST_TEST_KEY").ok() else {
        eprintln!("EASYPOST_TEST_KEY not set; skipping live easypost test");
        return;
    };
    let provider = stateset_embedded::EasyPostProvider::new(key).expect("easypost provider");
    let rates = provider.rates(&address(), &address(), &parcel()).expect("test rates");
    assert!(!rates.is_empty(), "test mode returned no rates");
    assert!(rates.iter().all(|r| r.amount > dec!(0)));
}
