//! Integration tests for manufacturing features (BOM and Work Orders)

use rust_decimal_macros::dec;
use stateset_embedded::{
    Commerce, CreateBom, CreateBomComponent, CreateWorkOrder, CreateWorkOrderTask,
    AddWorkOrderMaterial, BomStatus, WorkOrderStatus, TaskStatus,
};
use uuid::Uuid;

#[test]
fn test_bom_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a product for the BOM
    let product_id = Uuid::new_v4();

    // Create a BOM with components
    let bom = commerce.bom().create(CreateBom {
        product_id,
        name: "Widget Assembly".into(),
        description: Some("Assembly instructions for widget".into()),
        revision: Some("1.0".into()),
        components: Some(vec![
            CreateBomComponent {
                name: "Screw M3x10".into(),
                component_sku: Some("SCREW-M3-10".into()),
                quantity: dec!(4),
                unit_of_measure: Some("pcs".into()),
                ..Default::default()
            },
            CreateBomComponent {
                name: "Plastic Housing".into(),
                component_sku: Some("HOUSING-001".into()),
                quantity: dec!(1),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }).expect("Failed to create BOM");

    assert_eq!(bom.name, "Widget Assembly");
    assert_eq!(bom.status, BomStatus::Draft);
    assert!(bom.bom_number.starts_with("BOM-"));

    // Get BOM by ID
    let retrieved = commerce.bom().get(bom.id).expect("Failed to get BOM");
    assert!(retrieved.is_some());

    // Get components
    let components = commerce.bom().get_components(bom.id).expect("Failed to get components");
    assert_eq!(components.len(), 2);

    // Add another component
    let new_component = commerce.bom().add_component(bom.id, CreateBomComponent {
        name: "LED Light".into(),
        component_sku: Some("LED-001".into()),
        quantity: dec!(2),
        ..Default::default()
    }).expect("Failed to add component");

    assert_eq!(new_component.name, "LED Light");

    // Verify component count increased
    let components = commerce.bom().get_components(bom.id).expect("Failed to get components");
    assert_eq!(components.len(), 3);

    // Activate the BOM
    let activated = commerce.bom().activate(bom.id).expect("Failed to activate BOM");
    assert_eq!(activated.status, BomStatus::Active);

    // List BOMs for product
    let boms = commerce.bom().for_product(product_id).expect("Failed to list BOMs");
    assert_eq!(boms.len(), 1);

    println!("BOM lifecycle test passed!");
}

#[test]
fn test_work_order_lifecycle() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a product
    let product_id = Uuid::new_v4();

    // Create a work order
    let wo = commerce.work_orders().create(CreateWorkOrder {
        product_id,
        bom_id: None,
        quantity_to_build: dec!(100),
        priority: None,
        notes: Some("Rush order for Q4".into()),
        tasks: Some(vec![
            CreateWorkOrderTask {
                task_name: "Cut Materials".into(),
                sequence: Some(1),
                estimated_hours: Some(dec!(2)),
                ..Default::default()
            },
            CreateWorkOrderTask {
                task_name: "Assembly".into(),
                sequence: Some(2),
                estimated_hours: Some(dec!(4)),
                ..Default::default()
            },
            CreateWorkOrderTask {
                task_name: "Quality Check".into(),
                sequence: Some(3),
                estimated_hours: Some(dec!(1)),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }).expect("Failed to create work order");

    assert!(wo.work_order_number.starts_with("WO-"));
    assert_eq!(wo.status, WorkOrderStatus::Planned);
    assert_eq!(wo.quantity_to_build, dec!(100));

    // Get tasks
    let tasks = commerce.work_orders().get_tasks(wo.id).expect("Failed to get tasks");
    assert_eq!(tasks.len(), 3);

    // Add materials
    let material = commerce.work_orders().add_material(wo.id, AddWorkOrderMaterial {
        component_sku: "RAW-STEEL-001".into(),
        component_name: "Raw Steel Plate".into(),
        quantity: dec!(50),
        ..Default::default()
    }).expect("Failed to add material");

    assert_eq!(material.component_name, "Raw Steel Plate");

    // Start the work order
    let wo = commerce.work_orders().start(wo.id).expect("Failed to start work order");
    assert_eq!(wo.status, WorkOrderStatus::InProgress);
    assert!(wo.actual_start.is_some());

    // Start and complete tasks
    let task = &tasks[0];
    let started_task = commerce.work_orders().start_task(task.id).expect("Failed to start task");
    assert_eq!(started_task.status, TaskStatus::InProgress);

    let completed_task = commerce.work_orders().complete_task(task.id, Some(dec!(2.5))).expect("Failed to complete task");
    assert_eq!(completed_task.status, TaskStatus::Completed);
    assert_eq!(completed_task.actual_hours, Some(dec!(2.5)));

    // Consume material
    let consumed = commerce.work_orders().consume_material(material.id, dec!(25)).expect("Failed to consume material");
    assert_eq!(consumed.consumed_quantity, dec!(25));

    // Complete the work order
    let wo = commerce.work_orders().complete(wo.id, dec!(98)).expect("Failed to complete work order");
    assert_eq!(wo.status, WorkOrderStatus::PartiallyCompleted); // 98 < 100
    assert_eq!(wo.quantity_completed, dec!(98));

    // Count work orders
    let count = commerce.work_orders().count(Default::default()).expect("Failed to count");
    assert_eq!(count, 1);

    println!("Work order lifecycle test passed!");
}

#[test]
fn test_work_order_hold_and_cancel() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let wo = commerce.work_orders().create(CreateWorkOrder {
        product_id: Uuid::new_v4(),
        quantity_to_build: dec!(50),
        ..Default::default()
    }).expect("Failed to create work order");

    // Start it
    let wo = commerce.work_orders().start(wo.id).expect("Failed to start");
    assert_eq!(wo.status, WorkOrderStatus::InProgress);

    // Put on hold
    let wo = commerce.work_orders().hold(wo.id).expect("Failed to hold");
    assert_eq!(wo.status, WorkOrderStatus::OnHold);

    // Resume
    let wo = commerce.work_orders().resume(wo.id).expect("Failed to resume");
    assert_eq!(wo.status, WorkOrderStatus::InProgress);

    // Cancel
    let wo = commerce.work_orders().cancel(wo.id).expect("Failed to cancel");
    assert_eq!(wo.status, WorkOrderStatus::Cancelled);

    println!("Work order hold/cancel test passed!");
}

#[test]
fn test_bom_and_work_order_integration() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let product_id = Uuid::new_v4();

    // Create a BOM
    let bom = commerce.bom().create(CreateBom {
        product_id,
        name: "Gadget Assembly".into(),
        components: Some(vec![
            CreateBomComponent {
                name: "Circuit Board".into(),
                component_sku: Some("PCB-001".into()),
                quantity: dec!(1),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }).expect("Failed to create BOM");

    // Activate BOM
    commerce.bom().activate(bom.id).expect("Failed to activate BOM");

    // Create work order referencing the BOM
    let wo = commerce.work_orders().create(CreateWorkOrder {
        product_id,
        bom_id: Some(bom.id),
        quantity_to_build: dec!(25),
        ..Default::default()
    }).expect("Failed to create work order");

    assert_eq!(wo.bom_id, Some(bom.id));

    // Find work orders for this BOM
    let work_orders = commerce.work_orders().for_bom(bom.id).expect("Failed to find work orders");
    assert_eq!(work_orders.len(), 1);
    assert_eq!(work_orders[0].id, wo.id);

    println!("BOM and work order integration test passed!");
}
