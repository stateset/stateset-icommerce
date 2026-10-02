//! Activity logs  (append-only subject history).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Activity logs  (append-only subject history)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RecordActivityInput {
    /// Subject record type (e.g. "sales_order")
    pub subject_type: String,
    pub subject_id: String,
    /// Machine action key (e.g. "status_changed")
    pub action: String,
    pub summary: String,
    /// user, system, integration, agent
    #[napi(ts_type = "ActorKind")]
    pub actor_kind: Option<String>,
    pub actor: Option<String>,
    /// Metadata as JSON
    pub metadata: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityLogFilterInput {
    pub subject_type: Option<String>,
    pub subject_id: Option<String>,
    pub action: Option<String>,
    /// user, system, integration, agent
    #[napi(ts_type = "ActorKind")]
    pub actor_kind: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActivityLogEntryOutput {
    pub id: String,
    pub subject_type: String,
    pub subject_id: String,
    pub action: String,
    pub summary: String,
    /// user, system, integration, agent
    #[napi(ts_type = "ActorKind")]
    pub actor_kind: String,
    pub actor: Option<String>,
    /// Metadata as JSON
    pub metadata: String,
    pub created_at: String,
}

impl From<stateset_core::ActivityLogEntry> for ActivityLogEntryOutput {
    fn from(e: stateset_core::ActivityLogEntry) -> Self {
        Self {
            id: e.id.to_string(),
            subject_type: e.subject_type,
            subject_id: e.subject_id.to_string(),
            action: e.action,
            summary: e.summary,
            actor_kind: e.actor_kind.to_string(),
            actor: e.actor,
            metadata: e.metadata.to_string(),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_actor_kind(s: &str) -> Result<stateset_core::ActorKind> {
    s.parse::<stateset_core::ActorKind>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid actor kind: {s}")))
}

pub(crate) fn parse_metadata_json(s: Option<String>) -> Result<serde_json::Value> {
    match s {
        Some(s) => serde_json::from_str(&s)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid metadata JSON", e)),
        None => Ok(serde_json::Value::Null),
    }
}

#[napi]
pub struct ActivityLogs {
    pub(crate) commerce: Handle,
}

#[napi]
impl ActivityLogs {
    /// Whether the activity-logs backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.activity_logs().is_supported())
    }

    /// Record an activity log entry.
    #[napi]
    pub async fn record(&self, input: RecordActivityInput) -> Result<ActivityLogEntryOutput> {
        let commerce = self.commerce.get()?;
        let actor_kind = match input.actor_kind.as_deref() {
            Some(s) => parse_actor_kind(s)?,
            None => stateset_core::ActorKind::default(),
        };
        let entry = commerce
            .activity_logs()
            .record(stateset_core::RecordActivity {
                subject_type: input.subject_type,
                subject_id: parse_uuid_str(&input.subject_id, "subject_id")?,
                action: input.action,
                summary: input.summary,
                actor_kind,
                actor: input.actor,
                metadata: parse_metadata_json(input.metadata)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to record activity", e))?;
        Ok(entry.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ActivityLogEntryOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "activity_log")?;
        let entry = commerce
            .activity_logs()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get activity log entry", e))?;
        Ok(entry.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<ActivityLogFilterInput>,
    ) -> Result<Vec<ActivityLogEntryOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::ActivityLogFilter::default()),
            |f| -> Result<stateset_core::ActivityLogFilter> {
                Ok(stateset_core::ActivityLogFilter {
                    subject_type: f.subject_type,
                    subject_id: parse_optional_uuid(f.subject_id, "subject_id")?,
                    action: f.action,
                    actor_kind: f.actor_kind.as_deref().map(parse_actor_kind).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let entries = commerce
            .activity_logs()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list activity logs", e))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }

    /// Full history for a single subject, most recent first.
    #[napi]
    pub async fn history_for_subject(
        &self,
        subject_type: String,
        subject_id: String,
    ) -> Result<Vec<ActivityLogEntryOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&subject_id, "subject_id")?;
        let entries = commerce
            .activity_logs()
            .history_for_subject(&subject_type, uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get activity history", e))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }
}
