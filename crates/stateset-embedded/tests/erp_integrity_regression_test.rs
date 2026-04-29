#![cfg(feature = "sqlite")]

use chrono::Utc;
use rust_decimal_macros::dec;
use stateset_core::{
    CurrencyCode, JournalEntry, JournalEntryLine, JournalEntrySource, JournalEntryStatus,
    JournalEntryType, ReceivePurchaseOrderItem,
};
use stateset_embedded::{
    Commerce, CostLayerSource, CreateBill, CreateBillItem, CreateCostLayer, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateSupplier, IssueCostLayers, PayBill, PaymentMethodAP,
    PaymentTerms, ReceivePurchaseOrderItems,
};
use uuid::Uuid;

#[test]
fn pay_bill_rejects_overpayment() {
    let commerce = Commerce::new(":memory:").expect("create commerce");
    let supplier_id = Uuid::new_v4();

    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id,
            items: vec![CreateBillItem {
                description: "Inventory parts".into(),
                quantity: dec!(2),
                unit_price: dec!(50.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create bill");

    let approved = commerce.accounts_payable().approve_bill(bill.id).expect("approve bill");

    let err = commerce
        .accounts_payable()
        .pay_bill(
            approved.id,
            PayBill {
                amount: approved.amount_due + dec!(1.00),
                payment_method: PaymentMethodAP::Check,
                ..Default::default()
            },
        )
        .expect_err("overpayment must fail");

    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));
}

#[test]
fn purchase_order_receive_rejects_cross_order_item() {
    let commerce = Commerce::new(":memory:").expect("create commerce");

    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: "Integrity Supplier".into(),
            payment_terms: Some(PaymentTerms::Net30),
            ..Default::default()
        })
        .expect("create supplier");

    let po_one = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "PART-A".into(),
                name: "Part A".into(),
                quantity: dec!(5),
                unit_cost: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create po 1");

    let po_two = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "PART-B".into(),
                name: "Part B".into(),
                quantity: dec!(5),
                unit_cost: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create po 2");

    let po_one = commerce.purchase_orders().submit(po_one.id.into()).expect("submit po 1");
    let po_one =
        commerce.purchase_orders().approve(po_one.id.into(), "tester").expect("approve po 1");
    let po_one = commerce.purchase_orders().send(po_one.id.into()).expect("send po 1");

    let err = commerce
        .purchase_orders()
        .receive(
            po_one.id.into(),
            ReceivePurchaseOrderItems {
                items: vec![ReceivePurchaseOrderItem {
                    item_id: po_two.items[0].id,
                    quantity_received: dec!(1),
                    notes: None,
                }],
                notes: None,
            },
        )
        .expect_err("cross-order item receipt must fail");

    assert!(matches!(err, stateset_embedded::CommerceError::NotFound));
}

#[test]
fn issue_fifo_rolls_back_when_quantity_exceeds_layers() {
    let commerce = Commerce::new(":memory:").expect("create commerce");

    commerce
        .cost_accounting()
        .create_cost_layer(CreateCostLayer {
            sku: "COST-ROLLBACK-SKU".into(),
            quantity: dec!(5),
            unit_cost: dec!(12.00),
            source_type: CostLayerSource::Opening,
            source_id: None,
            lot_id: None,
            location_id: None,
        })
        .expect("create opening cost layer");

    let err = commerce
        .cost_accounting()
        .issue_fifo(IssueCostLayers {
            sku: "COST-ROLLBACK-SKU".into(),
            quantity: dec!(8),
            reference_type: Some("test".into()),
            reference_id: None,
            notes: None,
        })
        .expect_err("issuing more than available must fail");

    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));

    let remaining = commerce
        .cost_accounting()
        .get_layers_remaining("COST-ROLLBACK-SKU")
        .expect("get remaining layers");
    assert_eq!(remaining, dec!(5));
}

#[test]
fn has_stock_rejects_negative_and_unknown_sku() {
    let commerce = Commerce::new(":memory:").expect("create commerce");

    let neg_err = commerce
        .inventory()
        .has_stock("MISSING-SKU", dec!(-1))
        .expect_err("negative stock check must fail");
    assert!(matches!(neg_err, stateset_embedded::CommerceError::ValidationError(_)));

    let missing_err = commerce
        .inventory()
        .has_stock("MISSING-SKU", dec!(1))
        .expect_err("unknown sku stock check must fail");
    assert!(matches!(missing_err, stateset_embedded::CommerceError::NotFound));
}

#[test]
fn create_journal_entry_rejects_line_with_debit_and_credit() {
    let now = Utc::now();
    let line = JournalEntryLine {
        id: Uuid::new_v4(),
        journal_entry_id: Uuid::new_v4(),
        line_number: 1,
        account_id: Uuid::new_v4(),
        account_number: Some("1010".into()),
        account_name: Some("Cash".into()),
        description: Some("Invalid line".into()),
        debit_amount: dec!(50),
        credit_amount: dec!(50),
        currency: CurrencyCode::USD,
        reference_type: None,
        reference_id: None,
        created_at: now,
    };

    let entry = JournalEntry {
        id: Uuid::new_v4(),
        entry_number: "JE-TEST-001".into(),
        entry_date: now.date_naive(),
        period_id: Uuid::new_v4(),
        entry_type: JournalEntryType::Standard,
        source: JournalEntrySource::Manual,
        source_document_type: None,
        source_document_id: None,
        description: "Invalid line should not post".into(),
        total_debits: dec!(50),
        total_credits: dec!(50),
        is_balanced: true,
        status: JournalEntryStatus::Draft,
        posted_at: None,
        posted_by: None,
        reversed_entry_id: None,
        reversing_entry_id: None,
        lines: vec![line],
        created_at: now,
        updated_at: now,
    };

    assert!(!entry.can_post());
}
