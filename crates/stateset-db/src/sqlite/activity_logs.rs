//! SQLite implementation of the activity log repository

use super::{map_db_error, parse_datetime_row, parse_enum_row, parse_json_row, parse_uuid_row};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    ActivityLogEntry, ActivityLogFilter, ActivityLogId, ActivityLogRepository, ActorKind,
    CommerceError, RecordActivity, Result,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteActivityLogRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteActivityLogRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityLogEntry> {
        let metadata_json: String = row.get("metadata")?;
        Ok(ActivityLogEntry {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "activity_log", "id")?.into(),
            subject_type: row.get("subject_type")?,
            subject_id: parse_uuid_row(
                &row.get::<_, String>("subject_id")?,
                "activity_log",
                "subject_id",
            )?,
            action: row.get("action")?,
            summary: row.get("summary")?,
            actor_kind: parse_enum_row::<ActorKind>(
                &row.get::<_, String>("actor_kind")?,
                "activity_log",
                "actor_kind",
            )?,
            actor: row.get("actor")?,
            metadata: parse_json_row(&metadata_json, "activity_log", "metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "activity_log",
                "created_at",
            )?,
        })
    }
}

impl ActivityLogRepository for SqliteActivityLogRepository {
    fn record(&self, input: RecordActivity) -> Result<ActivityLogEntry> {
        let id = ActivityLogId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&input.metadata)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO activity_logs (id, subject_type, subject_id, action, summary, actor_kind, actor, metadata, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                &id_str,
                &input.subject_type,
                input.subject_id.to_string(),
                &input.action,
                &input.summary,
                input.actor_kind.to_string(),
                &input.actor,
                &metadata_json,
                &now_str,
            ],
        )
        .map_err(map_db_error)?;
        conn.query_row("SELECT * FROM activity_logs WHERE id = ?", [&id_str], Self::row_to_entry)
            .map_err(map_db_error)
    }

    fn get(&self, id: ActivityLogId) -> Result<Option<ActivityLogEntry>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM activity_logs WHERE id = ?",
            [id.to_string()],
            Self::row_to_entry,
        ) {
            Ok(e) => Ok(Some(e)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM activity_logs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref subject_type) = filter.subject_type {
            sql.push_str(" AND subject_type = ?");
            params.push(Box::new(subject_type.clone()));
        }
        if let Some(subject_id) = filter.subject_id {
            sql.push_str(" AND subject_id = ?");
            params.push(Box::new(subject_id.to_string()));
        }
        if let Some(ref action) = filter.action {
            sql.push_str(" AND action = ?");
            params.push(Box::new(action.clone()));
        }
        if let Some(actor_kind) = filter.actor_kind {
            sql.push_str(" AND actor_kind = ?");
            params.push(Box::new(actor_kind.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
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
            .query_map(param_refs.as_slice(), Self::row_to_entry)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn history_for_subject(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<ActivityLogEntry>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM activity_logs WHERE subject_type = ? AND subject_id = ? ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params![subject_type, subject_id.to_string()], Self::row_to_entry)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteActivityLogRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteActivityLogRepository::new(db.pool().clone())
    }

    fn record(repo: &SqliteActivityLogRepository, subject: Uuid, action: &str) -> ActivityLogEntry {
        repo.record(RecordActivity {
            subject_type: "sales_order".into(),
            subject_id: subject,
            action: action.into(),
            summary: format!("did {action}"),
            actor_kind: ActorKind::User,
            actor: Some("alice".into()),
            metadata: serde_json::json!({"k": "v"}),
        })
        .expect("record")
    }

    #[test]
    fn record_and_get() {
        let repo = test_repo();
        let subject = Uuid::new_v4();
        let e = record(&repo, subject, "created");
        assert_eq!(e.actor_label(), "alice");
        let fetched = repo.get(e.id).expect("get").expect("found");
        assert_eq!(fetched.action, "created");
        assert_eq!(fetched.metadata["k"], "v");
    }

    #[test]
    fn history_scoped_to_subject() {
        let repo = test_repo();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        record(&repo, a, "created");
        record(&repo, a, "status_changed");
        record(&repo, b, "created");
        let hist = repo.history_for_subject("sales_order", a).expect("history");
        assert_eq!(hist.len(), 2);
        // most recent first
        assert_eq!(hist[0].action, "status_changed");
    }

    #[test]
    fn list_filters_by_action() {
        let repo = test_repo();
        let subject = Uuid::new_v4();
        record(&repo, subject, "created");
        record(&repo, subject, "status_changed");
        let changed = repo
            .list(ActivityLogFilter { action: Some("status_changed".into()), ..Default::default() })
            .expect("list");
        assert_eq!(changed.len(), 1);
    }

    #[test]
    fn list_filters_by_subject() {
        let repo = test_repo();
        let a = Uuid::new_v4();
        record(&repo, a, "created");
        record(&repo, Uuid::new_v4(), "created");
        let scoped = repo
            .list(ActivityLogFilter { subject_id: Some(a), ..Default::default() })
            .expect("list");
        assert_eq!(scoped.len(), 1);
    }
}
