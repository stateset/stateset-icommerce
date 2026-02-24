//! In-memory audit log with filtering and auto-truncation.
//!
//! Records every authorization decision for traceability.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AccessDecision, Action, Resource};

/// A single audit record capturing an authorization decision.
///
/// ```rust
/// use stateset_authz::{AuditRecord, Action, Resource, AccessDecision};
///
/// let record = AuditRecord::new(
///     "actor-1",
///     Action::Read,
///     Resource::new("orders"),
///     AccessDecision::Allowed,
/// );
/// assert_eq!(record.actor_id(), "actor-1");
/// assert!(record.decision().is_allowed());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    id: Uuid,
    timestamp: DateTime<Utc>,
    actor_id: String,
    action: Action,
    resource: Resource,
    decision: AccessDecision,
    metadata: HashMap<String, String>,
}

impl AuditRecord {
    /// Creates a new audit record with the current timestamp.
    #[must_use]
    pub fn new(
        actor_id: impl Into<String>,
        action: Action,
        resource: Resource,
        decision: AccessDecision,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            actor_id: actor_id.into(),
            action,
            resource,
            decision,
            metadata: HashMap::new(),
        }
    }

    /// Creates a record with a specific timestamp (useful for testing).
    #[must_use]
    pub fn with_timestamp(
        actor_id: impl Into<String>,
        action: Action,
        resource: Resource,
        decision: AccessDecision,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp,
            actor_id: actor_id.into(),
            action,
            resource,
            decision,
            metadata: HashMap::new(),
        }
    }

    /// Adds metadata to this record.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns the record ID.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Returns the actor ID.
    #[must_use]
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    /// Returns the action.
    #[must_use]
    pub const fn action(&self) -> &Action {
        &self.action
    }

    /// Returns the resource.
    #[must_use]
    pub const fn resource(&self) -> &Resource {
        &self.resource
    }

    /// Returns the access decision.
    #[must_use]
    pub const fn decision(&self) -> &AccessDecision {
        &self.decision
    }

    /// Returns the metadata.
    #[must_use]
    pub const fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

/// Filter criteria for querying the audit log.
///
/// All fields are optional; unset fields match everything.
///
/// ```rust
/// use stateset_authz::AuditFilter;
/// use chrono::Utc;
///
/// let filter = AuditFilter::new()
///     .actor("alice")
///     .resource_type("orders")
///     .limit(50);
/// ```
#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    actor_id: Option<String>,
    resource_type: Option<String>,
    since: Option<DateTime<Utc>>,
    until: Option<DateTime<Utc>>,
    limit: Option<usize>,
}

impl AuditFilter {
    /// Creates an empty filter that matches everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by actor ID.
    #[must_use]
    pub fn actor(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = Some(actor_id.into());
        self
    }

    /// Filters by resource type.
    #[must_use]
    pub fn resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = Some(resource_type.into());
        self
    }

    /// Filters records after this timestamp (inclusive).
    #[must_use]
    pub const fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = Some(since);
        self
    }

    /// Filters records before this timestamp (inclusive).
    #[must_use]
    pub const fn until(mut self, until: DateTime<Utc>) -> Self {
        self.until = Some(until);
        self
    }

    /// Limits the number of results.
    #[must_use]
    pub const fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    fn matches(&self, record: &AuditRecord) -> bool {
        if let Some(ref actor) = self.actor_id {
            if record.actor_id != *actor {
                return false;
            }
        }
        if let Some(ref rt) = self.resource_type {
            if record.resource.resource_type() != rt.as_str() {
                return false;
            }
        }
        if let Some(since) = self.since {
            if record.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if record.timestamp > until {
                return false;
            }
        }
        true
    }
}

/// An in-memory audit log with configurable maximum size and auto-truncation.
///
/// ```rust
/// use stateset_authz::{AuditLog, AuditRecord, AuditFilter, Action, Resource, AccessDecision};
///
/// let mut log = AuditLog::new(1000);
/// log.record(AuditRecord::new(
///     "alice",
///     Action::Read,
///     Resource::new("orders"),
///     AccessDecision::Allowed,
/// ));
///
/// assert_eq!(log.len(), 1);
///
/// let results = log.query(&AuditFilter::new().actor("alice"));
/// assert_eq!(results.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct AuditLog {
    records: Vec<AuditRecord>,
    max_size: usize,
}

impl AuditLog {
    /// Creates a new audit log with the given maximum size.
    ///
    /// When the log exceeds `max_size`, the oldest records are discarded.
    #[must_use]
    pub const fn new(max_size: usize) -> Self {
        Self { records: Vec::new(), max_size }
    }

    /// Appends a record. If the log is full, the oldest record is removed.
    pub fn record(&mut self, record: AuditRecord) {
        if self.records.len() >= self.max_size && self.max_size > 0 {
            self.records.remove(0);
        }
        self.records.push(record);
    }

    /// Queries the log with the given filter.
    #[must_use]
    pub fn query(&self, filter: &AuditFilter) -> Vec<&AuditRecord> {
        let iter = self.records.iter().filter(|r| filter.matches(r));
        match filter.limit {
            Some(limit) => iter.take(limit).collect(),
            None => iter.collect(),
        }
    }

    /// Returns the total number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns `true` if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the maximum capacity.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    /// Clears all records.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    /// Returns all records as a slice.
    #[must_use]
    pub fn all(&self) -> &[AuditRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    fn make_record(actor: &str, resource: &str, decision: AccessDecision) -> AuditRecord {
        AuditRecord::new(actor, Action::Read, Resource::new(resource), decision)
    }

    // -- AuditRecord --

    #[test]
    fn record_new_has_uuid() {
        let r = make_record("alice", "orders", AccessDecision::Allowed);
        // UUID should not be nil
        assert_ne!(r.id(), Uuid::nil());
    }

    #[test]
    fn record_accessors() {
        let r = make_record("bob", "customers", AccessDecision::Allowed);
        assert_eq!(r.actor_id(), "bob");
        assert_eq!(*r.action(), Action::Read);
        assert_eq!(r.resource().resource_type(), "customers");
        assert!(r.decision().is_allowed());
        assert!(r.metadata().is_empty());
    }

    #[test]
    fn record_with_metadata() {
        let r = make_record("alice", "orders", AccessDecision::Allowed)
            .with_metadata("ip", "10.0.0.1")
            .with_metadata("session", "abc123");

        assert_eq!(r.metadata().get("ip"), Some(&"10.0.0.1".to_owned()));
        assert_eq!(r.metadata().get("session"), Some(&"abc123".to_owned()));
    }

    #[test]
    fn record_serde_roundtrip() {
        let r =
            make_record("alice", "orders", AccessDecision::Allowed).with_metadata("key", "value");

        let json = serde_json::to_string(&r).unwrap();
        let parsed: AuditRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.actor_id(), r.actor_id());
        assert_eq!(parsed.decision(), r.decision());
    }

    // -- AuditLog --

    #[test]
    fn empty_log() {
        let log = AuditLog::new(100);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.max_size(), 100);
    }

    #[test]
    fn record_and_query() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        log.record(make_record("bob", "orders", AccessDecision::denied("no")));

        assert_eq!(log.len(), 2);

        let all = log.query(&AuditFilter::new());
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn query_by_actor() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        log.record(make_record("bob", "orders", AccessDecision::Allowed));
        log.record(make_record("alice", "customers", AccessDecision::Allowed));

        let alice = log.query(&AuditFilter::new().actor("alice"));
        assert_eq!(alice.len(), 2);

        let bob = log.query(&AuditFilter::new().actor("bob"));
        assert_eq!(bob.len(), 1);
    }

    #[test]
    fn query_by_resource_type() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        log.record(make_record("alice", "customers", AccessDecision::Allowed));
        log.record(make_record("bob", "orders", AccessDecision::Allowed));

        let orders = log.query(&AuditFilter::new().resource_type("orders"));
        assert_eq!(orders.len(), 2);
    }

    #[test]
    fn query_by_time_range() {
        let base = Utc::now();
        let mut log = AuditLog::new(100);

        log.record(AuditRecord::with_timestamp(
            "alice",
            Action::Read,
            Resource::new("orders"),
            AccessDecision::Allowed,
            base - Duration::hours(2),
        ));
        log.record(AuditRecord::with_timestamp(
            "alice",
            Action::Read,
            Resource::new("orders"),
            AccessDecision::Allowed,
            base - Duration::minutes(30),
        ));
        log.record(AuditRecord::with_timestamp(
            "alice",
            Action::Read,
            Resource::new("orders"),
            AccessDecision::Allowed,
            base,
        ));

        let recent = log.query(&AuditFilter::new().since(base - Duration::hours(1)));
        assert_eq!(recent.len(), 2);

        let old = log.query(&AuditFilter::new().until(base - Duration::hours(1)));
        assert_eq!(old.len(), 1);
    }

    #[test]
    fn query_with_limit() {
        let mut log = AuditLog::new(100);
        for i in 0..10 {
            log.record(make_record(&format!("actor-{i}"), "orders", AccessDecision::Allowed));
        }

        let limited = log.query(&AuditFilter::new().limit(3));
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn query_combined_filters() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        log.record(make_record("alice", "customers", AccessDecision::Allowed));
        log.record(make_record("bob", "orders", AccessDecision::Allowed));

        let results = log.query(&AuditFilter::new().actor("alice").resource_type("orders"));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn truncation_when_full() {
        let mut log = AuditLog::new(3);
        for i in 0..5 {
            log.record(make_record(&format!("actor-{i}"), "orders", AccessDecision::Allowed));
        }

        assert_eq!(log.len(), 3);
        // Should have actors 2, 3, 4 (oldest removed)
        let all = log.all();
        assert_eq!(all[0].actor_id(), "actor-2");
        assert_eq!(all[1].actor_id(), "actor-3");
        assert_eq!(all[2].actor_id(), "actor-4");
    }

    #[test]
    fn clear() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        assert_eq!(log.len(), 1);
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn all_returns_slice() {
        let mut log = AuditLog::new(100);
        log.record(make_record("alice", "orders", AccessDecision::Allowed));
        let all = log.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn record_with_timestamp() {
        let ts = Utc::now();
        let r = AuditRecord::with_timestamp(
            "alice",
            Action::Create,
            Resource::with_id("orders", "ord_1"),
            AccessDecision::Allowed,
            ts,
        );
        assert_eq!(r.timestamp(), ts);
    }
}
