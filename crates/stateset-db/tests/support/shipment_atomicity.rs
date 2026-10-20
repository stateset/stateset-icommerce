use rust_decimal::Decimal;
use stateset_core::*;
use uuid::Uuid;

pub(crate) const FAULTS: [&str; 4] = ["ledger", "event", "order", "receipt"];

pub(crate) fn policy() -> KernelPolicy {
    KernelPolicy::new("shipment-atomicity-v1")
        .allow("orders.ship_reserved", KernelCommandPolicy::requiring(["orders.ship_reserved"]))
}

pub(crate) fn stock(sku: &str) -> CreateInventoryItem {
    CreateInventoryItem {
        sku: sku.into(),
        name: "Atomic shipment".into(),
        initial_quantity: Some(Decimal::from(10)),
        ..Default::default()
    }
}

pub(crate) fn customer() -> CreateCustomer {
    CreateCustomer {
        email: format!("atomic-{}@example.com", Uuid::new_v4()),
        first_name: "Atomic".into(),
        last_name: "Shipment".into(),
        ..Default::default()
    }
}

pub(crate) fn order(customer_id: CustomerId, skus: &[String; 2]) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: skus
            .iter()
            .zip([2, 3])
            .map(|(sku, quantity)| CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.clone(),
                name: "Stock".into(),
                quantity,
                unit_price: Decimal::from(10),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

pub(crate) fn payment(order: &Order) -> CreatePayment {
    CreatePayment {
        order_id: Some(order.id),
        amount: order.total_amount,
        currency: Some(order.currency),
        ..Default::default()
    }
}

pub(crate) fn command(order: &Order, partial: bool) -> CommandEnvelope<ShipOrderCommand> {
    let kind = "orders.ship_reserved";
    let mut command = CommandEnvelope::preview(
        kind,
        Uuid::new_v4().to_string(),
        KernelPrincipal {
            id: "agent:shipment-atomicity".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("atomicity-tenant".into()),
            delegated_by: Some("user:shipment-atomicity".into()),
            capabilities: vec![kind.into()],
        },
        ShipOrderCommand {
            order_id: order.id,
            tracking_number: Some("ATOMIC-TRACK".into()),
            lines: partial.then(|| {
                order
                    .items
                    .iter()
                    .map(|item| ShipmentLineInput { order_item_id: item.id, quantity: 1 })
                    .collect()
            }),
        },
    );
    command.store_id = Some("atomicity-store".into());
    command.policy_version = Some("shipment-atomicity-v1".into());
    command.expected_version = Some(order.version);
    command
}

/// Every predicate is scoped to this test's generated identifiers. Failing
/// the second item proves the first item's earlier writes also roll back.
pub(crate) fn fault(
    fault: &str,
    item: i64,
    reservation: Uuid,
    order: OrderId,
    key: &str,
) -> (&'static str, &'static str, String) {
    match fault {
        "ledger" => (
            "inventory_transactions",
            "INSERT",
            format!("NEW.item_id = {item} AND NEW.transaction_type = 'shipment'"),
        ),
        "event" => (
            "kernel_outbox",
            "INSERT",
            format!(
                "NEW.aggregate_id = '{reservation}' AND NEW.event_type = 'inventory.reservation_fulfilled.v1'"
            ),
        ),
        "order" => ("orders", "UPDATE", format!("NEW.id = '{order}'")),
        "receipt" => ("kernel_receipts", "INSERT", format!("NEW.idempotency_key = '{key}'")),
        _ => panic!("unknown fault"),
    }
}

pub(crate) fn check_receipt(receipt: &ExecutionReceipt<Order>, before: &Order, partial: bool) {
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{:?}", receipt.error_message);
    assert_eq!(receipt.version_before, Some(before.version));
    assert_eq!(receipt.version_after, Some(before.version + 1));
    let order = receipt.result.as_ref().unwrap();
    assert_eq!(
        order.status,
        if partial { OrderStatus::PartiallyShipped } else { OrderStatus::Shipped }
    );
    assert!(order.items.iter().all(|i| i.shipped_quantity == if partial { 1 } else { i.quantity }));
    assert_eq!(receipt.event_ids.len(), 3, "two inventory facts and one order fact");
    let mut ids = receipt.event_ids.clone();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3);
}
