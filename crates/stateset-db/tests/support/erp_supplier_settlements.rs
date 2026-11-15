use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

fn bill(db: &dyn Database, supplier: Uuid, currency: CurrencyCode, amount: Decimal) -> Bill {
    db.accounts_payable()
        .create_bill(CreateBill {
            created_by: None,
            bill_number: None,
            supplier_id: supplier,
            purchase_order_id: None,
            bill_date: None,
            due_date: Utc::now(),
            payment_terms: None,
            currency: Some(currency),
            reference_number: None,
            memo: None,
            items: vec![CreateBillItem {
                description: "Settlement".into(),
                quantity: dec!(1),
                unit_price: amount,
                ..Default::default()
            }],
        })
        .unwrap()
}
pub(crate) fn settlements(db: &dyn Database) {
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: "Settlement supplier".into(),
            ..Default::default()
        })
        .unwrap()
        .id;
    let ap = db.accounts_payable();
    let bill = bill(db, supplier, CurrencyCode::USD, dec!(100));
    ap.approve_bill(bill.id).unwrap();
    let pre = db
        .prepayments()
        .create(CreatePrepayment {
            supplier_id: supplier,
            amount: dec!(60),
            currency: Some(CurrencyCode::USD),
            method: None,
            reference: None,
            memo: None,
        })
        .unwrap();
    let credit = db
        .vendor_credits()
        .create(CreateVendorCredit {
            supplier_id: supplier,
            vendor_return_id: None,
            amount: dec!(40),
            currency: Some(CurrencyCode::USD),
            memo: None,
        })
        .unwrap();
    let apply = ApplyPrepayment {
        target_type: PrepaymentTargetType::Bill,
        target_id: bill.id,
        amount: dec!(60),
    };
    db.prepayments().apply(pre.id, apply).unwrap();
    let partial = ap.get_bill(bill.id).unwrap().unwrap();
    assert_eq!(partial.amount_due, dec!(40));
    assert_eq!(partial.status, BillStatus::PartiallyPaid);
    db.vendor_credits()
        .apply(
            credit.id,
            ApplyVendorCredit {
                target_type: VendorCreditTargetType::Bill,
                target_id: bill.id,
                amount: dec!(40),
            },
        )
        .unwrap();
    let paid = ap.get_bill(bill.id).unwrap().unwrap();
    assert_eq!(paid.amount_due, dec!(0));
    assert_eq!(paid.status, BillStatus::Paid);
    let ca = db.vendor_credits().list_applications(credit.id).unwrap()[0].id;
    db.vendor_credits().reverse_application(credit.id, ca).unwrap();
    assert_eq!(ap.get_bill(bill.id).unwrap().unwrap().amount_due, dec!(40));
    let payment = ap
        .create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(40),
            currency: Some(CurrencyCode::USD),
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(40) }],
        })
        .unwrap();
    assert_eq!(ap.get_bill(bill.id).unwrap().unwrap().amount_due, dec!(0));
    ap.void_payment(payment.id).unwrap();
    assert_eq!(
        ap.get_bill(bill.id).unwrap().unwrap().amount_due,
        dec!(40),
        "Voiding cash must preserve credit settlements"
    );
    let pa = db.prepayments().list_applications(pre.id).unwrap()[0].id;
    db.prepayments().reverse_application(pre.id, pa).unwrap();
    assert_eq!(ap.get_bill(bill.id).unwrap().unwrap().amount_due, dec!(100));
    assert!(db.prepayments().reverse_application(pre.id, pa).is_err());
    let obligation = db
        .payment_obligations()
        .create(CreatePaymentObligation {
            supplier_id: supplier,
            purchase_order_id: None,
            amount: dec!(100),
            currency: Some(CurrencyCode::USD),
            due_date: Utc::now().date_naive(),
            notes: None,
        })
        .unwrap();
    db.prepayments()
        .apply(
            pre.id,
            ApplyPrepayment {
                target_type: PrepaymentTargetType::PaymentObligation,
                target_id: obligation.id.into(),
                amount: dec!(60),
            },
        )
        .unwrap();
    db.vendor_credits()
        .apply(
            credit.id,
            ApplyVendorCredit {
                target_type: VendorCreditTargetType::PaymentObligation,
                target_id: obligation.id.into(),
                amount: dec!(40),
            },
        )
        .unwrap();
    let paid = db.payment_obligations().get(obligation.id).unwrap().unwrap();
    assert_eq!(paid.status, PaymentObligationStatus::Paid);
    assert_eq!(paid.amount_paid, dec!(100));
    let ca = db
        .vendor_credits()
        .list_applications(credit.id)
        .unwrap()
        .into_iter()
        .find(|a| !a.reversed)
        .unwrap()
        .id;
    db.vendor_credits().reverse_application(credit.id, ca).unwrap();
    assert_eq!(
        db.payment_obligations().get(obligation.id).unwrap().unwrap().outstanding(),
        dec!(40)
    );
}

pub(crate) fn invalid_targets(db: &dyn Database) {
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier {
            name: "Settlement supplier".into(),
            ..Default::default()
        })
        .unwrap()
        .id;
    let another = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Other supplier".into(), ..Default::default() })
        .unwrap()
        .id;
    let pre = db
        .prepayments()
        .create(CreatePrepayment {
            supplier_id: supplier,
            amount: dec!(100),
            currency: Some(CurrencyCode::USD),
            method: None,
            reference: None,
            memo: None,
        })
        .unwrap();
    let credit = db
        .vendor_credits()
        .create(CreateVendorCredit {
            supplier_id: supplier,
            vendor_return_id: None,
            amount: dec!(100),
            currency: Some(CurrencyCode::USD),
            memo: None,
        })
        .unwrap();
    let foreign = bill(db, supplier, CurrencyCode::EUR, dec!(100));
    db.accounts_payable().approve_bill(foreign.id).unwrap();
    let wrong_owner = bill(db, another, CurrencyCode::USD, dec!(100));
    db.accounts_payable().approve_bill(wrong_owner.id).unwrap();
    let draft = bill(db, supplier, CurrencyCode::USD, dec!(100));
    let cancelled = bill(db, supplier, CurrencyCode::USD, dec!(100));
    db.accounts_payable().cancel_bill(cancelled.id).unwrap();
    let small = bill(db, supplier, CurrencyCode::USD, dec!(1));
    db.accounts_payable().approve_bill(small.id).unwrap();
    for id in [foreign.id, wrong_owner.id, draft.id, cancelled.id, small.id, Uuid::new_v4()] {
        assert!(
            db.prepayments()
                .apply(
                    pre.id,
                    ApplyPrepayment {
                        target_type: PrepaymentTargetType::Bill,
                        target_id: id,
                        amount: dec!(10)
                    }
                )
                .is_err()
        );
        assert!(
            db.vendor_credits()
                .apply(
                    credit.id,
                    ApplyVendorCredit {
                        target_type: VendorCreditTargetType::Bill,
                        target_id: id,
                        amount: dec!(10)
                    }
                )
                .is_err()
        );
    }
    assert_eq!(db.prepayments().get(pre.id).unwrap().unwrap().remaining, dec!(100));
    assert!(db.prepayments().list_applications(pre.id).unwrap().is_empty());
    assert_eq!(db.vendor_credits().get(credit.id).unwrap().unwrap().remaining, dec!(100));
    assert!(db.vendor_credits().list_applications(credit.id).unwrap().is_empty());
}

pub(crate) fn disputes(db: &dyn Database) {
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Dispute supplier".into(), ..Default::default() })
        .unwrap()
        .id;
    let ap = db.accounts_payable();
    let draft = bill(db, supplier, CurrencyCode::USD, dec!(100));
    ap.dispute_bill(draft.id).unwrap();
    assert_eq!(ap.resolve_bill_dispute(draft.id).unwrap().status, BillStatus::Draft);
    assert!(ap.resolve_bill_dispute(draft.id).is_err());
    ap.approve_bill(draft.id).unwrap();
    let p = ap
        .create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(30),
            currency: Some(CurrencyCode::USD),
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: draft.id, amount: dec!(30) }],
        })
        .unwrap();
    ap.dispute_bill(draft.id).unwrap();
    assert!(ap.cancel_bill(draft.id).is_err());
    assert_eq!(ap.resolve_bill_dispute(draft.id).unwrap().status, BillStatus::PartiallyPaid);
    ap.dispute_bill(draft.id).unwrap();
    ap.void_payment(p.id).unwrap();
    assert_eq!(
        ap.get_bill(draft.id).unwrap().unwrap().status,
        BillStatus::Disputed,
        "Voiding funds must preserve the dispute"
    );
    assert_eq!(ap.resolve_bill_dispute(draft.id).unwrap().status, BillStatus::Approved);
    ap.cancel_bill(draft.id).unwrap();
    assert!(ap.dispute_bill(draft.id).is_err());
    assert!(ap.resolve_bill_dispute(draft.id).is_err());
}

pub(crate) fn currencies(db: &dyn Database) {
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Currency supplier".into(), ..Default::default() })
        .unwrap()
        .id;
    let ap = db.accounts_payable();
    let eur = bill(db, supplier, CurrencyCode::EUR, dec!(100));
    let usd = bill(db, supplier, CurrencyCode::USD, dec!(200));
    ap.approve_bill(eur.id).unwrap();
    ap.approve_bill(usd.id).unwrap();
    let input = CreateBillPayment {
        supplier_id: supplier,
        payment_date: None,
        payment_method: PaymentMethodAP::Check,
        amount: dec!(20),
        currency: None,
        reference_number: None,
        bank_account: None,
        check_number: None,
        memo: None,
        allocations: vec![PaymentAllocationInput { bill_id: eur.id, amount: dec!(20) }],
    };
    let paid = ap.create_payment(input.clone()).unwrap();
    assert_eq!(paid.currency, CurrencyCode::EUR);
    let mut wrong = input.clone();
    wrong.currency = Some(CurrencyCode::USD);
    assert!(ap.create_payment(wrong).is_err());
    let mut mixed = input;
    mixed.amount = dec!(40);
    mixed.allocations.push(PaymentAllocationInput { bill_id: usd.id, amount: dec!(20) });
    assert!(ap.create_payment(mixed).is_err());
    assert_eq!(ap.get_bill(eur.id).unwrap().unwrap().amount_due, dec!(80));
    assert_eq!(ap.get_bill(usd.id).unwrap().unwrap().amount_due, dec!(200));
    let run = CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Check,
        bill_ids: vec![eur.id, usd.id],
        notes: None,
        created_by: Some("creator".into()),
    };
    assert!(ap.create_payment_run(run.clone()).is_err());
    for (id, currency, amount) in
        [(eur.id, CurrencyCode::EUR, dec!(80)), (usd.id, CurrencyCode::USD, dec!(200))]
    {
        let mut input = run.clone();
        input.bill_ids = vec![id];
        let run = ap.create_payment_run(input).unwrap();
        assert_eq!(run.currency, Some(currency));
        assert_eq!(run.total_amount, amount);
        ap.approve_payment_run(run.id, "approver").unwrap();
        let run = ap.process_payment_run(run.id).unwrap();
        assert_eq!(run.currency, Some(currency));
        assert_eq!(run.status, PaymentRunStatus::Completed);
        assert_eq!(ap.get_bill(id).unwrap().unwrap().amount_due, Decimal::ZERO);
    }
    let payments = ap
        .list_payments(BillPaymentFilter { supplier_id: Some(supplier), ..Default::default() })
        .unwrap();
    assert_eq!(payments.len(), 3);
    assert_eq!(
        payments
            .iter()
            .filter(|p| p.currency == CurrencyCode::EUR)
            .map(|p| p.amount)
            .sum::<Decimal>(),
        dec!(100)
    );
}

pub(crate) fn refund_and_dashboard(db: &dyn Database) {
    use chrono::Duration;
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Refund supplier".into(), ..Default::default() })
        .unwrap()
        .id;
    let target = bill(db, supplier, CurrencyCode::USD, dec!(100));
    db.accounts_payable().approve_bill(target.id).unwrap();
    let pre = db
        .prepayments()
        .create(CreatePrepayment {
            supplier_id: supplier,
            amount: dec!(100),
            currency: None,
            method: None,
            reference: None,
            memo: None,
        })
        .unwrap();
    db.prepayments()
        .apply(
            pre.id,
            ApplyPrepayment {
                target_type: PrepaymentTargetType::Bill,
                target_id: target.id,
                amount: dec!(40),
            },
        )
        .unwrap();
    let refunded = db.prepayments().refund(pre.id).unwrap();
    assert_eq!(refunded.refunded_amount, dec!(60));
    assert_eq!(refunded.applied_amount(), dec!(40));
    assert!(refunded.refunded_at.is_some());
    let retry = db.prepayments().refund(pre.id).unwrap();
    assert_eq!(retry.refunded_amount, dec!(60));
    assert_eq!(retry.refunded_at, refunded.refunded_at);
    assert_eq!(db.accounts_payable().get_bill(target.id).unwrap().unwrap().amount_due, dec!(60));
    let today = Utc::now().date_naive();
    for i in 0..1001 {
        db.payment_obligations()
            .create(CreatePaymentObligation {
                supplier_id: supplier,
                purchase_order_id: None,
                amount: dec!(0.10),
                currency: None,
                due_date: if i < 501 {
                    today - Duration::days(1)
                } else {
                    today + Duration::days(1)
                },
                notes: None,
            })
            .unwrap();
    }
    let dash = db.payment_obligations().dashboard(today).unwrap();
    assert_eq!(dash.open_count, 1001);
    assert_eq!(dash.overdue_count, 501);
    assert_eq!(dash.total_outstanding, dec!(100.10));
    assert_eq!(dash.overdue_amount, dec!(50.10));
}
pub(crate) fn bill_matching(db: &dyn Database) {
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Match supplier".into(), ..Default::default() })
        .unwrap()
        .id;
    let po = db
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier,
            items: vec![CreatePurchaseOrderItem {
                sku: "MATCH-SKU".into(),
                name: "Match goods".into(),
                quantity: dec!(2),
                unit_cost: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.purchase_orders().submit_for_approval(po.id).unwrap();
    db.purchase_orders().approve(po.id, "Buyer").unwrap();
    db.purchase_orders().send(po.id).unwrap();
    let input = CreateBill {
        supplier_id: supplier,
        purchase_order_id: Some(po.id.into()),
        due_date: Utc::now(),
        items: vec![CreateBillItem {
            description: "Match goods".into(),
            quantity: dec!(2),
            unit_price: dec!(10),
            po_line_id: Some(po.items[0].id),
            ..Default::default()
        }],
        ..Default::default()
    };
    let first = db.accounts_payable().create_bill(input.clone()).unwrap();
    assert!(db.accounts_payable().approve_bill(first.id).is_err());
    assert!(db.accounts_payable().approve_bill_with_override(first.id, "  ").is_err());
    assert_eq!(
        db.accounts_payable().get_bill(first.id).unwrap().unwrap().status,
        BillStatus::Draft
    );
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: "MATCH-WH".into(),
            name: "Receiving".into(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    let receipt = db.receiving().create_receipt_from_po(po.id.into(), warehouse.id).unwrap();
    let item = db.receiving().get_receipt_items(receipt.id).unwrap()[0].id;
    db.receiving()
        .receive_items(ReceiveItems {
            receipt_id: receipt.id,
            receiving_location_id: Some(location.id),
            received_by: None,
            items: vec![ReceiveItemLine {
                receipt_item_id: item,
                quantity_received: dec!(2),
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
        })
        .unwrap();
    let approved = db.accounts_payable().approve_bill(first.id).unwrap();
    assert_eq!(approved.status, BillStatus::Approved);
    assert!(approved.match_override_reason.is_none());
    let duplicate = db.accounts_payable().create_bill(input).unwrap();
    assert!(
        db.accounts_payable().approve_bill(duplicate.id).is_err(),
        "Cumulative bills cannot consume the same receipt twice"
    );
    let approved = db
        .accounts_payable()
        .approve_bill_with_override(duplicate.id, "Accepted replacement invoice variance")
        .unwrap();
    assert_eq!(
        approved.match_override_reason.as_deref(),
        Some("Accepted replacement invoice variance")
    );
}

pub(crate) fn maker_checker(db: &dyn Database) {
    let ap = db.accounts_payable();
    let bill = ap
        .create_bill(CreateBill {
            created_by: Some("maker".into()),
            supplier_id: Uuid::new_v4(),
            due_date: Utc::now(),
            items: vec![CreateBillItem {
                description: "Approval".into(),
                quantity: dec!(1),
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    assert!(ap.approve_bill(bill.id).is_err());
    assert!(ap.approve_bill_with_override(bill.id, "override").is_err());
    assert!(ap.approve_bill_as(bill.id, "maker", None).is_err());
    assert!(ap.approve_bill_as(bill.id, " maker ", Some("override")).is_err());
    assert_eq!(ap.get_bill(bill.id).unwrap().unwrap().status, BillStatus::Draft);
    let approved = ap.approve_bill_as(bill.id, "checker", None).unwrap();
    assert_eq!(approved.created_by.as_deref(), Some("maker"));
    assert_eq!(approved.approved_by.as_deref(), Some("checker"));
    let run = ap
        .create_payment_run(CreatePaymentRun {
            created_by: Some("maker".into()),
            payment_date: Utc::now(),
            payment_method: PaymentMethodAP::Check,
            bill_ids: vec![bill.id],
            notes: None,
        })
        .unwrap();
    assert!(ap.approve_payment_run(run.id, " maker ").is_err());
    assert!(ap.approve_payment_run(run.id, " ").is_err());
    assert_eq!(
        ap.approve_payment_run(run.id, "checker").unwrap().approved_by.as_deref(),
        Some("checker")
    );
}
