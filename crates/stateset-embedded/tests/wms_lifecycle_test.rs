#![cfg(feature = "sqlite")]

//! Lifecycle guards and atomicity for the WMS modules — fulfillment
//! (wave/pick/pack/ship) and receiving (receipt/put-away) — on the SQLite
//! backend through the sync `Commerce` engine.
//!
//! Covers:
//! - wave transitions: `release`/`complete`/`cancel` are refused from statuses
//!   that are not on their allowlist, and a cancelled wave can no longer be
//!   "completed";
//! - pick transitions: cancelling or shortening a *finished* pick is refused, so
//!   `waves.completed_pick_count` keeps describing reality; `report_short`
//!   finalizes a pick and folds it into the wave counter exactly once, like
//!   `complete_pick`;
//! - pack/ship/carton guards: no writes to a completed or cancelled task;
//! - the happy path still runs end to end
//!   (wave -> release -> assign -> start -> complete -> wave complete);
//! - receiving: over-receipt is capped against the line's expected quantity,
//!   receipts in a terminal status refuse items, and a multi-line receive where
//!   one line is invalid applies NOTHING (atomicity);
//! - put-away transitions are guarded, and completing one that was cancelled no
//!   longer inflates `receipts.put_away_quantity`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::CommerceError;
use stateset_embedded::{
    AddCarton, AddCartonItem, Commerce, CompletePick, CompletePutAway, CompleteShip,
    CreateLocation, CreatePackTask, CreatePickTask, CreatePutAway, CreateReceipt,
    CreateReceiptItem, CreateShipTask, CreateWarehouse, CreateWave, LocationType, OrderId,
    OrderItemId, PackStatus, PackageType, PickStatus, PickTask, PutAway, PutAwayStatus, Receipt,
    ReceiptItem, ReceiptStatus, ReceiveItemLine, ReceiveItems, ShipStatus, ShipTask, ShipmentId,
    WarehouseType, Wave, WaveStatus,
};
use uuid::Uuid;

// ============================================================================
// Fixtures
// ============================================================================

/// The SKU every pick task in this file draws on.
const WMS_SKU: &str = "WMS-SKU-1";

/// An in-memory engine plus the warehouse and pickable location that
/// fulfillment/receiving rows carry foreign keys onto.
struct Wms {
    commerce: Commerce,
    warehouse_id: i32,
    location_id: i32,
}

fn wms() -> Wms {
    let commerce = Commerce::new(":memory:").expect("in-memory Commerce");
    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &Uuid::new_v4().to_string()[..8]),
            name: "WMS Lifecycle".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .expect("create warehouse");
    let location = commerce
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some("PICK-1".into()),
            location_type: LocationType::Pick,
            is_pickable: Some(true),
            is_receivable: Some(true),
            ..Default::default()
        })
        .expect("create location");

    // Completing a pick now takes the units out of the source bin and
    // allocates them at warehouse level, so the shelf has to hold them first.
    // Every pick in this file draws on `WMS_SKU`; stock it generously so the
    // lifecycle tests exercise the state machine, not the stock guard.
    commerce
        .inventory()
        .create_item(stateset_core::CreateInventoryItem {
            sku: WMS_SKU.into(),
            name: WMS_SKU.into(),
            ..Default::default()
        })
        .expect("create inventory item");
    commerce
        .inventory()
        .adjust_at_location(WMS_SKU, warehouse.id, Decimal::from(10_000), "seed shelf")
        .expect("seed warehouse balance");
    commerce
        .warehouse()
        .adjust_inventory(stateset_core::AdjustLocationInventory {
            location_id: location.id,
            sku: WMS_SKU.into(),
            lot_id: None,
            quantity: Decimal::from(10_000),
            reason: "seed shelf".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .expect("seed bin");

    Wms { commerce, warehouse_id: warehouse.id, location_id: location.id }
}

impl Wms {
    fn wave(&self) -> Wave {
        self.commerce
            .fulfillment()
            .create_wave(CreateWave {
                warehouse_id: self.warehouse_id,
                order_ids: vec![OrderId::new()],
                priority: Some(1),
                notes: None,
                created_by: None,
            })
            .expect("create wave")
    }

    /// A wave that has been released to the floor.
    fn released_wave(&self) -> Wave {
        let wave = self.wave();
        self.commerce.fulfillment().release_wave(wave.id).expect("release wave")
    }

    fn pick(&self, wave: Option<&Wave>, qty: Decimal) -> PickTask {
        self.commerce
            .fulfillment()
            .create_pick(CreatePickTask {
                wave_id: wave.map(|w| w.id),
                order_id: OrderId::new(),
                order_item_id: OrderItemId::new(),
                warehouse_id: self.warehouse_id,
                sku: WMS_SKU.into(),
                product_name: Some("Widget".into()),
                source_location_id: self.location_id,
                quantity_requested: qty,
                lot_id: None,
                serial_number: None,
                priority: Some(1),
                notes: None,
            })
            .expect("create pick")
    }

    fn complete_pick(&self, pick: &PickTask, qty: Decimal) -> PickTask {
        self.commerce
            .fulfillment()
            .complete_pick(CompletePick {
                pick_id: pick.id,
                quantity_picked: qty,
                quantity_short: None,
                short_reason: None,
                lot_id: None,
                serial_number: None,
                completed_by: Some("alice".into()),
            })
            .expect("complete pick")
    }

    fn completed_pick_count(&self, wave: &Wave) -> i32 {
        self.commerce
            .fulfillment()
            .get_wave(wave.id)
            .expect("get wave")
            .expect("wave exists")
            .completed_pick_count
    }

    fn pick_status(&self, id: Uuid) -> PickStatus {
        self.commerce.fulfillment().get_pick(id).expect("get pick").expect("pick exists").status
    }

    fn ship_task(&self) -> ShipTask {
        let order_id = OrderId::new();
        let pack = self
            .commerce
            .fulfillment()
            .create_pack(CreatePackTask { order_id, notes: None })
            .expect("create pack");
        self.commerce
            .fulfillment()
            .create_ship(CreateShipTask {
                order_id,
                shipment_id: ShipmentId::new(),
                pack_task_id: pack.id,
                carrier: Some("UPS".into()),
                service_level: None,
                notes: None,
            })
            .expect("create ship")
    }

    /// A receipt with the given per-line expected quantities, plus its lines.
    fn receipt(&self, expected: &[Decimal]) -> (Receipt, Vec<ReceiptItem>) {
        let receipt = self
            .commerce
            .receiving()
            .create_receipt(CreateReceipt {
                warehouse_id: self.warehouse_id,
                items: expected
                    .iter()
                    .enumerate()
                    .map(|(idx, qty)| CreateReceiptItem {
                        sku: format!("RCV-SKU-{idx}"),
                        expected_quantity: *qty,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })
            .expect("create receipt");
        let items = self.commerce.receiving().get_receipt_items(receipt.id).expect("items");
        (receipt, items)
    }

    fn receive(
        &self,
        receipt: &Receipt,
        lines: &[(Uuid, Decimal)],
    ) -> stateset_core::Result<Receipt> {
        self.commerce.receiving().receive_items(ReceiveItems {
            receipt_id: receipt.id,
            items: lines
                .iter()
                .map(|(item_id, qty)| ReceiveItemLine {
                    receipt_item_id: *item_id,
                    quantity_received: *qty,
                    quantity_rejected: None,
                    rejection_reason: None,
                    lot_number: None,
                    serial_numbers: None,
                    expiration_date: None,
                    notes: None,
                })
                .collect(),
            receiving_location_id: None,
            received_by: None,
        })
    }

    fn line_received(&self, receipt: &Receipt, item_id: Uuid) -> Decimal {
        self.commerce
            .receiving()
            .get_receipt_items(receipt.id)
            .expect("items")
            .into_iter()
            .find(|i| i.id == item_id)
            .expect("line exists")
            .received_quantity
    }

    fn receipt_now(&self, receipt: &Receipt) -> Receipt {
        self.commerce
            .receiving()
            .get_receipt(receipt.id)
            .expect("get receipt")
            .expect("receipt exists")
    }

    fn put_away(&self, receipt: &Receipt, item: &ReceiptItem, qty: Decimal) -> PutAway {
        self.commerce
            .receiving()
            .create_put_away(CreatePutAway {
                receipt_id: receipt.id,
                receipt_item_id: item.id,
                sku: item.sku.clone(),
                from_location_id: None,
                to_location_id: self.location_id,
                quantity: qty,
                lot_id: None,
                assigned_to: None,
                notes: None,
            })
            .expect("create put-away")
    }
}

fn assert_conflict(err: &CommerceError) {
    assert!(matches!(err, CommerceError::Conflict(_)), "expected Conflict, got {err:?}");
}

// ============================================================================
// Wave lifecycle
// ============================================================================

/// Allowlist: Released|InProgress -> Completed. A cancelled wave is terminal.
#[test]
fn complete_wave_rejects_cancelled_wave() {
    let wms = wms();
    let wave = wms.wave();
    wms.commerce.fulfillment().cancel_wave(wave.id).expect("cancel wave");

    let err = wms
        .commerce
        .fulfillment()
        .complete_wave(wave.id)
        .expect_err("a cancelled wave must not be completable");
    assert_conflict(&err);

    let after = wms.commerce.fulfillment().get_wave(wave.id).expect("get").expect("exists");
    assert_eq!(after.status, WaveStatus::Cancelled, "status must remain cancelled");
}

#[test]
fn complete_wave_rejects_draft_wave_and_is_not_repeatable() {
    let wms = wms();
    let wave = wms.wave();

    // Never released -> nothing was ever on the floor to complete.
    let err = wms.commerce.fulfillment().complete_wave(wave.id).expect_err("draft is not legal");
    assert_conflict(&err);

    let released = wms.commerce.fulfillment().release_wave(wave.id).expect("release");
    assert_eq!(released.status, WaveStatus::Released);
    let done = wms.commerce.fulfillment().complete_wave(wave.id).expect("complete");
    assert_eq!(done.status, WaveStatus::Completed);

    // Completed is terminal: a second completion is refused, not silently
    // rewriting `completed_at`.
    let err = wms.commerce.fulfillment().complete_wave(wave.id).expect_err("already completed");
    assert_conflict(&err);
}

/// Allowlist: Draft only -> Released.
#[test]
fn release_wave_rejects_already_released_and_cancelled_waves() {
    let wms = wms();
    let wave = wms.released_wave();
    let err = wms.commerce.fulfillment().release_wave(wave.id).expect_err("already released");
    assert_conflict(&err);

    let other = wms.wave();
    wms.commerce.fulfillment().cancel_wave(other.id).expect("cancel");
    let err = wms.commerce.fulfillment().release_wave(other.id).expect_err("cancelled wave");
    assert_conflict(&err);
}

/// Allowlist: Draft|Released|InProgress -> Cancelled.
#[test]
fn cancel_wave_rejects_completed_wave() {
    let wms = wms();
    let wave = wms.released_wave();
    wms.commerce.fulfillment().complete_wave(wave.id).expect("complete");

    let err = wms.commerce.fulfillment().cancel_wave(wave.id).expect_err("completed is terminal");
    assert_conflict(&err);

    let after = wms.commerce.fulfillment().get_wave(wave.id).expect("get").expect("exists");
    assert_eq!(after.status, WaveStatus::Completed);
}

#[test]
fn wave_transitions_on_unknown_wave_are_not_found() {
    let wms = wms();
    let ghost = stateset_embedded::FulfillmentId::new();
    for err in [
        wms.commerce.fulfillment().release_wave(ghost).expect_err("release"),
        wms.commerce.fulfillment().complete_wave(ghost).expect_err("complete"),
        wms.commerce.fulfillment().cancel_wave(ghost).expect_err("cancel"),
    ] {
        assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    }
}

// ============================================================================
// Pick lifecycle — and the wave counter it feeds
// ============================================================================

/// The headline defect: cancelling an already-completed pick used to succeed
/// while `waves.completed_pick_count` kept counting it.
#[test]
fn cancel_pick_rejects_completed_pick_and_wave_counter_stays_correct() {
    let wms = wms();
    let wave = wms.released_wave();
    let pick = wms.pick(Some(&wave), dec!(5));

    wms.complete_pick(&pick, dec!(5));
    assert_eq!(wms.completed_pick_count(&wave), 1);

    let err = wms
        .commerce
        .fulfillment()
        .cancel_pick(pick.id)
        .expect_err("a completed pick must not be cancellable");
    assert_conflict(&err);

    assert_eq!(wms.pick_status(pick.id), PickStatus::Completed, "status must remain completed");
    assert_eq!(
        wms.completed_pick_count(&wave),
        1,
        "the wave counter must still describe the completed pick"
    );
}

#[test]
fn cancel_pick_rejects_short_and_cancelled_picks() {
    let wms = wms();
    let short = wms.pick(None, dec!(5));
    wms.commerce.fulfillment().report_short(short.id, dec!(5), "empty bin").expect("short");
    let err = wms.commerce.fulfillment().cancel_pick(short.id).expect_err("short is terminal");
    assert_conflict(&err);

    let cancelled = wms.pick(None, dec!(5));
    wms.commerce.fulfillment().cancel_pick(cancelled.id).expect("cancel");
    let err = wms.commerce.fulfillment().cancel_pick(cancelled.id).expect_err("already cancelled");
    assert_conflict(&err);
}

/// Allowlist: Pending|Assigned|InProgress -> Short.
#[test]
fn report_short_rejects_terminal_picks() {
    let wms = wms();
    let wave = wms.released_wave();

    let completed = wms.pick(Some(&wave), dec!(5));
    wms.complete_pick(&completed, dec!(5));
    let err = wms
        .commerce
        .fulfillment()
        .report_short(completed.id, dec!(2), "miscount")
        .expect_err("a completed pick must not be re-reported short");
    assert_conflict(&err);
    assert_eq!(wms.pick_status(completed.id), PickStatus::Completed);
    assert_eq!(wms.completed_pick_count(&wave), 1, "counter unchanged by the refused call");

    let cancelled = wms.pick(Some(&wave), dec!(5));
    wms.commerce.fulfillment().cancel_pick(cancelled.id).expect("cancel");
    let err = wms
        .commerce
        .fulfillment()
        .report_short(cancelled.id, dec!(2), "miscount")
        .expect_err("a cancelled pick must not be reported short");
    assert_conflict(&err);
    assert_eq!(wms.completed_pick_count(&wave), 1);
}

/// A shortage finalizes the pick exactly like a completion, so it folds into the
/// wave counter exactly once — and cannot be reported twice.
#[test]
fn report_short_finalizes_pick_and_counts_once() {
    let wms = wms();
    let wave = wms.released_wave();
    let pick = wms.pick(Some(&wave), dec!(5));

    let short =
        wms.commerce.fulfillment().report_short(pick.id, dec!(5), "empty bin").expect("short");
    assert_eq!(short.status, PickStatus::Short);
    assert_eq!(short.quantity_short, dec!(5));
    assert_eq!(wms.completed_pick_count(&wave), 1);

    let err = wms
        .commerce
        .fulfillment()
        .report_short(pick.id, dec!(5), "empty bin again")
        .expect_err("short is terminal");
    assert_conflict(&err);
    assert_eq!(wms.completed_pick_count(&wave), 1, "no double count");
}

/// Allowlist: Pending|Assigned -> Assigned / `InProgress`.
#[test]
fn assign_and_start_pick_reject_finished_picks() {
    let wms = wms();
    let pick = wms.pick(None, dec!(5));
    wms.complete_pick(&pick, dec!(5));

    let err = wms.commerce.fulfillment().assign_pick(pick.id, "bob").expect_err("assign");
    assert_conflict(&err);
    let err = wms.commerce.fulfillment().start_pick(pick.id).expect_err("start");
    assert_conflict(&err);
    assert_eq!(wms.pick_status(pick.id), PickStatus::Completed);
}

#[test]
fn start_pick_rejects_restart_of_in_progress_pick() {
    let wms = wms();
    let pick = wms.pick(None, dec!(5));
    let started = wms.commerce.fulfillment().start_pick(pick.id).expect("start");
    assert_eq!(started.status, PickStatus::InProgress);
    let first_started_at = started.started_at.expect("started_at set");

    let err = wms.commerce.fulfillment().start_pick(pick.id).expect_err("already started");
    assert_conflict(&err);

    let after = wms.commerce.fulfillment().get_pick(pick.id).expect("get").expect("exists");
    assert_eq!(after.started_at, Some(first_started_at), "started_at must not be reset");
}

/// The full outbound happy path still works end to end.
#[test]
fn happy_path_wave_release_assign_start_complete() {
    let wms = wms();
    let f = wms.commerce.fulfillment();

    let wave = wms.wave();
    assert_eq!(wave.status, WaveStatus::Draft);

    let released = f.release_wave(wave.id).expect("release");
    assert_eq!(released.status, WaveStatus::Released);

    let pick_a = wms.pick(Some(&wave), dec!(5));
    let pick_b = wms.pick(Some(&wave), dec!(3));

    let assigned = f.assign_pick(pick_a.id, "alice").expect("assign");
    assert_eq!(assigned.status, PickStatus::Assigned);
    assert_eq!(assigned.assigned_to.as_deref(), Some("alice"));

    let started = f.start_pick(pick_a.id).expect("start");
    assert_eq!(started.status, PickStatus::InProgress);

    let done = wms.complete_pick(&pick_a, dec!(5));
    assert_eq!(done.status, PickStatus::Completed);
    assert_eq!(done.quantity_picked, dec!(5));

    let short = f.report_short(pick_b.id, dec!(3), "empty bin").expect("short");
    assert_eq!(short.status, PickStatus::Short);

    // Both picks are finished, so both are folded into the wave counter.
    assert_eq!(wms.completed_pick_count(&wave), 2);

    let completed = f.complete_wave(wave.id).expect("complete wave");
    assert_eq!(completed.status, WaveStatus::Completed);
}

// ============================================================================
// Pack / carton / ship lifecycle
// ============================================================================

#[test]
fn pack_transitions_reject_completed_and_cancelled_tasks() {
    let wms = wms();
    let f = wms.commerce.fulfillment();

    let pack =
        f.create_pack(CreatePackTask { order_id: OrderId::new(), notes: None }).expect("pack");
    let completed = f.complete_pack(pack.id).expect("complete");
    assert_eq!(completed.status, PackStatus::Completed);

    for err in [
        f.complete_pack(pack.id).expect_err("re-complete"),
        f.cancel_pack(pack.id).expect_err("cancel completed"),
        f.start_pack(pack.id).expect_err("start completed"),
        f.assign_pack(pack.id, "bob").expect_err("assign completed"),
    ] {
        assert_conflict(&err);
    }

    let other =
        f.create_pack(CreatePackTask { order_id: OrderId::new(), notes: None }).expect("pack");
    f.cancel_pack(other.id).expect("cancel");
    let err = f.complete_pack(other.id).expect_err("a cancelled pack must not complete");
    assert_conflict(&err);
}

/// SQLite used to leave the status alone on assign while Postgres set
/// `assigned` — both now agree.
#[test]
fn assign_pack_sets_assigned_status() {
    let wms = wms();
    let f = wms.commerce.fulfillment();
    let pack =
        f.create_pack(CreatePackTask { order_id: OrderId::new(), notes: None }).expect("pack");

    let assigned = f.assign_pack(pack.id, "bob").expect("assign");
    assert_eq!(assigned.status, PackStatus::Assigned);
    assert_eq!(assigned.assigned_to.as_deref(), Some("bob"));
}

#[test]
fn cartons_cannot_be_added_to_a_finished_pack_task() {
    let wms = wms();
    let f = wms.commerce.fulfillment();
    let pack =
        f.create_pack(CreatePackTask { order_id: OrderId::new(), notes: None }).expect("pack");

    let carton = f
        .add_carton(AddCarton {
            pack_task_id: pack.id,
            package_type: PackageType::Box,
            weight_kg: Some(dec!(1.5)),
            length_cm: None,
            width_cm: None,
            height_cm: None,
        })
        .expect("add carton");
    let counted = f.get_pack(pack.id).expect("get").expect("exists");
    assert_eq!(counted.carton_count, 1, "carton and counter move together");

    f.complete_pack(pack.id).expect("complete");

    let err = f
        .add_carton(AddCarton {
            pack_task_id: pack.id,
            package_type: PackageType::Box,
            weight_kg: None,
            length_cm: None,
            width_cm: None,
            height_cm: None,
        })
        .expect_err("a sealed pack task must not grow another carton");
    assert_conflict(&err);

    let err = f
        .add_carton_item(AddCartonItem {
            carton_id: carton.id,
            sku: WMS_SKU.into(),
            quantity: dec!(1),
            lot_id: None,
            serial_number: None,
        })
        .expect_err("a sealed carton must not gain contents");
    assert_conflict(&err);

    let after = f.get_pack(pack.id).expect("get").expect("exists");
    assert_eq!(after.carton_count, 1, "the refused carton must not bump the counter");
    assert_eq!(f.get_cartons(pack.id).expect("cartons").len(), 1);
}

#[test]
fn ship_transitions_reject_shipped_and_cancelled_tasks() {
    let wms = wms();
    let f = wms.commerce.fulfillment();

    let ship = wms.ship_task();
    let printed = f.print_label(ship.id, "https://labels.example/1").expect("print");
    assert_eq!(printed.status, ShipStatus::LabelPrinted);

    let shipped = f
        .complete_ship(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-AAA".into(),
            shipping_cost: Some(dec!(12.50)),
            shipped_by: None,
        })
        .expect("ship");
    assert_eq!(shipped.status, ShipStatus::Shipped);

    // Re-shipping used to overwrite the tracking number and cost.
    let err = f
        .complete_ship(CompleteShip {
            ship_task_id: ship.id,
            tracking_number: "1Z-BBB".into(),
            shipping_cost: Some(dec!(99)),
            shipped_by: None,
        })
        .expect_err("already shipped");
    assert_conflict(&err);

    for err in [
        f.cancel_ship(ship.id).expect_err("cancel shipped"),
        f.print_label(ship.id, "https://labels.example/2").expect_err("relabel shipped"),
        f.assign_ship(ship.id, "carl").expect_err("assign shipped"),
    ] {
        assert_conflict(&err);
    }

    let after = f.get_ship(ship.id).expect("get").expect("exists");
    assert_eq!(after.tracking_number.as_deref(), Some("1Z-AAA"), "tracking must be untouched");
    assert_eq!(after.shipping_cost, Some(dec!(12.50)));

    let other = wms.ship_task();
    f.cancel_ship(other.id).expect("cancel");
    let err = f
        .complete_ship(CompleteShip {
            ship_task_id: other.id,
            tracking_number: "1Z-CCC".into(),
            shipping_cost: None,
            shipped_by: None,
        })
        .expect_err("a cancelled ship task must not ship");
    assert_conflict(&err);
}

// ============================================================================
// Receiving — over-receipt cap, status guards, atomicity
// ============================================================================

/// The headline receiving defect: nothing bounded the received quantity, so 100
/// units could be booked against a 10-unit line.
#[test]
fn receive_items_rejects_over_receipt() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    let line = items[0].id;

    let err = wms.receive(&receipt, &[(line, dec!(100))]).expect_err("over-receipt");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.line_received(&receipt, line), Decimal::ZERO, "nothing may be applied");

    // Partial receipts accumulate up to — but never past — the expected quantity.
    wms.receive(&receipt, &[(line, dec!(4))]).expect("partial receive");
    assert_eq!(wms.line_received(&receipt, line), dec!(4));

    let err = wms.receive(&receipt, &[(line, dec!(7))]).expect_err("4 + 7 > 10");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.line_received(&receipt, line), dec!(4), "the refused line is unchanged");

    let full = wms.receive(&receipt, &[(line, dec!(6))]).expect("exact remainder");
    assert_eq!(wms.line_received(&receipt, line), dec!(10));
    assert_eq!(full.received_quantity, dec!(10), "header total tracks the lines");
}

/// Two lines in one call may not jointly exceed a shared line's expectation:
/// the cap is re-read under the write lock for every line.
#[test]
fn receive_items_caps_repeated_lines_within_one_call() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    let line = items[0].id;

    let err = wms
        .receive(&receipt, &[(line, dec!(6)), (line, dec!(6))])
        .expect_err("6 + 6 > 10 within one call");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.line_received(&receipt, line), Decimal::ZERO, "nothing applied");
}

/// Atomicity: a multi-line receive where the LAST line is invalid must leave
/// every line and the header exactly as they were.
#[test]
fn receive_items_is_all_or_nothing_across_lines() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10), dec!(10), dec!(10)]);
    let (a, b, c) = (items[0].id, items[1].id, items[2].id);

    assert_eq!(wms.receipt_now(&receipt).status, ReceiptStatus::Expected);

    let err = wms
        .receive(&receipt, &[(a, dec!(5)), (b, dec!(5)), (c, dec!(50))])
        .expect_err("the third line exceeds its expectation");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Every line untouched...
    assert_eq!(wms.line_received(&receipt, a), Decimal::ZERO, "line A must not be applied");
    assert_eq!(wms.line_received(&receipt, b), Decimal::ZERO, "line B must not be applied");
    assert_eq!(wms.line_received(&receipt, c), Decimal::ZERO, "line C must not be applied");

    // ...and so is the header, including the `Expected -> InProgress` flip that
    // the same transaction would have made.
    let header = wms.receipt_now(&receipt);
    assert_eq!(header.received_quantity, Decimal::ZERO, "header total must not move");
    assert_eq!(header.status, ReceiptStatus::Expected, "the status flip must roll back too");
    assert!(header.received_date.is_none(), "received_date must not be stamped");

    // The same call with a legal third line applies all three together.
    let ok = wms
        .receive(&receipt, &[(a, dec!(5)), (b, dec!(5)), (c, dec!(5))])
        .expect("all lines within their expectations");
    assert_eq!(ok.received_quantity, dec!(15));
    assert_eq!(ok.status, ReceiptStatus::InProgress);
}

#[test]
fn receive_items_rejects_completed_and_cancelled_receipts() {
    let wms = wms();

    let (completed, items) = wms.receipt(&[dec!(10)]);
    wms.commerce.receiving().start_receiving(completed.id).expect("start");
    wms.receive(&completed, &[(items[0].id, dec!(10))]).expect("receive");
    wms.commerce.receiving().complete_receiving(completed.id).expect("complete");

    let err = wms
        .receive(&completed, &[(items[0].id, dec!(1))])
        .expect_err("a completed receipt must not take more goods");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.line_received(&completed, items[0].id), dec!(10), "line unchanged");

    let (cancelled, cancelled_items) = wms.receipt(&[dec!(10)]);
    wms.commerce.receiving().cancel_receipt(cancelled.id).expect("cancel");
    let err = wms
        .receive(&cancelled, &[(cancelled_items[0].id, dec!(1))])
        .expect_err("a cancelled receipt must not take goods");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.line_received(&cancelled, cancelled_items[0].id), Decimal::ZERO);
}

#[test]
fn receive_items_rejects_non_positive_quantities_and_foreign_lines() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    let (other, other_items) = wms.receipt(&[dec!(10)]);

    let err = wms.receive(&receipt, &[(items[0].id, Decimal::ZERO)]).expect_err("zero");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let err = wms.receive(&receipt, &[(items[0].id, dec!(-1))]).expect_err("negative");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // A line belonging to another receipt is not receivable through this one.
    let err =
        wms.receive(&receipt, &[(other_items[0].id, dec!(1))]).expect_err("cross-receipt line");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    assert_eq!(wms.line_received(&other, other_items[0].id), Decimal::ZERO);
}

#[test]
fn complete_receiving_rejects_receipts_that_never_started() {
    let wms = wms();
    let (receipt, _) = wms.receipt(&[dec!(10)]);

    let err = wms
        .commerce
        .receiving()
        .complete_receiving(receipt.id)
        .expect_err("only in_progress receipts complete");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(wms.receipt_now(&receipt).status, ReceiptStatus::Expected);

    wms.commerce.receiving().start_receiving(receipt.id).expect("start");
    wms.commerce.receiving().complete_receiving(receipt.id).expect("complete");

    // Terminal: a second completion is refused rather than restamping the date.
    let err =
        wms.commerce.receiving().complete_receiving(receipt.id).expect_err("already completed");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

#[test]
fn start_receiving_and_delete_receipt_are_guarded() {
    let wms = wms();
    let (receipt, _) = wms.receipt(&[dec!(10)]);
    wms.commerce.receiving().start_receiving(receipt.id).expect("start");

    let err =
        wms.commerce.receiving().start_receiving(receipt.id).expect_err("already in progress");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let err = wms
        .commerce
        .receiving()
        .delete_receipt(receipt.id)
        .expect_err("only untouched receipts may be deleted");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert!(wms.commerce.receiving().get_receipt(receipt.id).expect("get").is_some());
}

// ============================================================================
// Put-away lifecycle
// ============================================================================

#[test]
fn complete_put_away_rejects_cancelled_task_and_leaves_receipt_total_alone() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    // Put-aways are capped at the received quantity, so receive the line first.
    wms.receive(&receipt, &[(items[0].id, dec!(10))]).expect("receive");
    let task = wms.put_away(&receipt, &items[0], dec!(10));

    wms.commerce.receiving().cancel_put_away(task.id).expect("cancel");

    let err = wms
        .commerce
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: task.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect_err("a cancelled put-away must not complete");
    assert_conflict(&err);

    assert_eq!(
        wms.receipt_now(&receipt).put_away_quantity,
        Decimal::ZERO,
        "stock that was never put away must not be counted"
    );
}

#[test]
fn complete_put_away_folds_quantity_into_receipt_exactly_once() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    // Put-aways are capped at the received quantity, so receive the line first.
    wms.receive(&receipt, &[(items[0].id, dec!(10))]).expect("receive");
    let task = wms.put_away(&receipt, &items[0], dec!(10));

    let done = wms
        .commerce
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: task.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect("complete put-away");
    assert_eq!(done.status, PutAwayStatus::Completed);
    assert_eq!(wms.receipt_now(&receipt).put_away_quantity, dec!(10));

    let err = wms
        .commerce
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: task.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect_err("completed is terminal");
    assert_conflict(&err);
    assert_eq!(wms.receipt_now(&receipt).put_away_quantity, dec!(10));
}

#[test]
fn put_away_assign_start_cancel_are_guarded() {
    let wms = wms();
    let (receipt, items) = wms.receipt(&[dec!(10)]);
    wms.receive(&receipt, &[(items[0].id, dec!(10))]).expect("receive");

    let completed = wms.put_away(&receipt, &items[0], dec!(4));
    wms.commerce
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: completed.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect("complete");
    for err in [
        wms.commerce.receiving().assign_put_away(completed.id, "dana").expect_err("assign"),
        wms.commerce.receiving().start_put_away(completed.id).expect_err("start"),
        wms.commerce.receiving().cancel_put_away(completed.id).expect_err("cancel"),
    ] {
        assert_conflict(&err);
    }

    // Happy path: pending -> assigned -> in progress -> completed.
    let task = wms.put_away(&receipt, &items[0], dec!(6));
    let assigned = wms.commerce.receiving().assign_put_away(task.id, "dana").expect("assign");
    assert_eq!(assigned.status, PutAwayStatus::Assigned);
    let started = wms.commerce.receiving().start_put_away(task.id).expect("start");
    assert_eq!(started.status, PutAwayStatus::InProgress);
    // Re-assigning a started task would rewind it to `assigned`.
    let err = wms
        .commerce
        .receiving()
        .assign_put_away(task.id, "erin")
        .expect_err("a started put-away must not be rewound");
    assert_conflict(&err);
    let done = wms
        .commerce
        .receiving()
        .complete_put_away(CompletePutAway {
            put_away_id: task.id,
            actual_location_id: None,
            completed_by: None,
            notes: None,
        })
        .expect("complete");
    assert_eq!(done.status, PutAwayStatus::Completed);
    assert_eq!(wms.receipt_now(&receipt).put_away_quantity, dec!(10));
}
