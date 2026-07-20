//! SQLite implementation of customer segment repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_json_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateSegment, CustomerId, Result, Segment, SegmentFilter, SegmentId,
    SegmentMembership, SegmentRepository, SegmentRule, UpdateSegment,
};

#[derive(Debug)]
pub struct SqliteSegmentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteSegmentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_segment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Segment> {
        let rules_json: String = row.get("rules")?;
        let rules: Vec<SegmentRule> = parse_json_row(&rules_json, "segment", "rules")?;

        Ok(Segment {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "segment", "id")?.into(),
            name: row.get("name")?,
            description: row.get("description")?,
            segment_type: parse_enum_row(
                &row.get::<_, String>("segment_type")?,
                "segment",
                "segment_type",
            )?,
            rules,
            member_count: row.get::<_, i64>("member_count")? as u64,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "segment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "segment",
                "updated_at",
            )?,
        })
    }

    fn row_to_membership(row: &rusqlite::Row<'_>) -> rusqlite::Result<SegmentMembership> {
        Ok(SegmentMembership {
            segment_id: parse_uuid_row(
                &row.get::<_, String>("segment_id")?,
                "segment_membership",
                "segment_id",
            )?
            .into(),
            customer_id: parse_uuid_row(
                &row.get::<_, String>("customer_id")?,
                "segment_membership",
                "customer_id",
            )?
            .into(),
            joined_at: parse_datetime_row(
                &row.get::<_, String>("joined_at")?,
                "segment_membership",
                "joined_at",
            )?,
        })
    }
}

impl SegmentRepository for SqliteSegmentRepository {
    fn create(&self, input: CreateSegment) -> Result<Segment> {
        let id = SegmentId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        let rules_json = serde_json::to_string(&input.rules)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO segments (id, name, description, segment_type, rules, member_count, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.description,
                    input.segment_type.to_string(),
                    &rules_json,
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM segments WHERE id = ?", [&id_str], Self::row_to_segment)
        })
    }

    fn get(&self, id: SegmentId) -> Result<Option<Segment>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM segments WHERE id = ?",
            [id.to_string()],
            Self::row_to_segment,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: SegmentId, input: UpdateSegment) -> Result<Segment> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref description) = input.description {
                sets.push("description = ?".into());
                params.push(Box::new(description.clone()));
            }
            if let Some(ref rules) = input.rules {
                let rules_json = serde_json::to_string(rules).map_err(|e| {
                    rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
                        e.to_string(),
                    )))
                })?;
                sets.push("rules = ?".into());
                params.push(Box::new(rules_json));
            }

            let sql = format!("UPDATE segments SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM segments WHERE id = ?", [&id_str], Self::row_to_segment)
        })
    }

    fn list(&self, filter: SegmentFilter) -> Result<Vec<Segment>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM segments WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(segment_type) = filter.segment_type {
            sql.push_str(" AND segment_type = ?");
            params.push(Box::new(segment_type.to_string()));
        }
        if let Some(ref name) = filter.name {
            sql.push_str(" AND name LIKE ?");
            params.push(Box::new(format!("%{name}%")));
        }

        sql.push_str(" ORDER BY created_at DESC");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_segment)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: SegmentId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM segment_memberships WHERE segment_id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        conn.execute("DELETE FROM segments WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn add_member(
        &self,
        segment_id: SegmentId,
        customer_id: CustomerId,
    ) -> Result<SegmentMembership> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let seg_str = segment_id.to_string();
        let cust_str = customer_id.to_string();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT OR IGNORE INTO segment_memberships (segment_id, customer_id, joined_at) VALUES (?, ?, ?)",
                rusqlite::params![&seg_str, &cust_str, &now_str],
            )?;

            // Update cached member count
            tx.execute(
                "UPDATE segments SET member_count = (SELECT COUNT(*) FROM segment_memberships WHERE segment_id = ?), updated_at = ? WHERE id = ?",
                rusqlite::params![&seg_str, &now_str, &seg_str],
            )?;

            tx.query_row(
                "SELECT * FROM segment_memberships WHERE segment_id = ? AND customer_id = ?",
                rusqlite::params![&seg_str, &cust_str],
                Self::row_to_membership,
            )
        })
    }

    fn remove_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<()> {
        let seg_str = segment_id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "DELETE FROM segment_memberships WHERE segment_id = ? AND customer_id = ?",
                rusqlite::params![&seg_str, customer_id.to_string()],
            )?;

            tx.execute(
                "UPDATE segments SET member_count = (SELECT COUNT(*) FROM segment_memberships WHERE segment_id = ?), updated_at = ? WHERE id = ?",
                rusqlite::params![&seg_str, &now_str, &seg_str],
            )?;

            Ok(())
        })
    }

    fn list_members(
        &self,
        segment_id: SegmentId,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<SegmentMembership>> {
        let conn = self.conn()?;
        let mut sql =
            "SELECT * FROM segment_memberships WHERE segment_id = ? ORDER BY joined_at DESC"
                .to_string();

        crate::sqlite::append_limit_offset(&mut sql, limit, offset);

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map([segment_id.to_string()], Self::row_to_membership)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn is_member(&self, segment_id: SegmentId, customer_id: CustomerId) -> Result<bool> {
        let conn = self.conn()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM segment_memberships WHERE segment_id = ? AND customer_id = ?",
                rusqlite::params![segment_id.to_string(), customer_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        Ok(count > 0)
    }

    fn count_members(&self, segment_id: SegmentId) -> Result<u64> {
        let conn = self.conn()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM segment_memberships WHERE segment_id = ?",
                [segment_id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;
        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::SegmentType;

    fn test_repo() -> SqliteSegmentRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS segments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                segment_type TEXT NOT NULL DEFAULT 'static',
                rules TEXT NOT NULL DEFAULT '[]',
                member_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS segment_memberships (
                segment_id TEXT NOT NULL,
                customer_id TEXT NOT NULL,
                joined_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (segment_id, customer_id),
                FOREIGN KEY (segment_id) REFERENCES segments(id)
            );",
        )
        .expect("create tables");
        SqliteSegmentRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_segment() {
        let repo = test_repo();
        let segment = repo
            .create(CreateSegment {
                name: "VIP Customers".into(),
                description: Some("High-value customers".into()),
                segment_type: SegmentType::Static,
                rules: vec![],
            })
            .expect("create");

        assert_eq!(segment.name, "VIP Customers");
        assert_eq!(segment.member_count, 0);

        let fetched = repo.get(segment.id).expect("get").expect("found");
        assert_eq!(fetched.id, segment.id);
        assert_eq!(fetched.name, "VIP Customers");
    }

    #[test]
    fn add_and_count_members() {
        let repo = test_repo();
        let segment = repo
            .create(CreateSegment {
                name: "Test Segment".into(),
                description: None,
                segment_type: SegmentType::Static,
                rules: vec![],
            })
            .expect("create");

        let c1 = CustomerId::new();
        let c2 = CustomerId::new();

        repo.add_member(segment.id, c1).expect("add c1");
        repo.add_member(segment.id, c2).expect("add c2");

        assert!(repo.is_member(segment.id, c1).expect("is_member c1"));
        assert!(repo.is_member(segment.id, c2).expect("is_member c2"));
        assert_eq!(repo.count_members(segment.id).expect("count"), 2);

        let members = repo.list_members(segment.id, None, None).expect("list members");
        assert_eq!(members.len(), 2);

        repo.remove_member(segment.id, c1).expect("remove c1");
        assert!(!repo.is_member(segment.id, c1).expect("not member"));
        assert_eq!(repo.count_members(segment.id).expect("count after remove"), 1);
    }

    #[test]
    fn rules_round_trip() {
        // Guards against the tiers-class bug: a modeled Vec field that is
        // persisted but never asserted round-trip. Segments store rules as a
        // JSON column; verify a non-empty rule set survives create + re-read.
        use stateset_core::{SegmentOperator, SegmentRule};

        let repo = test_repo();
        let created = repo
            .create(CreateSegment {
                name: "High spenders".into(),
                description: None,
                segment_type: SegmentType::Dynamic,
                rules: vec![
                    SegmentRule {
                        field: "lifetime_value".into(),
                        operator: SegmentOperator::Gte,
                        value: "1000".into(),
                    },
                    SegmentRule {
                        field: "country".into(),
                        operator: SegmentOperator::Eq,
                        value: "US".into(),
                    },
                ],
            })
            .expect("create");
        assert_eq!(created.rules.len(), 2, "rules returned on create");

        let fetched = repo.get(created.id).expect("get").expect("found");
        assert_eq!(fetched.rules.len(), 2, "rules survive a re-read");
        assert_eq!(fetched.rules[0].field, "lifetime_value");
        assert_eq!(fetched.rules[0].operator, SegmentOperator::Gte);
        assert_eq!(fetched.rules[1].value, "US");
    }
}
