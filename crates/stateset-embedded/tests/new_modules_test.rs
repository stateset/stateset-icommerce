//! Tests for newly added commerce modules

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateCustomer,
    // Warehouse types
    CreateWarehouse, CreateLocation, CreateZone, WarehouseType, LocationType,
    // Lot types
    CreateLot,
    // Serial types
    CreateSerialNumber, SerialStatus,
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
