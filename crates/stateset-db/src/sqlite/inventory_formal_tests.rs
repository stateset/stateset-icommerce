#![cfg(test)]
//! Bounded conformance to Lean's executable, safety-proved transitions.
//! Each fixture runs through real SQL at three exact decimal scales.
use super::*;
use crate::SqliteDatabase;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    status: String,
    quantity: i64,
}

#[derive(Debug, Deserialize)]
struct State {
    on_hand: i64,
    allocated: i64,
    shipped: i64,
    rows: Vec<Row>,
}

#[derive(Debug, Deserialize)]
struct Case {
    operation: String,
    amount: i64,
    time: String,
    #[serde(default)]
    decision: String,
    before: State,
    after: State,
}

fn scaled(value: i64, scale: u32) -> Decimal {
    Decimal::new(value, scale)
}

fn run_cases(operation: &str) {
    let db = SqliteDatabase::in_memory().expect("database");
    let repo = db.inventory();
    let item = repo
        .create_item(CreateInventoryItem {
            sku: "LEAN-CONFORMANCE".into(),
            name: "Lean conformance".into(),
            initial_quantity: Some(Decimal::from(10)),
            ..Default::default()
        })
        .expect("item");
    let now: DateTime<Utc> = "2030-01-01T00:00:00Z".parse().expect("fixed time");
    let golden = include_str!("../../../../formal/lean/inventory_transitions.golden.jsonl");
    let mut conn = repo.pool.get().expect("connection");
    let mut checked = 0;
    let mut failures = Vec::new();
    for line in golden.lines() {
        let case: Case = serde_json::from_str(line).expect("Lean-generated case");
        if case.operation != operation {
            continue;
        }
        for scale in [0, 2, 6] {
            checked += 1;
            let tx = conn.transaction().expect("transaction");
            let target = Uuid::new_v4();
            let other = Uuid::new_v4();
            let expires = match case.time.as_str() {
                "past" => Some(now - chrono::Duration::seconds(1)),
                "now" => Some(now),
                "future" => Some(now + chrono::Duration::seconds(1)),
                "none" => None,
                unexpected => panic!("unknown time: {unexpected}"),
            };
            for (index, row) in case.before.rows.iter().enumerate() {
                tx.execute(
                    "INSERT INTO inventory_reservations
                     (id, item_id, location_id, quantity, status, reference_type, reference_id,
                      expires_at, created_at)
                     VALUES (?, ?, 1, ?, ?, 'formal', 'lean', ?, ?)",
                    rusqlite::params![
                        if index == 0 { target } else { other }.to_string(),
                        item.id,
                        scaled(row.quantity, scale).to_string(),
                        row.status,
                        if index == 0 { expires.map(|t| t.to_rfc3339()) } else { None },
                        now.to_rfc3339(),
                    ],
                )
                .expect("seed row");
            }
            tx.execute(
                "UPDATE inventory_balances SET quantity_on_hand = ?, quantity_allocated = ?,
                 quantity_available = ?, version = 1 WHERE item_id = ? AND location_id = 1",
                rusqlite::params![
                    scaled(case.before.on_hand, scale).to_string(),
                    scaled(case.before.allocated, scale).to_string(),
                    scaled(case.before.on_hand - case.before.allocated, scale).to_string(),
                    item.id,
                ],
            )
            .expect("seed balance");

            let status: ReservationStatus = case.before.rows[0].status.parse().expect("status");
            let due = case.time == "past";
            let mut fulfil_event = None;
            let outcome_ok = if operation == "fulfil" {
                let result = SqliteInventoryRepository::fulfil_reservation_in_tx(
                    &tx,
                    target,
                    scaled(case.amount, scale),
                    "Lean conformance",
                    now,
                );
                let decision = match &result {
                    Ok(_) => "accepted",
                    Err(CommerceError::ValidationError(_)) => "invalid_quantity",
                    Err(CommerceError::Conflict(_)) => "closed",
                    Err(CommerceError::InsufficientStock { .. }) => "excess",
                    Err(error) => panic!("unexpected fulfilment error: {error}"),
                };
                assert_eq!(decision, case.decision, "{case:?} scale={scale}");
                fulfil_event = result.ok();
                true
            } else if operation == "confirm_quantity" {
                let outcome =
                    SqliteInventoryRepository::confirm_reservation_quantity_in_tx_with_now(
                        &tx,
                        target,
                        scaled(case.amount, scale),
                        now,
                    )
                    .expect("confirm");
                let should_expire = status == ReservationStatus::Expired
                    || (matches!(
                        status,
                        ReservationStatus::Pending | ReservationStatus::Allocated
                    ) && due);
                (outcome == ReservationConfirmOutcome::Expired) == should_expire
            } else {
                let expired =
                    SqliteInventoryRepository::expire_reservation_if_needed_in_tx(&tx, target, now)
                        .expect("expire");
                expired == (status == ReservationStatus::Expired || (status.holds_stock() && due))
            };

            let actual_balance: (String, String, String) = tx
                .query_row(
                    "SELECT quantity_on_hand, quantity_allocated, quantity_available
                     FROM inventory_balances WHERE item_id = ? AND location_id = 1",
                    [item.id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .expect("balance");
            let actual_balance = (
                actual_balance.0.parse::<Decimal>().expect("on hand"),
                actual_balance.1.parse::<Decimal>().expect("allocated"),
                actual_balance.2.parse::<Decimal>().expect("available"),
            );
            let mut stmt = tx
                .prepare("SELECT status, quantity FROM inventory_reservations WHERE item_id = ?")
                .expect("rows");
            let mut actual_rows: Vec<(String, Decimal)> = stmt
                .query_map([item.id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .expect("rows")
                .map(|r| {
                    let (status, quantity) = r.expect("row");
                    (status, quantity.parse().expect("exact quantity"))
                })
                .collect();
            let actual_target: (String, String) = tx
                .query_row(
                    "SELECT status, quantity FROM inventory_reservations WHERE id = ?",
                    [target.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .expect("target identity preserved");
            let expected_target = &case.after.rows[0];
            let mut expected_rows: Vec<_> = case
                .after
                .rows
                .iter()
                .map(|r| (r.status.clone(), scaled(r.quantity, scale)))
                .collect();
            actual_rows.sort();
            expected_rows.sort();
            let expected_balance = (
                scaled(case.after.on_hand, scale),
                scaled(case.after.allocated, scale),
                scaled(case.after.on_hand - case.after.allocated, scale),
            );
            if operation == "fulfil" {
                let accepted = case.decision == "accepted";
                let version: i32 = tx.query_row(
                    "SELECT version FROM inventory_balances WHERE item_id = ? AND location_id = 1",
                    [item.id], |r| r.get(0),
                ).unwrap();
                assert_eq!(version, 1 + i32::from(accepted));
                let mut shipment_stmt = tx.prepare(
                    "SELECT quantity, reference_type, reference_id FROM inventory_transactions WHERE item_id = ? AND transaction_type = 'shipment'",
                ).unwrap();
                let shipments: Vec<(String, String, String)> = shipment_stmt
                    .query_map([item.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                assert_eq!(shipments.len(), usize::from(accepted));
                if let Some((quantity, reference_type, reference_id)) = shipments.first() {
                    assert_eq!(
                        quantity.parse::<Decimal>().unwrap(),
                        -scaled(case.after.shipped - case.before.shipped, scale)
                    );
                    assert_eq!(
                        (reference_type.as_str(), reference_id.as_str()),
                        ("formal", "lean")
                    );
                }
                let mut event_stmt = tx
                    .prepare(
                        "SELECT id, event_type, payload FROM kernel_outbox WHERE aggregate_id = ?",
                    )
                    .unwrap();
                let events: Vec<(String, String, String)> = event_stmt
                    .query_map([target.to_string()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                assert_eq!(events.len(), usize::from(accepted));
                if let Some((id, event_type, payload)) = events.first() {
                    assert_eq!(id, &fulfil_event.unwrap().to_string());
                    assert_eq!(event_type, "inventory.reservation_fulfilled.v1");
                    let payload: serde_json::Value = serde_json::from_str(payload).unwrap();
                    assert_eq!(payload["reservation_id"], target.to_string());
                    assert_eq!(payload["item_id"], item.id);
                    assert_eq!(payload["location_id"], 1);
                    assert_eq!(payload["balance_version"], version);
                    assert_eq!(
                        payload["quantity"].as_str().unwrap().parse::<Decimal>().unwrap(),
                        scaled(case.amount, scale)
                    );
                    assert_eq!(
                        payload["remaining_quantity"].as_str().unwrap().parse::<Decimal>().unwrap(),
                        scaled(case.before.rows[0].quantity - case.amount, scale)
                    );
                }
            } else {
                assert_eq!(case.before.shipped, case.after.shipped, "these actions do not ship");
            }
            if !outcome_ok
                || actual_balance != expected_balance
                || actual_rows != expected_rows
                || actual_target.0 != expected_target.status
                || actual_target.1.parse::<Decimal>().expect("quantity")
                    != scaled(expected_target.quantity, scale)
            {
                failures.push(format!(
                    "{operation} status={status} quantity={} other={} amount={} time={} scale={scale}: \
                     balance actual: {actual_balance:?}, expected: {expected_balance:?}; \
                     rows actual: {actual_rows:?}, expected: {expected_rows:?}",
                    case.before.rows[0].quantity, case.before.rows[1].quantity,
                    case.amount, case.time,
                ));
            }
            // Dropping this transaction restores the fixture for the next independent case.
        }
    }
    assert_eq!(
        checked,
        match operation {
            "confirm_quantity" => 3024,
            "fulfil" => 945,
            _ => 756,
        },
        "the complete shared matrix must execute"
    );
    assert!(
        failures.is_empty(),
        "{} / {checked} mismatches:\n{}",
        failures.len(),
        failures.into_iter().take(8).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn quantity_confirmation_matches_lean() {
    run_cases("confirm_quantity");
}

#[test]
fn expiry_by_id_matches_lean() {
    run_cases("expire_if_due");
}

#[test]
fn fulfilment_matches_lean() {
    run_cases("fulfil");
}
