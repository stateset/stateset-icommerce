//! Both SQL backends use this planner. Compare its actual selections with
//! Lean's executable algorithm, retaining reservation identity and row order.
use super::*;
use serde::Deserialize;
use stateset_core::{FulfillmentStatus, OrderItem};

#[derive(Debug, Deserialize)]
struct Row {
    owner: u128,
    quantity: i64,
}
#[derive(Debug, Deserialize)]
struct Case {
    scale: u32,
    demand: [i32; 2],
    request: [i32; 2],
    rows: Vec<Row>,
    accepted: bool,
    amounts: Vec<i64>,
}

fn line_id(owner: u128) -> Uuid {
    Uuid::from_u128(owner + 1)
}

fn order(demand: [i32; 2], prior: bool) -> Order {
    let now = chrono::Utc::now();
    let id = Uuid::from_u128(1000).into();
    Order {
        id,
        order_number: "OWNERSHIP".into(),
        customer_id: Uuid::from_u128(2000).into(),
        status: if prior { OrderStatus::PartiallyShipped } else { OrderStatus::Processing },
        order_date: now,
        total_amount: Decimal::from(100),
        tax_amount: Decimal::ZERO,
        shipping_amount: Decimal::ZERO,
        discount_amount: Decimal::ZERO,
        currency: Default::default(),
        payment_status: PaymentStatus::Paid,
        fulfillment_status: FulfillmentStatus::default(),
        payment_method: None,
        shipping_method: None,
        tracking_number: None,
        notes: None,
        shipping_address: None,
        billing_address: None,
        items: demand
            .into_iter()
            .enumerate()
            .map(|(i, quantity)| OrderItem {
                id: line_id(i as u128).into(),
                order_id: id,
                product_id: Uuid::from_u128(3000).into(),
                variant_id: None,
                sku: "SAME-SKU".into(),
                name: "Same SKU, distinct line".into(),
                quantity: quantity + i32::from(prior),
                shipped_quantity: i32::from(prior),
                unit_price: Decimal::ONE,
                discount: Decimal::ZERO,
                tax_amount: Decimal::ZERO,
                total: Decimal::from(quantity + i32::from(prior)),
            })
            .collect(),
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn reserved_stock_selection_matches_lean_ownership() {
    let mut checked = 0;
    for line in
        include_str!("../../../../../formal/lean/reservation_ownership.golden.jsonl").lines()
    {
        let case: Case = serde_json::from_str(line).unwrap();
        let allocated: Decimal =
            case.rows.iter().map(|r| Decimal::new(r.quantity, case.scale)).sum();
        let rows: Vec<_> = case
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| ReservedShipmentStock {
                id: Uuid::from_u128(100 + i as u128),
                order_item_id: Some(line_id(row.owner).to_string()),
                sku: "SAME-SKU".into(),
                quantity: Decimal::new(row.quantity, case.scale),
                item_id: 1,
                location_id: 1,
                on_hand: allocated + Decimal::from(10),
                allocated,
                available: Decimal::from(10),
                expires_at: None,
            })
            .collect();
        for prior in [false, true] {
            let order = order(case.demand, prior);
            for repeated in [false, true] {
                let deltas: Vec<_> = case
                    .request
                    .iter()
                    .enumerate()
                    .flat_map(|(i, &quantity)| {
                        let id = line_id(i as u128).to_string();
                        if repeated {
                            vec![(id, 1); quantity as usize]
                        } else {
                            vec![(id, quantity)]
                        }
                    })
                    .collect();
                let result = plan_reserved_stock(
                    &order,
                    &["SAME-SKU".into()],
                    &rows,
                    &deltas,
                    prior.then_some(&order),
                );
                assert_eq!(
                    result.is_ok(),
                    case.accepted,
                    "case={line} prior={prior} repeated={repeated}: {result:?}"
                );
                if let Ok(selected) = result {
                    let actual: Vec<_> = selected.iter().map(|r| (r.id, r.quantity)).collect();
                    let expected: Vec<_> = case
                        .amounts
                        .iter()
                        .enumerate()
                        .filter(|(_, q)| **q > 0)
                        .map(|(i, &q)| (rows[i].id, Decimal::new(q, case.scale)))
                        .collect();
                    assert_eq!(actual, expected, "case={line} prior={prior} repeated={repeated}");
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 8640, "all shared ownership fixtures and request encodings must run");
}
