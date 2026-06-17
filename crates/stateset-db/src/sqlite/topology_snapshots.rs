//! SQLite implementation of the topology snapshot repository

use super::{map_db_error, parse_datetime_row, parse_enum_row, parse_json_row, parse_uuid_row};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CaptureTopologySnapshot, CommerceError, HealthGrade, Result, TopologySnapshot,
    TopologySnapshotFilter, TopologySnapshotId, TopologySnapshotRepository,
};

#[derive(Debug)]
pub struct SqliteTopologySnapshotRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteTopologySnapshotRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_snapshot(row: &rusqlite::Row<'_>) -> rusqlite::Result<TopologySnapshot> {
        let signals_json: String = row.get("signals")?;
        Ok(TopologySnapshot {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "topology_snapshot", "id")?.into(),
            channels_total: row.get::<_, i64>("channels_total")? as u64,
            channels_active: row.get::<_, i64>("channels_active")? as u64,
            warehouses_total: row.get::<_, i64>("warehouses_total")? as u64,
            products_total: row.get::<_, i64>("products_total")? as u64,
            open_orders: row.get::<_, i64>("open_orders")? as u64,
            health: parse_enum_row::<HealthGrade>(
                &row.get::<_, String>("health")?,
                "topology_snapshot",
                "health",
            )?,
            signals: parse_json_row(&signals_json, "topology_snapshot", "signals")?,
            captured_at: parse_datetime_row(
                &row.get::<_, String>("captured_at")?,
                "topology_snapshot",
                "captured_at",
            )?,
        })
    }
}

impl TopologySnapshotRepository for SqliteTopologySnapshotRepository {
    fn capture(&self, input: CaptureTopologySnapshot) -> Result<TopologySnapshot> {
        let id = TopologySnapshotId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let health = input.derive_health();
        let signals_json = serde_json::to_string(&input.signals)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO topology_snapshots (id, channels_total, channels_active, warehouses_total, products_total, open_orders, health, signals, captured_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &id_str,
                input.channels_total as i64,
                input.channels_active as i64,
                input.warehouses_total as i64,
                input.products_total as i64,
                input.open_orders as i64,
                health.to_string(),
                &signals_json,
                &now,
            ],
        )
        .map_err(map_db_error)?;
        conn.query_row(
            "SELECT * FROM topology_snapshots WHERE id = ?",
            [&id_str],
            Self::row_to_snapshot,
        )
        .map_err(map_db_error)
    }

    fn get(&self, id: TopologySnapshotId) -> Result<Option<TopologySnapshot>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM topology_snapshots WHERE id = ?",
            [id.to_string()],
            Self::row_to_snapshot,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn latest(&self) -> Result<Option<TopologySnapshot>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM topology_snapshots ORDER BY captured_at DESC LIMIT 1",
            [],
            Self::row_to_snapshot,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: TopologySnapshotFilter) -> Result<Vec<TopologySnapshot>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM topology_snapshots WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(health) = filter.health {
            sql.push_str(" AND health = ?");
            params.push(Box::new(health.to_string()));
        }
        sql.push_str(" ORDER BY captured_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_snapshot)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: TopologySnapshotId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM topology_snapshots WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteTopologySnapshotRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteTopologySnapshotRepository::new(db.pool().clone())
    }

    fn capture(repo: &SqliteTopologySnapshotRepository, active: u64) -> TopologySnapshot {
        repo.capture(CaptureTopologySnapshot {
            channels_total: 2,
            channels_active: active,
            warehouses_total: 1,
            products_total: 50,
            open_orders: 3,
            signals: serde_json::json!({"sync": "ok"}),
        })
        .expect("capture")
    }

    #[test]
    fn capture_derives_health() {
        let repo = test_repo();
        let healthy = capture(&repo, 1);
        assert_eq!(healthy.health, HealthGrade::Healthy);
        let critical = capture(&repo, 0);
        assert_eq!(critical.health, HealthGrade::Critical);
    }

    #[test]
    fn latest_returns_most_recent() {
        let repo = test_repo();
        capture(&repo, 1);
        let second = capture(&repo, 0);
        let latest = repo.latest().expect("latest").expect("found");
        assert_eq!(latest.id, second.id);
    }

    #[test]
    fn latest_none_when_empty() {
        let repo = test_repo();
        assert!(repo.latest().expect("latest").is_none());
    }

    #[test]
    fn list_filters_by_health() {
        let repo = test_repo();
        capture(&repo, 1); // healthy
        capture(&repo, 0); // critical
        let critical = repo
            .list(TopologySnapshotFilter {
                health: Some(HealthGrade::Critical),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(critical.len(), 1);
    }

    #[test]
    fn get_and_delete() {
        let repo = test_repo();
        let s = capture(&repo, 1);
        assert!(repo.get(s.id).expect("get").is_some());
        repo.delete(s.id).expect("delete");
        assert!(repo.get(s.id).expect("get").is_none());
    }
}
