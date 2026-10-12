//! Run the same Lean-generated matrix as SQLite against a live PostgreSQL store.
use super::*;
use crate::PostgresDatabase;
use serde_json::Value;

fn number(value: &Value, key: &str, scale: u32) -> Decimal {
    Decimal::new(value[key].as_i64().expect("integral model quantity"), scale)
}

fn rows(value: &Value, scale: u32) -> Vec<(String, Decimal)> {
    value["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| (r["status"].as_str().expect("status").to_owned(), number(r, "quantity", scale)))
        .collect()
}

#[tokio::test]
async fn postgres_inventory_transitions_match_lean() {
    let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL"));
    let Ok(url) = url else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping live Lean conformance");
        return;
    };
    let db = PostgresDatabase::connect(url).await.expect("connect and migrate");
    let repo = db.inventory();
    let now: DateTime<Utc> = "2030-01-01T00:00:00Z".parse().expect("fixed time");
    // All fixture data is rolled back, including the item and its balance.
    let mut tx = db.pool().begin().await.expect("transaction");
    let item: i64 = sqlx::query_scalar(
        "INSERT INTO inventory_items (sku, name) VALUES ($1, 'Lean conformance') RETURNING id",
    )
    .bind(format!("LEAN-{}", Uuid::new_v4()))
    .fetch_one(tx.as_mut())
    .await
    .expect("item");
    sqlx::query("INSERT INTO inventory_balances (item_id, location_id) VALUES ($1, 1)")
        .bind(item)
        .execute(tx.as_mut())
        .await
        .expect("balance");
    let mut checked = 0;
    for line in include_str!("../../../../formal/lean/inventory_transitions.golden.jsonl").lines() {
        let case: Value = serde_json::from_str(line).expect("Lean case");
        for scale in [0, 2, 6] {
            sqlx::query("SAVEPOINT formal_case").execute(tx.as_mut()).await.expect("savepoint");
            let before = &case["before"];
            let after = &case["after"];
            let source = Uuid::new_v4();
            let time = case["time"].as_str().expect("time");
            let expires = match time {
                "past" => Some(now - chrono::Duration::seconds(1)),
                "now" => Some(now),
                "future" => Some(now + chrono::Duration::seconds(1)),
                "none" => None,
                _ => panic!("invalid time"),
            };
            for (i, (status, quantity)) in rows(before, scale).into_iter().enumerate() {
                sqlx::query(
                    "INSERT INTO inventory_reservations
                     (id, item_id, location_id, quantity, status, reference_type, reference_id, expires_at)
                     VALUES ($1, $2, 1, $3, $4, 'formal', 'lean', $5)",
                )
                .bind(if i == 0 { source } else { Uuid::new_v4() })
                .bind(item)
                .bind(quantity)
                .bind(status)
                .bind(if i == 0 { expires } else { None })
                .execute(tx.as_mut())
                .await
                .expect("seed row");
            }
            sqlx::query(
                "UPDATE inventory_balances SET quantity_on_hand = $1, quantity_allocated = $2,
                 quantity_available = $1 - $2, version = 1 WHERE item_id = $3 AND location_id = 1",
            )
            .bind(number(before, "on_hand", scale))
            .bind(number(before, "allocated", scale))
            .bind(item)
            .execute(tx.as_mut())
            .await
            .expect("seed balance");
            let status: ReservationStatus = before["rows"][0]["status"]
                .as_str()
                .expect("status")
                .parse()
                .expect("reservation status");
            let mut fulfil_event = None;
            if case["operation"] == "fulfil" {
                let result = PgInventoryRepository::fulfil_reservation_in_tx(
                    &mut tx,
                    source,
                    number(&case, "amount", scale),
                    "Lean conformance",
                    now,
                )
                .await;
                let decision = match &result {
                    Ok(_) => "accepted",
                    Err(CommerceError::ValidationError(_)) => "invalid_quantity",
                    Err(CommerceError::Conflict(_)) => "closed",
                    Err(CommerceError::InsufficientStock { .. }) => "excess",
                    Err(error) => panic!("unexpected fulfilment error: {error}"),
                };
                assert_eq!(decision, case["decision"].as_str().unwrap(), "{case} scale={scale}");
                fulfil_event = result.ok();
            } else if case["operation"] == "confirm_quantity" {
                let outcome = repo
                    .confirm_reservation_quantity_in_tx_with_now(
                        &mut tx,
                        source,
                        number(&case, "amount", scale),
                        now,
                    )
                    .await
                    .expect("confirm");
                let expired = status == ReservationStatus::Expired
                    || (matches!(
                        status,
                        ReservationStatus::Pending | ReservationStatus::Allocated
                    ) && time == "past");
                assert_eq!(outcome == ReservationConfirmOutcome::Expired, expired, "{case}");
            } else {
                let expired = repo
                    .expire_reservation_if_needed_in_tx(&mut tx, source, now)
                    .await
                    .expect("expire");
                assert_eq!(
                    expired,
                    status == ReservationStatus::Expired
                        || (status.holds_stock() && time == "past"),
                    "{case}"
                );
            }
            let balance: (Decimal, Decimal, Decimal) = sqlx::query_as(
                "SELECT quantity_on_hand, quantity_allocated, quantity_available
                 FROM inventory_balances WHERE item_id = $1 AND location_id = 1",
            )
            .bind(item)
            .fetch_one(tx.as_mut())
            .await
            .expect("read balance");
            assert_eq!(
                balance,
                (
                    number(after, "on_hand", scale),
                    number(after, "allocated", scale),
                    number(after, "on_hand", scale) - number(after, "allocated", scale),
                ),
                "case={case} scale={scale}"
            );
            let mut actual: Vec<(String, Decimal)> = sqlx::query_as(
                "SELECT status, quantity FROM inventory_reservations WHERE item_id = $1",
            )
            .bind(item)
            .fetch_all(tx.as_mut())
            .await
            .expect("read rows");
            let mut expected = rows(after, scale);
            let target: (String, Decimal) =
                sqlx::query_as("SELECT status, quantity FROM inventory_reservations WHERE id = $1")
                    .bind(source)
                    .fetch_one(tx.as_mut())
                    .await
                    .expect("source identity");
            assert_eq!(target, expected[0], "case={case} scale={scale}");
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "case={case} scale={scale}");
            if case["operation"] == "fulfil" {
                let accepted = case["decision"] == "accepted";
                let version: i32 = sqlx::query_scalar(
                    "SELECT version FROM inventory_balances WHERE item_id = $1 AND location_id = 1",
                )
                .bind(item)
                .fetch_one(tx.as_mut())
                .await
                .unwrap();
                assert_eq!(version, 1 + i32::from(accepted));
                let shipments: Vec<(Decimal, String, String)> = sqlx::query_as(
                    "SELECT quantity, reference_type, reference_id FROM inventory_transactions WHERE item_id = $1 AND transaction_type = 'shipment'",
                ).bind(item).fetch_all(tx.as_mut()).await.unwrap();
                assert_eq!(shipments.len(), usize::from(accepted));
                if let Some((quantity, reference_type, reference_id)) = shipments.first() {
                    assert_eq!(
                        *quantity,
                        number(before, "shipped", scale) - number(after, "shipped", scale)
                    );
                    assert_eq!(
                        (reference_type.as_str(), reference_id.as_str()),
                        ("formal", "lean")
                    );
                }
                let events: Vec<(Uuid, String, Value)> = sqlx::query_as(
                    "SELECT id, event_type, payload FROM kernel_outbox WHERE aggregate_id = $1",
                )
                .bind(source.to_string())
                .fetch_all(tx.as_mut())
                .await
                .unwrap();
                assert_eq!(events.len(), usize::from(accepted));
                if let Some((id, event_type, payload)) = events.first() {
                    assert_eq!(*id, fulfil_event.unwrap());
                    assert_eq!(event_type, "inventory.reservation_fulfilled.v1");
                    assert_eq!(payload["reservation_id"], source.to_string());
                    assert_eq!(payload["item_id"], item);
                    assert_eq!(payload["location_id"], 1);
                    assert_eq!(payload["balance_version"], version);
                    assert_eq!(
                        payload["quantity"].as_str().unwrap().parse::<Decimal>().unwrap(),
                        number(&case, "amount", scale)
                    );
                    assert_eq!(
                        payload["remaining_quantity"].as_str().unwrap().parse::<Decimal>().unwrap(),
                        number(&before["rows"][0], "quantity", scale)
                            - number(&case, "amount", scale)
                    );
                }
            }
            sqlx::query("ROLLBACK TO SAVEPOINT formal_case")
                .execute(tx.as_mut())
                .await
                .expect("restore");
            sqlx::query("RELEASE SAVEPOINT formal_case")
                .execute(tx.as_mut())
                .await
                .expect("release");
            checked += 1;
        }
    }
    assert_eq!(checked, 4725, "the complete shared matrix must execute");
    tx.rollback().await.expect("rollback fixtures");
}
