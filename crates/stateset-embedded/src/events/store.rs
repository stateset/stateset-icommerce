//! Event store implementations for persisting events

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_core::{CommerceEvent, EventStore, Result};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Stored event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Sequence number (monotonically increasing)
    pub sequence: u64,
    /// Unique event ID
    pub id: Uuid,
    /// Event type string
    pub event_type: String,
    /// Aggregate type (e.g., "order", "customer")
    pub aggregate_type: Option<String>,
    /// Aggregate ID
    pub aggregate_id: Option<String>,
    /// Serialized event data
    pub data: String,
    /// When the event was stored
    pub stored_at: DateTime<Utc>,
}

/// In-memory event store for testing
pub struct InMemoryEventStore {
    events: Arc<RwLock<VecDeque<StoredEvent>>>,
    sequence: Arc<RwLock<u64>>,
    max_events: usize,
}

impl InMemoryEventStore {
    /// Create a new in-memory event store
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(max_events))),
            sequence: Arc::new(RwLock::new(0)),
            max_events,
        }
    }

    /// Get aggregate info from event
    fn extract_aggregate(event: &CommerceEvent) -> (Option<String>, Option<String>) {
        match event {
            CommerceEvent::OrderCreated { order_id, .. }
            | CommerceEvent::OrderStatusChanged { order_id, .. }
            | CommerceEvent::OrderPaymentStatusChanged { order_id, .. }
            | CommerceEvent::OrderFulfillmentStatusChanged { order_id, .. }
            | CommerceEvent::OrderCancelled { order_id, .. }
            | CommerceEvent::OrderItemAdded { order_id, .. }
            | CommerceEvent::OrderItemRemoved { order_id, .. } => {
                (Some("order".to_string()), Some(order_id.to_string()))
            }
            CommerceEvent::CustomerCreated { customer_id, .. }
            | CommerceEvent::CustomerUpdated { customer_id, .. }
            | CommerceEvent::CustomerStatusChanged { customer_id, .. }
            | CommerceEvent::CustomerAddressAdded { customer_id, .. } => {
                (Some("customer".to_string()), Some(customer_id.to_string()))
            }
            CommerceEvent::ProductCreated { product_id, .. }
            | CommerceEvent::ProductUpdated { product_id, .. }
            | CommerceEvent::ProductStatusChanged { product_id, .. } => {
                (Some("product".to_string()), Some(product_id.to_string()))
            }
            CommerceEvent::ProductVariantAdded { variant_id, .. }
            | CommerceEvent::ProductVariantUpdated { variant_id, .. } => {
                (Some("variant".to_string()), Some(variant_id.to_string()))
            }
            CommerceEvent::InventoryItemCreated { item_id, .. }
            | CommerceEvent::InventoryAdjusted { item_id, .. } => {
                (Some("inventory".to_string()), Some(item_id.to_string()))
            }
            CommerceEvent::InventoryReserved { reservation_id, .. }
            | CommerceEvent::InventoryReservationReleased { reservation_id, .. }
            | CommerceEvent::InventoryReservationConfirmed { reservation_id, .. } => (
                Some("reservation".to_string()),
                Some(reservation_id.to_string()),
            ),
            CommerceEvent::LowStockAlert { sku, .. } => {
                (Some("inventory".to_string()), Some(sku.clone()))
            }
            CommerceEvent::ReturnRequested { return_id, .. }
            | CommerceEvent::ReturnStatusChanged { return_id, .. }
            | CommerceEvent::ReturnApproved { return_id, .. }
            | CommerceEvent::ReturnRejected { return_id, .. }
            | CommerceEvent::ReturnCompleted { return_id, .. }
            | CommerceEvent::RefundIssued { return_id, .. } => {
                (Some("return".to_string()), Some(return_id.to_string()))
            }
        }
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, event: &CommerceEvent) -> Result<u64> {
        let mut sequence = self.sequence.write().unwrap();
        *sequence += 1;
        let seq = *sequence;

        let (aggregate_type, aggregate_id) = Self::extract_aggregate(event);

        let stored = StoredEvent {
            sequence: seq,
            id: Uuid::new_v4(),
            event_type: event.event_type().to_string(),
            aggregate_type,
            aggregate_id,
            data: event.to_json().map_err(|e| {
                stateset_core::CommerceError::Internal(format!(
                    "Failed to serialize event: {}",
                    e
                ))
            })?,
            stored_at: Utc::now(),
        };

        let mut events = self.events.write().unwrap();
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(stored);

        Ok(seq)
    }

    fn get_events_since(&self, sequence: u64, limit: u32) -> Result<Vec<(u64, CommerceEvent)>> {
        let events = self.events.read().unwrap();
        let result: Vec<(u64, CommerceEvent)> = events
            .iter()
            .filter(|e| e.sequence > sequence)
            .take(limit as usize)
            .filter_map(|e| {
                CommerceEvent::from_json(&e.data)
                    .ok()
                    .map(|event| (e.sequence, event))
            })
            .collect();
        Ok(result)
    }

    fn get_events_for_aggregate(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<CommerceEvent>> {
        let events = self.events.read().unwrap();
        let result: Vec<CommerceEvent> = events
            .iter()
            .filter(|e| {
                e.aggregate_type.as_deref() == Some(aggregate_type)
                    && e.aggregate_id.as_deref() == Some(aggregate_id)
            })
            .filter_map(|e| CommerceEvent::from_json(&e.data).ok())
            .collect();
        Ok(result)
    }

    fn latest_sequence(&self) -> Result<u64> {
        Ok(*self.sequence.read().unwrap())
    }
}

/// SQLite event store implementation
#[cfg(feature = "sqlite-events")]
pub struct SqliteEventStore {
    pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
}

#[cfg(feature = "sqlite-events")]
impl SqliteEventStore {
    /// Create a new SQLite event store
    pub fn new(pool: r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>) -> Result<Self> {
        let store = Self { pool };
        store.create_table()?;
        Ok(store)
    }

    fn create_table(&self) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to get connection: {}", e))
        })?;

        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS commerce_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                aggregate_type TEXT,
                aggregate_id TEXT,
                data TEXT NOT NULL,
                stored_at TEXT NOT NULL
            )
            "#,
            [],
        )
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to create table: {}", e))
        })?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_aggregate ON commerce_events(aggregate_type, aggregate_id)",
            [],
        ).map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to create index: {}", e))
        })?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_events_type ON commerce_events(event_type)",
            [],
        )
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to create index: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(feature = "sqlite-events")]
impl EventStore for SqliteEventStore {
    fn append(&self, event: &CommerceEvent) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to get connection: {}", e))
        })?;

        let (aggregate_type, aggregate_id) = InMemoryEventStore::extract_aggregate(event);
        let data = event.to_json().map_err(|e| {
            stateset_core::CommerceError::Internal(format!(
                "Failed to serialize event: {}",
                e
            ))
        })?;

        conn.execute(
            r#"
            INSERT INTO commerce_events (id, event_type, aggregate_type, aggregate_id, data, stored_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                Uuid::new_v4().to_string(),
                event.event_type(),
                aggregate_type,
                aggregate_id,
                data,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to insert event: {}", e))
        })?;

        let sequence = conn.last_insert_rowid() as u64;
        Ok(sequence)
    }

    fn get_events_since(&self, sequence: u64, limit: u32) -> Result<Vec<(u64, CommerceEvent)>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                r#"
            SELECT sequence, data FROM commerce_events
            WHERE sequence > ?1
            ORDER BY sequence ASC
            LIMIT ?2
            "#,
            )
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!(
                    "Failed to prepare statement: {}",
                    e
                ))
            })?;

        let rows = stmt
            .query_map(rusqlite::params![sequence as i64, limit], |row| {
                let seq: i64 = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((seq as u64, data))
            })
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Failed to query events: {}", e))
            })?;

        let mut events = Vec::new();
        for row in rows {
            let (seq, data) = row.map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Failed to read row: {}", e))
            })?;
            if let Ok(event) = CommerceEvent::from_json(&data) {
                events.push((seq, event));
            }
        }

        Ok(events)
    }

    fn get_events_for_aggregate(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<CommerceEvent>> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to get connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                r#"
            SELECT data FROM commerce_events
            WHERE aggregate_type = ?1 AND aggregate_id = ?2
            ORDER BY sequence ASC
            "#,
            )
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!(
                    "Failed to prepare statement: {}",
                    e
                ))
            })?;

        let rows = stmt
            .query_map(rusqlite::params![aggregate_type, aggregate_id], |row| {
                let data: String = row.get(0)?;
                Ok(data)
            })
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Failed to query events: {}", e))
            })?;

        let mut events = Vec::new();
        for row in rows {
            let data = row.map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!("Failed to read row: {}", e))
            })?;
            if let Ok(event) = CommerceEvent::from_json(&data) {
                events.push(event);
            }
        }

        Ok(events)
    }

    fn latest_sequence(&self) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to get connection: {}", e))
        })?;

        let sequence: Option<i64> = conn
            .query_row(
                "SELECT MAX(sequence) FROM commerce_events",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!(
                    "Failed to get latest sequence: {}",
                    e
                ))
            })?;

        Ok(sequence.unwrap_or(0) as u64)
    }
}

/// PostgreSQL event store implementation
#[cfg(feature = "postgres")]
pub struct PostgresEventStore {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresEventStore {
    /// Create a new PostgreSQL event store
    pub async fn new(pool: sqlx::PgPool) -> Result<Self> {
        let store = Self { pool };
        store.create_table().await?;
        Ok(store)
    }

    async fn create_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS commerce_events (
                sequence BIGSERIAL PRIMARY KEY,
                id UUID NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                aggregate_type TEXT,
                aggregate_id TEXT,
                data JSONB NOT NULL,
                stored_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to create table: {}", e))
        })?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_events_aggregate ON commerce_events(aggregate_type, aggregate_id)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to create index: {}", e))
        })?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_events_type ON commerce_events(event_type)")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                stateset_core::CommerceError::DatabaseError(format!(
                    "Failed to create index: {}",
                    e
                ))
            })?;

        Ok(())
    }

    /// Append an event (async version)
    pub async fn append_async(&self, event: &CommerceEvent) -> Result<u64> {
        let (aggregate_type, aggregate_id) = InMemoryEventStore::extract_aggregate(event);
        let data = serde_json::to_value(event).map_err(|e| {
            stateset_core::CommerceError::Internal(format!(
                "Failed to serialize event: {}",
                e
            ))
        })?;

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO commerce_events (id, event_type, aggregate_type, aggregate_id, data)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING sequence
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(event.event_type())
        .bind(aggregate_type)
        .bind(aggregate_id)
        .bind(data)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to insert event: {}", e))
        })?;

        Ok(row.0 as u64)
    }

    /// Get events since sequence (async version)
    pub async fn get_events_since_async(
        &self,
        sequence: u64,
        limit: u32,
    ) -> Result<Vec<(u64, CommerceEvent)>> {
        let rows: Vec<(i64, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT sequence, data FROM commerce_events
            WHERE sequence > $1
            ORDER BY sequence ASC
            LIMIT $2
            "#,
        )
        .bind(sequence as i64)
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to query events: {}", e))
        })?;

        let mut events = Vec::new();
        for (seq, data) in rows {
            if let Ok(event) = serde_json::from_value(data) {
                events.push((seq as u64, event));
            }
        }

        Ok(events)
    }

    /// Get events for aggregate (async version)
    pub async fn get_events_for_aggregate_async(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<CommerceEvent>> {
        let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
            r#"
            SELECT data FROM commerce_events
            WHERE aggregate_type = $1 AND aggregate_id = $2
            ORDER BY sequence ASC
            "#,
        )
        .bind(aggregate_type)
        .bind(aggregate_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            stateset_core::CommerceError::DatabaseError(format!("Failed to query events: {}", e))
        })?;

        let mut events = Vec::new();
        for (data,) in rows {
            if let Ok(event) = serde_json::from_value(data) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Get latest sequence (async version)
    pub async fn latest_sequence_async(&self) -> Result<u64> {
        let row: Option<(Option<i64>,)> =
            sqlx::query_as("SELECT MAX(sequence) FROM commerce_events")
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    stateset_core::CommerceError::DatabaseError(format!(
                        "Failed to get latest sequence: {}",
                        e
                    ))
                })?;

        Ok(row.and_then(|(s,)| s).unwrap_or(0) as u64)
    }
}

#[cfg(feature = "postgres")]
impl EventStore for PostgresEventStore {
    fn append(&self, event: &CommerceEvent) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.append_async(event))
    }

    fn get_events_since(&self, sequence: u64, limit: u32) -> Result<Vec<(u64, CommerceEvent)>> {
        tokio::runtime::Handle::current().block_on(self.get_events_since_async(sequence, limit))
    }

    fn get_events_for_aggregate(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<CommerceEvent>> {
        tokio::runtime::Handle::current()
            .block_on(self.get_events_for_aggregate_async(aggregate_type, aggregate_id))
    }

    fn latest_sequence(&self) -> Result<u64> {
        tokio::runtime::Handle::current().block_on(self.latest_sequence_async())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_in_memory_store() {
        let store = InMemoryEventStore::new(100);

        let event = CommerceEvent::OrderCreated {
            order_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            total_amount: dec!(100.00),
            item_count: 2,
            timestamp: Utc::now(),
        };

        let seq = store.append(&event).unwrap();
        assert_eq!(seq, 1);

        let events = store.get_events_since(0, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, 1);
    }

    #[test]
    fn test_in_memory_store_max_events() {
        let store = InMemoryEventStore::new(2);

        for i in 0..5 {
            let event = CommerceEvent::CustomerCreated {
                customer_id: Uuid::new_v4(),
                email: format!("test{}@example.com", i),
                timestamp: Utc::now(),
            };
            store.append(&event).unwrap();
        }

        // Should only keep last 2 events
        let events = store.get_events_since(0, 10).unwrap();
        assert_eq!(events.len(), 2);
    }
}
