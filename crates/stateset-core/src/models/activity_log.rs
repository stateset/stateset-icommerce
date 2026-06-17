//! Activity log domain models
//!
//! An activity log is an append-only history of changes to a subject record —
//! a sales order, fulfillment order, shipment, or any other entity. Each entry
//! captures what changed (`action`), a human-readable `summary`, the `actor`
//! responsible, and arbitrary structured `metadata`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::ActivityLogId;
use strum::{Display, EnumString};
use uuid::Uuid;

/// The kind of actor that produced an activity log entry.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ActorKind {
    /// A human user.
    #[default]
    User,
    /// An automated system process.
    System,
    /// An external integration / API caller.
    Integration,
    /// An autonomous agent (A2A commerce).
    Agent,
}

/// A single append-only activity log entry for a subject record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogEntry {
    /// Unique entry ID.
    pub id: ActivityLogId,
    /// Subject record type (e.g. `sales_order`, `fulfillment_order`, `shipment`).
    pub subject_type: String,
    /// Subject record ID.
    pub subject_id: Uuid,
    /// Machine action key (e.g. `status_changed`, `field_edited`, `created`).
    pub action: String,
    /// Human-readable summary of the change.
    pub summary: String,
    /// What kind of actor produced this entry.
    pub actor_kind: ActorKind,
    /// Optional actor identifier (user id, integration name, agent id).
    pub actor: Option<String>,
    /// Arbitrary structured metadata (e.g. before/after values).
    pub metadata: serde_json::Value,
    /// When the entry was recorded.
    pub created_at: DateTime<Utc>,
}

impl ActivityLogEntry {
    /// A display label for the actor, falling back to the actor kind.
    #[must_use]
    pub fn actor_label(&self) -> String {
        match &self.actor {
            Some(a) if !a.is_empty() => a.clone(),
            _ => self.actor_kind.to_string(),
        }
    }
}

/// Input for recording an activity log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordActivity {
    /// Subject record type.
    pub subject_type: String,
    /// Subject record ID.
    pub subject_id: Uuid,
    /// Machine action key.
    pub action: String,
    /// Human-readable summary.
    pub summary: String,
    /// Actor kind (defaults to `System`).
    #[serde(default)]
    pub actor_kind: ActorKind,
    /// Optional actor identifier.
    pub actor: Option<String>,
    /// Structured metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Filter for listing activity log entries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActivityLogFilter {
    /// Filter by subject type.
    pub subject_type: Option<String>,
    /// Filter by subject ID.
    pub subject_id: Option<Uuid>,
    /// Filter by action key.
    pub action: Option<String>,
    /// Filter by actor kind.
    pub actor_kind: Option<ActorKind>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(actor_kind: ActorKind, actor: Option<&str>) -> ActivityLogEntry {
        ActivityLogEntry {
            id: ActivityLogId::new(),
            subject_type: "sales_order".into(),
            subject_id: Uuid::nil(),
            action: "status_changed".into(),
            summary: "Status changed from pending to shipped".into(),
            actor_kind,
            actor: actor.map(String::from),
            metadata: serde_json::json!({"from": "pending", "to": "shipped"}),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn actor_label_prefers_actor() {
        assert_eq!(make(ActorKind::User, Some("alice@x.test")).actor_label(), "alice@x.test");
    }

    #[test]
    fn actor_label_falls_back_to_kind() {
        assert_eq!(make(ActorKind::System, None).actor_label(), "system");
        assert_eq!(make(ActorKind::Agent, Some("")).actor_label(), "agent");
    }

    #[test]
    fn actor_kind_roundtrip() {
        for k in [ActorKind::User, ActorKind::System, ActorKind::Integration, ActorKind::Agent] {
            assert_eq!(k.to_string().parse::<ActorKind>().unwrap(), k);
        }
    }

    #[test]
    fn actor_kind_default_is_user() {
        assert_eq!(ActorKind::default(), ActorKind::User);
    }
}
