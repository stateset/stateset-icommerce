use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;
fn customer(db: &dyn Database) -> CustomerId {
    db.customers()
        .create(CreateCustomer {
            email: format!("credit-{}@example.com", Uuid::new_v4()),
            first_name: "Credit".into(),
            last_name: "Tests".into(),
            ..Default::default()
        })
        .unwrap()
        .id
}
fn account(db: &dyn Database) -> CustomerId {
    let id = customer(db);
    db.credit()
        .create_credit_account(CreateCreditAccount {
            customer_id: id,
            credit_limit: dec!(100),
            ..Default::default()
        })
        .unwrap();
    id
}
fn order_input(id: CustomerId, amount: Decimal) -> CreateOrder {
    CreateOrder {
        customer_id: id,
        items: vec![CreateOrderItem {
            product_id: ProductId::new(),
            sku: "CREDIT-SERVICE".into(),
            name: "Service".into(),
            quantity: 1,
            unit_price: amount,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn draft(db: &dyn Database, id: CustomerId, order: Option<OrderId>, amount: Decimal) -> Invoice {
    db.invoices()
        .create(CreateInvoice {
            customer_id: id,
            order_id: order,
            items: vec![CreateInvoiceItem {
                description: "Service".into(),
                quantity: dec!(1),
                unit_price: amount,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
}
pub(crate) fn eligibility(db: &dyn Database) {
    let customer = account(db);
    let credit = db.credit();
    let order = OrderId::new();
    credit.suspend_credit_account(customer, "Review").unwrap();
    assert!(!credit.check_credit(customer, dec!(10)).unwrap().approved);
    assert!(credit.reserve_credit(customer, order, dec!(10)).is_err());
    assert!(credit.charge_credit(customer, order, dec!(10)).is_err());
    assert!(db.orders().create(order_input(customer, dec!(10))).is_err());
    let invoice = draft(db, customer, None, dec!(10));
    assert!(db.invoices().send(invoice.id).is_err());
    credit.reactivate_credit_account(customer).unwrap();
    let hold = credit
        .place_hold(PlaceCreditHold {
            customer_id: customer,
            order_id: None,
            hold_type: CreditHoldType::Manual,
            hold_amount: dec!(0),
            reason: "Review".into(),
            placed_by: None,
        })
        .unwrap();
    assert!(!credit.check_credit(customer, dec!(10)).unwrap().approved);
    assert!(credit.reserve_credit(customer, order, dec!(10)).is_err());
    assert!(credit.charge_credit(customer, order, dec!(10)).is_err());
    assert!(db.orders().create(order_input(customer, dec!(10))).is_err());
    assert!(db.invoices().send(invoice.id).is_err());
    credit
        .release_hold(ReleaseCreditHold {
            hold_id: hold.id,
            released_by: None,
            release_notes: None,
        })
        .unwrap();
    credit.reserve_credit(customer, order, dec!(10)).unwrap();
    credit.charge_credit(customer, order, dec!(10)).unwrap();
    assert_eq!(
        credit.get_credit_account_by_customer(customer).unwrap().unwrap().current_balance,
        dec!(10)
    );
    db.invoices().send(invoice.id).unwrap();
    assert_eq!(credit.check_credit(customer, dec!(0)).unwrap().available_credit, dec!(80));
    assert!(!credit.check_credit(customer, dec!(-1)).unwrap().approved);
    let acc = credit.get_credit_account_by_customer(customer).unwrap().unwrap();
    credit
        .update_credit_account(
            acc.id,
            UpdateCreditAccount { status: Some(CreditAccountStatus::Closed), ..Default::default() },
        )
        .unwrap();
    assert!(credit.reserve_credit(customer, OrderId::new(), dec!(1)).is_err());
    assert!(credit.charge_credit(customer, OrderId::new(), dec!(1)).is_err());
}
pub(crate) fn documents(db: &dyn Database) {
    let customer = account(db);
    let order = db.orders().create(order_input(customer, dec!(60))).unwrap();
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(40));
    assert!(db.orders().create(order_input(customer, dec!(41))).is_err());
    let inv = draft(db, customer, Some(order.id), dec!(60));
    db.invoices().send(inv.id).unwrap();
    assert_eq!(
        db.credit().check_credit(customer, dec!(0)).unwrap().available_credit,
        dec!(40),
        "Invoicing an order must not consume credit twice"
    );
    let other = draft(db, customer, None, dec!(41));
    assert!(db.invoices().send(other.id).is_err());
    assert_eq!(db.invoices().get(other.id).unwrap().unwrap().status, InvoiceStatus::Draft);
    db.invoices()
        .record_payment(
            inv.id,
            RecordInvoicePayment {
                amount: dec!(30),
                payment_id: None,
                payment_method: None,
                reference: None,
                notes: None,
            },
        )
        .unwrap();
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(70));
    db.invoices().send(other.id).unwrap();
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(29));
    assert!(
        db.orders()
            .add_item(
                order.id,
                CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "MORE".into(),
                    name: "More".into(),
                    quantity: 1,
                    unit_price: dec!(30),
                    ..Default::default()
                }
            )
            .is_err()
    );
    let other_customer = customer_without_limit(db);
    assert!(db.orders().create(order_input(other_customer, dec!(10000))).is_ok());
}
fn customer_without_limit(db: &dyn Database) -> CustomerId {
    customer(db)
}
pub(crate) fn currency_and_manual_overlap(db: &dyn Database) {
    let customer = account(db);
    let mut foreign = order_input(customer, dec!(10));
    foreign.currency = Some(CurrencyCode::EUR);
    assert!(db.orders().create(foreign).is_err());
    let order = db.orders().create(order_input(customer, dec!(80))).unwrap();
    db.credit().reserve_credit(customer, order.id, dec!(80)).unwrap();
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(20));
    db.credit().charge_credit(customer, order.id, dec!(80)).unwrap();
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(20));
    assert!(db.credit().reserve_credit(customer, OrderId::new(), dec!(21)).is_err());
    let mut input = CreateInvoice {
        customer_id: customer,
        currency: Some(CurrencyCode::EUR),
        ..Default::default()
    };
    input.items.push(CreateInvoiceItem {
        description: "Foreign".into(),
        quantity: dec!(1),
        unit_price: dec!(1),
        ..Default::default()
    });
    let invoice = db.invoices().create(input).unwrap();
    assert!(db.invoices().send(invoice.id).is_err());
}
pub(crate) fn race(db: &dyn Database) {
    let customer = account(db);
    let gate = std::sync::Barrier::new(2);
    let results = std::thread::scope(|s| {
        let a = s.spawn(|| {
            gate.wait();
            db.orders().create(order_input(customer, dec!(70)))
        });
        let b = s.spawn(|| {
            gate.wait();
            db.orders().create(order_input(customer, dec!(70)))
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1, "{results:?}");
    assert_eq!(db.credit().check_credit(customer, dec!(0)).unwrap().available_credit, dec!(30));
}
