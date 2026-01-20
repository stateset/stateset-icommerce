//! Tests for newly added commerce modules

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer,
    // Product types
    CreateProduct, CreateProductVariant,
    // Warehouse types
    CreateWarehouse, CreateLocation, CreateZone, WarehouseType, LocationType,
    // Lot types
    CreateLot,
    // Serial types
    CreateSerialNumber, SerialStatus, UpdateSerialNumber,
    // Receiving types
    CreateReceipt, ReceiptType,
    // Accounts Payable types
    CreateBill, CreateBillItem, BillStatus,
    // Cost Accounting types
    CreateCostLayer, CostLayerSource, SetItemCost, CostMethod,
    // Credit types
    CreateCreditAccount, RiskRating,
    // Backorder types
    CreateBackorder, BackorderPriority,
    // Shipment types
    CreateShipment, CreateShipmentItem, ShipmentStatus, ShippingCarrier,
    // Warranty types
    ClaimResolution, ClaimStatus, CreateWarranty, CreateWarrantyClaim, UpdateWarrantyClaim,
    WarrantyStatus, WarrantyType,
    // Purchase Order types
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PaymentTerms, PurchaseOrderStatus,
    // Invoice types
    CreateInvoice, CreateInvoiceItem, InvoiceStatus, RecordInvoicePayment,
};
use uuid::Uuid;

// ============================================================================
// Warehouse Tests
// ============================================================================

#[test]
fn test_warehouse_create_and_list() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
        code: "WH-TEST-001".into(),
        name: "Test Warehouse".into(),
        warehouse_type: WarehouseType::Distribution,
        ..Default::default()
    }).expect("Failed to create warehouse");

    assert_eq!(warehouse.code, "WH-TEST-001");
    assert_eq!(warehouse.name, "Test Warehouse");

    let warehouses = commerce.warehouse().list_warehouses(Default::default())
        .expect("Failed to list warehouses");
    assert!(warehouses.iter().any(|w| w.id == warehouse.id));
}

#[test]
fn test_location_create_and_get() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
        code: "WH-LOC-001".into(),
        name: "Location Test Warehouse".into(),
        warehouse_type: WarehouseType::Distribution,
        ..Default::default()
    }).expect("Failed to create warehouse");

    let location = commerce.warehouse().create_location(CreateLocation {
        warehouse_id: warehouse.id,
        location_type: LocationType::Pick,
        zone: Some("A".into()),
        aisle: Some("01".into()),
        rack: Some("02".into()),
        bin: Some("03".into()),
        ..Default::default()
    }).expect("Failed to create location");

    assert_eq!(location.warehouse_id, warehouse.id);
    assert!(location.code.contains("A"));

    let fetched = commerce.warehouse().get_location(location.id)
        .expect("Failed to get location")
        .expect("Location not found");
    assert_eq!(fetched.id, location.id);
}

#[test]
fn test_zone_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
        code: "WH-ZONE-001".into(),
        name: "Zone Test Warehouse".into(),
        warehouse_type: WarehouseType::Distribution,
        ..Default::default()
    }).expect("Failed to create warehouse");

    let zone = commerce.warehouse().create_zone(CreateZone {
        warehouse_id: warehouse.id,
        code: "ZONE-A".into(),
        name: "Zone A".into(),
        ..Default::default()
    }).expect("Failed to create zone");

    assert_eq!(zone.code, "ZONE-A");
    assert_eq!(zone.warehouse_id, warehouse.id);
}

// ============================================================================
// Lot Tests
// ============================================================================

#[test]
fn test_lot_create_and_get() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let lot = commerce.lots().create(CreateLot {
        lot_number: Some("LOT-TEST-001".into()),
        sku: "LOT-SKU-001".into(),
        quantity: dec!(1000),
        ..Default::default()
    }).expect("Failed to create lot");

    assert_eq!(lot.lot_number, "LOT-TEST-001");
    assert_eq!(lot.quantity_remaining, dec!(1000));

    // Get the lot
    let fetched = commerce.lots().get(lot.id)
        .expect("Failed to get lot")
        .expect("Lot not found");
    assert_eq!(fetched.id, lot.id);
}

// ============================================================================
// Serial Number Tests
// ============================================================================

#[test]
fn test_serial_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let serial = commerce.serials().create(CreateSerialNumber {
        serial: Some("SN-TEST-001".into()),
        sku: "SERIAL-SKU-001".into(),
        ..Default::default()
    }).expect("Failed to create serial");

    assert_eq!(serial.serial, "SN-TEST-001");
    assert_eq!(serial.status, SerialStatus::Available);
}

#[test]
fn test_serial_lookup_with_related_data() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    commerce
        .products()
        .create(CreateProduct {
            name: "Lookup Widget".into(),
            variants: Some(vec![CreateProductVariant {
                sku: "SERIAL-LOOKUP-SKU".into(),
                name: Some("Lookup Variant".into()),
                price: dec!(99.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect("Failed to create product");

    let lot = commerce
        .lots()
        .create(CreateLot {
            lot_number: Some("LOT-LOOKUP-001".into()),
            sku: "SERIAL-LOOKUP-SKU".into(),
            quantity: dec!(10),
            ..Default::default()
        })
        .expect("Failed to create lot");

    let serial = commerce
        .serials()
        .create(CreateSerialNumber {
            serial: Some("SN-LOOKUP-001".into()),
            sku: "SERIAL-LOOKUP-SKU".into(),
            lot_id: Some(lot.id),
            ..Default::default()
        })
        .expect("Failed to create serial");

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: "serial-lookup@example.com".into(),
            first_name: "Serial".into(),
            last_name: "Lookup".into(),
            ..Default::default()
        })
        .expect("Failed to create customer");

    let warranty = commerce
        .warranties()
        .create(CreateWarranty {
            customer_id: customer.id,
            sku: Some("SERIAL-LOOKUP-SKU".into()),
            serial_number: Some(serial.serial.clone()),
            warranty_type: Some(WarrantyType::Standard),
            duration_months: Some(12),
            ..Default::default()
        })
        .expect("Failed to create warranty");

    commerce
        .serials()
        .update(
            serial.id,
            UpdateSerialNumber {
                warranty_id: Some(warranty.id),
                ..Default::default()
            },
        )
        .expect("Failed to update serial");

    let lookup = commerce
        .serials()
        .lookup("SN-LOOKUP-001")
        .expect("Failed to lookup serial")
        .expect("Serial not found");

    assert_eq!(lookup.product_name.as_deref(), Some("Lookup Widget"));
    let lookup_lot = lookup.lot.expect("Lot not loaded");
    assert_eq!(lookup_lot.id, lot.id);
    let warranty_status = lookup.warranty_status.expect("Warranty status not loaded");
    assert_eq!(warranty_status.warranty_id, warranty.id);
    assert!(warranty_status.is_active);
    assert_eq!(warranty_status.coverage_type.as_deref(), Some("standard"));
}

// ============================================================================
// Receiving Tests
// ============================================================================

#[test]
fn test_receipt_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a warehouse first (receipts require a warehouse_id)
    let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
        code: "WH-RECV-001".into(),
        name: "Receiving Warehouse".into(),
        warehouse_type: WarehouseType::Distribution,
        ..Default::default()
    }).expect("Failed to create warehouse");

    let receipt = commerce.receiving().create_receipt(CreateReceipt {
        receipt_type: ReceiptType::PurchaseOrder,
        warehouse_id: warehouse.id,
        ..Default::default()
    }).expect("Failed to create receipt");

    assert_eq!(receipt.receipt_type, ReceiptType::PurchaseOrder);

    // Get the receipt
    let fetched = commerce.receiving().get_receipt(receipt.id)
        .expect("Failed to get receipt")
        .expect("Receipt not found");
    assert_eq!(fetched.id, receipt.id);
}

// ============================================================================
// Accounts Payable Tests
// ============================================================================

#[test]
fn test_bill_create_and_list() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let supplier_id = Uuid::new_v4();

    let bill = commerce.accounts_payable().create_bill(CreateBill {
        supplier_id,
        items: vec![
            CreateBillItem {
                description: "Widget supplies".into(),
                quantity: dec!(100),
                unit_price: dec!(10.00),
                ..Default::default()
            }
        ],
        ..Default::default()
    }).expect("Failed to create bill");

    assert_eq!(bill.status, BillStatus::Draft);
    assert_eq!(bill.total_amount, dec!(1000.00));

    let bills = commerce.accounts_payable().list_bills(Default::default())
        .expect("Failed to list bills");
    assert!(bills.iter().any(|b| b.id == bill.id));
}

// ============================================================================
// Cost Accounting Tests
// ============================================================================

#[test]
fn test_item_cost_set_and_get() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Set item cost
    let item_cost = commerce.cost_accounting().set_item_cost(SetItemCost {
        sku: "COST-SKU-001".into(),
        cost_method: Some(CostMethod::Standard),
        standard_cost: Some(dec!(25.00)),
        ..Default::default()
    }).expect("Failed to set item cost");

    assert_eq!(item_cost.sku, "COST-SKU-001");
    assert_eq!(item_cost.cost_method, CostMethod::Standard);
    assert_eq!(item_cost.standard_cost, dec!(25.00));

    // Get item cost
    let fetched = commerce.cost_accounting().get_item_cost("COST-SKU-001")
        .expect("Failed to get item cost")
        .expect("Item cost not found");
    assert_eq!(fetched.sku, "COST-SKU-001");
}

#[test]
fn test_cost_layer_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let layer = commerce.cost_accounting().create_cost_layer(CreateCostLayer {
        sku: "LAYER-SKU-001".into(),
        quantity: dec!(100),
        unit_cost: dec!(15.50),
        source_type: CostLayerSource::Purchase,
        source_id: None,
        lot_id: None,
        location_id: None,
    }).expect("Failed to create cost layer");

    assert_eq!(layer.sku, "LAYER-SKU-001");
    assert_eq!(layer.quantity, dec!(100));
    assert_eq!(layer.unit_cost, dec!(15.50));
}

// ============================================================================
// Credit Tests
// ============================================================================

#[test]
fn test_credit_account_create_and_get() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a customer first
    let customer = commerce.customers().create(CreateCustomer {
        email: "credit@example.com".into(),
        first_name: "Credit".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let account = commerce.credit().create_credit_account(CreateCreditAccount {
        customer_id: customer.id,
        credit_limit: dec!(5000.00),
        risk_rating: Some(RiskRating::Low),
        ..Default::default()
    }).expect("Failed to create credit account");

    assert_eq!(account.customer_id, customer.id);
    assert_eq!(account.credit_limit, dec!(5000.00));

    // Get account
    let fetched = commerce.credit().get_credit_account(account.id)
        .expect("Failed to get account")
        .expect("Account not found");
    assert_eq!(fetched.id, account.id);
}

// ============================================================================
// Backorder Tests
// ============================================================================

#[test]
fn test_backorder_create_and_list() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a customer
    let customer = commerce.customers().create(CreateCustomer {
        email: "backorder@example.com".into(),
        first_name: "Backorder".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    // Create an order for the backorder
    let order = commerce.orders().create(stateset_embedded::CreateOrder {
        customer_id: customer.id,
        items: vec![
            stateset_embedded::CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "BO-SKU-001".into(),
                quantity: 10,
                unit_price: dec!(25.00),
                name: "Backorder Item".into(),
                ..Default::default()
            }
        ],
        ..Default::default()
    }).expect("Failed to create order");

    let backorder = commerce.backorder().create_backorder(CreateBackorder {
        order_id: order.id,
        customer_id: customer.id,
        sku: "BO-SKU-001".into(),
        quantity: dec!(50),
        priority: Some(BackorderPriority::High),
        order_line_id: None,
        expected_date: None,
        promised_date: None,
        source_location_id: None,
        notes: None,
    }).expect("Failed to create backorder");

    assert_eq!(backorder.sku, "BO-SKU-001");
    assert_eq!(backorder.priority, BackorderPriority::High);

    let backorders = commerce.backorder().list_backorders(Default::default())
        .expect("Failed to list backorders");
    assert!(backorders.iter().any(|b| b.id == backorder.id));
}

// ============================================================================
// Quality Tests
// ============================================================================

#[test]
fn test_quality_inspection_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Use a dummy reference ID for the inspection
    let reference_id = Uuid::new_v4();

    let inspection = commerce.quality().create_inspection(stateset_embedded::CreateInspection {
        inspection_type: stateset_embedded::InspectionType::Incoming,
        reference_type: "receipt".into(),
        reference_id,
        inspector_id: Some("QC-001".into()),
        scheduled_at: None,
        notes: Some("Incoming quality check".into()),
        items: vec![stateset_embedded::CreateInspectionItem {
            sku: "QC-SKU-001".into(),
            lot_number: None,
            serial_number: None,
            quantity_to_inspect: dec!(100),
        }],
    }).expect("Failed to create inspection");

    assert_eq!(inspection.reference_type, "receipt");
    assert_eq!(inspection.reference_id, reference_id);

    // List inspections
    let inspections = commerce.quality().list_inspections(Default::default())
        .expect("Failed to list inspections");
    assert!(inspections.iter().any(|i| i.id == inspection.id));
}

// ============================================================================
// General Ledger Tests
// ============================================================================

#[test]
fn test_gl_account_create_and_list() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let account = commerce.general_ledger().create_account(stateset_embedded::CreateGlAccount {
        account_number: "1000".into(),
        name: "Cash".into(),
        account_type: stateset_embedded::AccountType::Asset,
        description: Some("Main cash account".into()),
        account_sub_type: None,
        parent_account_id: None,
        is_header: Some(false),
        is_posting: Some(true),
        currency: None,
    }).expect("Failed to create GL account");

    assert_eq!(account.account_number, "1000");
    assert_eq!(account.name, "Cash");

    let accounts = commerce.general_ledger().list_accounts(Default::default())
        .expect("Failed to list accounts");
    assert!(accounts.iter().any(|a| a.id == account.id));
}

// ============================================================================
// Accounts Receivable Tests
// ============================================================================

#[test]
fn test_ar_aging_summary() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // AR aging should work even with no invoices
    let summary = commerce.accounts_receivable().get_aging_summary()
        .expect("Failed to get aging summary");

    assert_eq!(summary.total, dec!(0));
    assert_eq!(summary.current, dec!(0));
}

// ============================================================================
// Fulfillment Tests
// ============================================================================

#[test]
fn test_fulfillment_wave_create() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a warehouse first
    let warehouse = commerce.warehouse().create_warehouse(CreateWarehouse {
        code: "WH-FULFILL-001".into(),
        name: "Fulfillment Warehouse".into(),
        warehouse_type: WarehouseType::Distribution,
        ..Default::default()
    }).expect("Failed to create warehouse");

    let wave = commerce.fulfillment().create_wave(stateset_embedded::CreateWave {
        warehouse_id: warehouse.id,
        order_ids: vec![],
        priority: Some(1),
        notes: Some("Test Wave".into()),
        created_by: None,
    }).expect("Failed to create wave");

    assert_eq!(wave.warehouse_id, warehouse.id);

    // List waves
    let waves = commerce.fulfillment().list_waves(Default::default())
        .expect("Failed to list waves");
    assert!(waves.iter().any(|w| w.id == wave.id));
}

// ============================================================================
// Shipments Tests
// ============================================================================

#[test]
fn test_shipment_tracking_flow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "ship@example.com".into(),
        first_name: "Ship".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let order = commerce.orders().create(stateset_embedded::CreateOrder {
        customer_id: customer.id,
        items: vec![
            stateset_embedded::CreateOrderItem {
                product_id: Uuid::new_v4(),
                sku: "SHIP-SKU-001".into(),
                name: "Shipment Item".into(),
                quantity: 1,
                unit_price: dec!(19.99),
                ..Default::default()
            }
        ],
        ..Default::default()
    }).expect("Failed to create order");

    let tracking_number = "1Z999AA10123456784".to_string();

    let shipment = commerce.shipments().create(CreateShipment {
        order_id: order.id,
        carrier: Some(ShippingCarrier::Ups),
        recipient_name: "Alice Smith".into(),
        shipping_address: "123 Main St, City, ST 12345".into(),
        items: Some(vec![CreateShipmentItem {
            sku: "SHIP-SKU-001".into(),
            name: "Shipment Item".into(),
            quantity: 1,
            ..Default::default()
        }]),
        ..Default::default()
    }).expect("Failed to create shipment");

    assert_eq!(shipment.status, ShipmentStatus::Pending);

    let shipped = commerce.shipments().ship(shipment.id, Some(tracking_number.clone()))
        .expect("Failed to ship");
    assert_eq!(shipped.status, ShipmentStatus::Shipped);
    assert!(shipped.tracking_url.as_ref().map_or(false, |url| url.contains("ups.com")));

    let delivered = commerce.shipments().mark_delivered(shipment.id)
        .expect("Failed to mark delivered");
    assert_eq!(delivered.status, ShipmentStatus::Delivered);

    let by_tracking = commerce.shipments().get_by_tracking(&tracking_number)
        .expect("Failed to fetch by tracking")
        .expect("Shipment not found by tracking");
    assert_eq!(by_tracking.id, shipment.id);
}

// ============================================================================
// Warranty Tests
// ============================================================================

#[test]
fn test_warranty_claim_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "warranty@example.com".into(),
        first_name: "Warranty".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let warranty = commerce.warranties().create(CreateWarranty {
        customer_id: customer.id,
        sku: Some("WRN-SKU-001".into()),
        warranty_type: Some(WarrantyType::Standard),
        duration_months: Some(12),
        ..Default::default()
    }).expect("Failed to create warranty");

    assert_eq!(warranty.status, WarrantyStatus::Active);

    let fetched = commerce.warranties().get_by_number(&warranty.warranty_number)
        .expect("Failed to get warranty by number")
        .expect("Warranty not found");
    assert_eq!(fetched.id, warranty.id);

    let claim = commerce.warranties().create_claim(CreateWarrantyClaim {
        warranty_id: warranty.id,
        issue_description: "Stopped working after 3 months".into(),
        contact_email: Some("warranty@example.com".into()),
        ..Default::default()
    }).expect("Failed to create warranty claim");

    assert_eq!(claim.status, ClaimStatus::Submitted);

    let approved = commerce.warranties().approve_claim(claim.id)
        .expect("Failed to approve claim");
    assert_eq!(approved.status, ClaimStatus::Approved);

    let completed = commerce.warranties().complete_claim(claim.id, ClaimResolution::Replacement)
        .expect("Failed to complete claim");
    assert_eq!(completed.status, ClaimStatus::Completed);
    assert_eq!(completed.resolution, ClaimResolution::Replacement);
}

#[test]
fn test_warranty_claim_invalid_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "warranty-invalid@example.com".into(),
        first_name: "Warranty".into(),
        last_name: "Invalid".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let warranty = commerce.warranties().create(CreateWarranty {
        customer_id: customer.id,
        sku: Some("WRN-SKU-002".into()),
        warranty_type: Some(WarrantyType::Standard),
        duration_months: Some(12),
        ..Default::default()
    }).expect("Failed to create warranty");

    let claim = commerce.warranties().create_claim(CreateWarrantyClaim {
        warranty_id: warranty.id,
        issue_description: "Battery failure".into(),
        ..Default::default()
    }).expect("Failed to create warranty claim");

    let result = commerce.warranties().complete_claim(claim.id, ClaimResolution::Replacement);
    assert!(result.is_err());

    let result = commerce.warranties().deny_claim(claim.id, "");
    assert!(result.is_err());

    let approved = commerce.warranties().approve_claim(claim.id)
        .expect("Failed to approve claim");
    assert_eq!(approved.status, ClaimStatus::Approved);

    let result = commerce.warranties().approve_claim(claim.id);
    assert!(result.is_err());

    let result = commerce.warranties().deny_claim(claim.id, "Late filing");
    assert!(result.is_err());

    let result = commerce.warranties().complete_claim(claim.id, ClaimResolution::None);
    assert!(result.is_err());

    let completed = commerce.warranties().complete_claim(claim.id, ClaimResolution::Repair)
        .expect("Failed to complete claim");
    assert_eq!(completed.status, ClaimStatus::Completed);

    let result = commerce.warranties().cancel_claim(claim.id);
    assert!(result.is_err());
}

#[test]
fn test_warranty_status_transition_guards() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "warranty-transition@example.com".into(),
        first_name: "Warranty".into(),
        last_name: "Transition".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let new_customer = commerce.customers().create(CreateCustomer {
        email: "warranty-new-owner@example.com".into(),
        first_name: "New".into(),
        last_name: "Owner".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let warranty = commerce.warranties().create(CreateWarranty {
        customer_id: customer.id,
        sku: Some("WRN-SKU-003".into()),
        warranty_type: Some(WarrantyType::Standard),
        duration_months: Some(12),
        ..Default::default()
    }).expect("Failed to create warranty");

    let expired = commerce.warranties().expire(warranty.id)
        .expect("Failed to expire warranty");
    assert_eq!(expired.status, WarrantyStatus::Expired);

    let result = commerce.warranties().void(warranty.id);
    assert!(result.is_err());

    let result = commerce.warranties().transfer(warranty.id, new_customer.id);
    assert!(result.is_err());
}

#[test]
fn test_warranty_update_claim_transitions() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "warranty-update@example.com".into(),
        first_name: "Warranty".into(),
        last_name: "Update".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let warranty = commerce.warranties().create(CreateWarranty {
        customer_id: customer.id,
        sku: Some("WRN-SKU-004".into()),
        warranty_type: Some(WarrantyType::Standard),
        duration_months: Some(12),
        ..Default::default()
    }).expect("Failed to create warranty");

    let claim = commerce.warranties().create_claim(CreateWarrantyClaim {
        warranty_id: warranty.id,
        issue_description: "Screen flicker".into(),
        ..Default::default()
    }).expect("Failed to create warranty claim");

    let under_review = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::UnderReview),
            ..Default::default()
        },
    ).expect("Failed to update claim to under_review");
    assert_eq!(under_review.status, ClaimStatus::UnderReview);

    let result = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Completed),
            resolution: Some(ClaimResolution::Refund),
            ..Default::default()
        },
    );
    assert!(result.is_err());

    let approved = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Approved),
            ..Default::default()
        },
    ).expect("Failed to update claim to approved");
    assert_eq!(approved.status, ClaimStatus::Approved);
    assert!(approved.approved_at.is_some());

    let in_progress = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::InProgress),
            ..Default::default()
        },
    ).expect("Failed to update claim to in_progress");
    assert_eq!(in_progress.status, ClaimStatus::InProgress);

    let result = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Completed),
            resolution: Some(ClaimResolution::None),
            ..Default::default()
        },
    );
    assert!(result.is_err());

    let completed = commerce.warranties().update_claim(
        claim.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Completed),
            resolution: Some(ClaimResolution::Refund),
            ..Default::default()
        },
    ).expect("Failed to complete claim");
    assert_eq!(completed.status, ClaimStatus::Completed);
    assert_eq!(completed.resolution, ClaimResolution::Refund);
    assert!(completed.resolved_at.is_some());

    let claim2 = commerce.warranties().create_claim(CreateWarrantyClaim {
        warranty_id: warranty.id,
        issue_description: "Battery drain".into(),
        ..Default::default()
    }).expect("Failed to create second claim");

    let result = commerce.warranties().update_claim(
        claim2.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::UnderReview),
            resolution: Some(ClaimResolution::Denied),
            ..Default::default()
        },
    );
    assert!(result.is_err());

    let result = commerce.warranties().update_claim(
        claim2.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Denied),
            ..Default::default()
        },
    );
    assert!(result.is_err());

    let denied = commerce.warranties().update_claim(
        claim2.id,
        UpdateWarrantyClaim {
            status: Some(ClaimStatus::Denied),
            denial_reason: Some("Out of coverage".into()),
            ..Default::default()
        },
    ).expect("Failed to deny claim");
    assert_eq!(denied.status, ClaimStatus::Denied);
    assert_eq!(denied.resolution, ClaimResolution::Denied);
    assert!(denied.resolved_at.is_some());
}

// ============================================================================
// Purchase Order Tests
// ============================================================================

#[test]
fn test_purchase_order_workflow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let supplier = commerce.purchase_orders().create_supplier(CreateSupplier {
        name: "Acme Supplies".into(),
        email: Some("orders@acme.test".into()),
        payment_terms: Some(PaymentTerms::Net30),
        ..Default::default()
    }).expect("Failed to create supplier");

    let po = commerce.purchase_orders().create(CreatePurchaseOrder {
        supplier_id: supplier.id,
        items: vec![
            CreatePurchaseOrderItem {
                sku: "PART-001".into(),
                name: "Widget Part".into(),
                quantity: dec!(10),
                unit_cost: dec!(2.50),
                ..Default::default()
            }
        ],
        ..Default::default()
    }).expect("Failed to create purchase order");

    assert_eq!(po.status, PurchaseOrderStatus::Draft);
    assert_eq!(po.items.len(), 1);

    let submitted = commerce.purchase_orders().submit(po.id)
        .expect("Failed to submit PO");
    assert_eq!(submitted.status, PurchaseOrderStatus::PendingApproval);

    let approved = commerce.purchase_orders().approve(po.id, "tester")
        .expect("Failed to approve PO");
    assert_eq!(approved.status, PurchaseOrderStatus::Approved);

    let sent = commerce.purchase_orders().send(po.id)
        .expect("Failed to send PO");
    assert_eq!(sent.status, PurchaseOrderStatus::Sent);

    let fetched = commerce.purchase_orders().get_by_number(&po.po_number)
        .expect("Failed to get PO by number")
        .expect("PO not found");
    assert_eq!(fetched.id, po.id);

    let by_code = commerce.purchase_orders().get_supplier_by_code(&supplier.supplier_code)
        .expect("Failed to get supplier by code")
        .expect("Supplier not found");
    assert_eq!(by_code.id, supplier.id);
}

// ============================================================================
// Invoice Tests
// ============================================================================

#[test]
fn test_invoice_send_and_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let customer = commerce.customers().create(CreateCustomer {
        email: "invoice@example.com".into(),
        first_name: "Invoice".into(),
        last_name: "Test".into(),
        ..Default::default()
    }).expect("Failed to create customer");

    let invoice = commerce.invoices().create(CreateInvoice {
        customer_id: customer.id,
        items: vec![
            CreateInvoiceItem {
                description: "Implementation services".into(),
                quantity: dec!(2),
                unit_price: dec!(100.00),
                ..Default::default()
            }
        ],
        ..Default::default()
    }).expect("Failed to create invoice");

    assert_eq!(invoice.status, InvoiceStatus::Draft);
    assert_eq!(invoice.total, dec!(200.00));

    let sent = commerce.invoices().send(invoice.id)
        .expect("Failed to send invoice");
    assert_eq!(sent.status, InvoiceStatus::Sent);

    let paid = commerce.invoices().record_payment(invoice.id, RecordInvoicePayment {
        amount: dec!(200.00),
        payment_method: Some("card".into()),
        reference: Some("PAY-TEST-001".into()),
        ..Default::default()
    }).expect("Failed to record payment");

    assert_eq!(paid.status, InvoiceStatus::Paid);
    assert_eq!(paid.balance_due, dec!(0));

    let fetched = commerce.invoices().get_by_number(&invoice.invoice_number)
        .expect("Failed to get invoice by number")
        .expect("Invoice not found");
    assert_eq!(fetched.id, invoice.id);
}
