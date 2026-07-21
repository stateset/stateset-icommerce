//! PostgreSQL activity log repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    ActivityLogEntry, ActivityLogFilter, ActivityLogId, ActivityLogRepository, ActorKind,
    CommerceError, RecordActivity, Result,
};
use uuid::Uuid;

/// PostgreSQL implementation of `ActivityLogRepository`
#[derive(Debug, Clone)]
pub struct PgActivityLogRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ActivityLogRow {
    id: Uuid,
    subject_type: String,
    subject_id: Uuid,
    action: String,
    summary: String,
    actor_kind: String,
    actor: Option<String>,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl PgActivityLogRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_entry(row: ActivityLogRow) -> Result<ActivityLogEntry> {
        let actor_kind: ActorKind = row.actor_kind.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid activity_log.actor_kind '{}': {}",
                row.actor_kind, e
            ))
        })?;
        Ok(ActivityLogEntry {
            id: row.id.into(),
            subject_type: row.subject_type,
            subject_id: row.subject_id,
            action: row.action,
            summary: row.summary,
            actor_kind,
            actor: row.actor,
            metadata: row.metadata,
            created_at: row.created_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<ActivityLogEntry>> {
        let row = sqlx::query_as::<_, ActivityLogRow>("SELECT * FROM activity_logs WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_entry).transpose()
    }

    /// Record a new activity log entry (async)
    pub async fn record_async(&self, input: RecordActivity) -> Result<ActivityLogEntry> {
        let id = ActivityLogId::new();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO activity_logs (id, subject_type, subject_id, action, summary, actor_kind, actor, metadata, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(Uuid::from(id))
        .bind(&input.subject_type)
        .bind(input.subject_id)
        .bind(&input.action)
        .bind(&input.summary)
        .bind(input.actor_kind.to_string())
        .bind(&input.actor)
        .bind(&input.metadata)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get an entry by ID (async)
    pub async fn get_async(&self, id: ActivityLogId) -> Result<Option<ActivityLogEntry>> {
        self.fetch_async(id.into()).await
    }

    /// List entries with filter (async), most recent first.
    pub async fn list_async(&self, filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM activity_logs WHERE 1=1");
        let mut param_idx = 1;
        if filter.subject_type.is_some() {
            query.push_str(&format!(" AND subject_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.subject_id.is_some() {
            query.push_str(&format!(" AND subject_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.action.is_some() {
            query.push_str(&format!(" AND action = ${param_idx}"));
            param_idx += 1;
        }
        if filter.actor_kind.is_some() {
            query.push_str(&format!(" AND actor_kind = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, ActivityLogRow>(&query);
        if let Some(subject_type) = filter.subject_type {
            q = q.bind(subject_type);
        }
        if let Some(subject_id) = filter.subject_id {
            q = q.bind(subject_id);
        }
        if let Some(action) = filter.action {
            q = q.bind(action);
        }
        if let Some(actor_kind) = filter.actor_kind {
            q = q.bind(actor_kind.to_string());
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_entry).collect()
    }

    /// Full (unpaginated) history for a subject (async), most recent first.
    pub async fn history_for_subject_async(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<ActivityLogEntry>> {
        let rows = sqlx::query_as::<_, ActivityLogRow>(
            "SELECT * FROM activity_logs WHERE subject_type = $1 AND subject_id = $2 ORDER BY created_at DESC",
        )
        .bind(subject_type)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_entry).collect()
    }
}

impl ActivityLogRepository for PgActivityLogRepository {
    fn record(&self, input: RecordActivity) -> Result<ActivityLogEntry> {
        super::block_on(self.record_async(input))
    }

    fn get(&self, id: ActivityLogId) -> Result<Option<ActivityLogEntry>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>> {
        super::block_on(self.list_async(filter))
    }

    fn history_for_subject(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<ActivityLogEntry>> {
        super::block_on(self.history_for_subject_async(subject_type, subject_id))
    }
}
