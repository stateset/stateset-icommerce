//! The authorization engine — combines roles, rate limiting, audit, and redaction.
//!
//! [`AuthzEngine`] is the main entry point for authorization checks. It is IO-free
//! and framework-agnostic, designed to be embedded in any runtime.

use std::collections::HashMap;

use crate::{
    AccessDecision, Action, AuditFilter, AuditLog, AuditRecord, AuthzError, AuthzResult,
    RateLimitDecision, RateLimitRule, RateLimiter, RedactionConfig, Resource, Role,
};

/// Default audit log capacity.
const DEFAULT_AUDIT_MAX_SIZE: usize = 1000;

/// The central authorization engine.
///
/// Combines role-based access control, rate limiting, audit logging, and
/// field redaction into a single, coherent API.
///
/// ```rust
/// use stateset_authz::{AuthzEngineBuilder, Role, Action, Resource};
///
/// let mut engine = AuthzEngineBuilder::new()
///     .add_role(Role::admin())
///     .add_role(Role::viewer())
///     .assign_role("alice", "admin")
///     .assign_role("bob", "viewer")
///     .build();
///
/// let decision = engine.authorize("alice", &Resource::new("orders"), &Action::Delete);
/// assert!(decision.is_allowed());
///
/// let decision = engine.authorize("bob", &Resource::new("orders"), &Action::Delete);
/// assert!(decision.is_denied());
/// ```
#[derive(Debug)]
pub struct AuthzEngine {
    roles: HashMap<String, Role>,
    actor_roles: HashMap<String, String>,
    rate_limiter: RateLimiter,
    audit_log: AuditLog,
    redaction_config: RedactionConfig,
    approval_required: Vec<ApprovalRule>,
}

/// A rule that requires explicit approval for specific operations.
#[derive(Debug, Clone)]
struct ApprovalRule {
    resource_type: Option<String>,
    action: Option<Action>,
}

impl AuthzEngine {
    /// Checks whether `actor_id` is allowed to perform `action` on `resource`.
    ///
    /// This method:
    /// 1. Looks up the actor's role
    /// 2. Checks the role's permission for the resource type
    /// 3. Checks rate limits
    /// 4. Checks approval-required rules
    /// 5. Records an audit entry
    ///
    /// ```rust
    /// use stateset_authz::{AuthzEngineBuilder, Role, Action, Resource};
    ///
    /// let mut engine = AuthzEngineBuilder::new()
    ///     .add_role(Role::viewer())
    ///     .assign_role("bob", "viewer")
    ///     .build();
    ///
    /// let d = engine.authorize("bob", &Resource::new("orders"), &Action::Read);
    /// assert!(d.is_allowed());
    /// ```
    pub fn authorize(
        &mut self,
        actor_id: &str,
        resource: &Resource,
        action: &Action,
    ) -> AccessDecision {
        // 1. Look up role
        let role = match self.resolve_role(actor_id) {
            Some(r) => r,
            None => {
                let decision =
                    AccessDecision::denied(format!("actor '{actor_id}' has no assigned role"));
                self.record_audit(actor_id, action, resource, &decision);
                return decision;
            }
        };

        // 2. Check permission level
        let decision = role.check(resource.resource_type(), action);
        if decision.is_denied() {
            self.record_audit(actor_id, action, resource, &decision);
            return decision;
        }

        // 3. Check rate limits
        let rate_decision = self.rate_limiter.check_and_record(actor_id, resource.resource_type());
        if let RateLimitDecision::Exceeded { retry_after } = rate_decision {
            let decision = AccessDecision::denied(format!(
                "rate limit exceeded for '{}' on '{}' (retry after {}ms)",
                actor_id,
                resource.resource_type(),
                retry_after.as_millis(),
            ));
            self.record_audit(actor_id, action, resource, &decision);
            return decision;
        }

        // 4. Check approval-required rules
        for rule in &self.approval_required {
            let resource_match =
                rule.resource_type.as_ref().is_none_or(|rt| rt == resource.resource_type());
            let action_match = rule.action.as_ref().is_none_or(|a| a == action);

            if resource_match && action_match {
                let decision = AccessDecision::requires_approval(format!(
                    "action '{action}' on '{}' requires explicit approval",
                    resource.resource_type(),
                ));
                self.record_audit(actor_id, action, resource, &decision);
                return decision;
            }
        }

        // 5. All checks passed
        let decision = AccessDecision::Allowed;
        self.record_audit(actor_id, action, resource, &decision);
        decision
    }

    /// Adds a new role to the engine.
    ///
    /// If a role with the same name already exists, it is replaced.
    pub fn add_role(&mut self, role: Role) {
        self.roles.insert(role.name().to_owned(), role);
    }

    /// Assigns a role to an actor.
    ///
    /// Returns an error if the role does not exist.
    pub fn assign_role(&mut self, actor_id: &str, role_name: &str) -> AuthzResult<()> {
        if !self.roles.contains_key(role_name) {
            return Err(AuthzError::invalid_role(role_name));
        }
        self.actor_roles.insert(actor_id.to_owned(), role_name.to_owned());
        Ok(())
    }

    /// Removes a role assignment for an actor.
    pub fn remove_role(&mut self, actor_id: &str) {
        self.actor_roles.remove(actor_id);
    }

    /// Returns the role assigned to an actor, if any.
    #[must_use]
    pub fn actor_role(&self, actor_id: &str) -> Option<&str> {
        self.actor_roles.get(actor_id).map(String::as_str)
    }

    /// Returns a reference to the rate limiter.
    #[must_use]
    pub const fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Returns a mutable reference to the rate limiter.
    pub const fn rate_limiter_mut(&mut self) -> &mut RateLimiter {
        &mut self.rate_limiter
    }

    /// Returns a reference to the audit log.
    #[must_use]
    pub const fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    /// Queries the audit log with the given filter.
    #[must_use]
    pub fn query_audit(&self, filter: &AuditFilter) -> Vec<&AuditRecord> {
        self.audit_log.query(filter)
    }

    /// Returns a reference to the redaction config.
    #[must_use]
    pub const fn redaction_config(&self) -> &RedactionConfig {
        &self.redaction_config
    }

    /// Redacts sensitive fields in a JSON value using the engine's redaction config.
    pub fn redact(&self, value: &mut serde_json::Value) {
        crate::redact_value(value, &self.redaction_config);
    }

    /// Adds a rate limit rule.
    pub fn add_rate_limit_rule(&mut self, rule: RateLimitRule) {
        self.rate_limiter.add_rule(rule);
    }

    /// Requires explicit approval for operations matching the criteria.
    pub fn require_approval(&mut self, resource_type: Option<String>, action: Option<Action>) {
        self.approval_required.push(ApprovalRule { resource_type, action });
    }

    // -- Private helpers --

    fn resolve_role(&self, actor_id: &str) -> Option<Role> {
        let role_name = self.actor_roles.get(actor_id)?;
        self.roles.get(role_name).cloned()
    }

    fn record_audit(
        &mut self,
        actor_id: &str,
        action: &Action,
        resource: &Resource,
        decision: &AccessDecision,
    ) {
        let record = AuditRecord::new(actor_id, *action, resource.clone(), decision.clone());
        self.audit_log.record(record);
    }
}

/// Builder for constructing an [`AuthzEngine`].
///
/// ```rust
/// use stateset_authz::{AuthzEngineBuilder, Role, RateLimitRule, RedactionConfig};
/// use std::time::Duration;
///
/// let engine = AuthzEngineBuilder::new()
///     .add_role(Role::admin())
///     .add_role(Role::viewer())
///     .assign_role("alice", "admin")
///     .assign_role("bob", "viewer")
///     .rate_limit_rule(RateLimitRule::new("orders", 100, Duration::from_secs(60)))
///     .redaction_config(RedactionConfig::default())
///     .audit_max_size(5000)
///     .build();
/// ```
#[derive(Debug)]
pub struct AuthzEngineBuilder {
    roles: HashMap<String, Role>,
    actor_roles: HashMap<String, String>,
    rate_limit_rules: Vec<RateLimitRule>,
    redaction_config: RedactionConfig,
    audit_max_size: usize,
    approval_rules: Vec<ApprovalRule>,
}

impl AuthzEngineBuilder {
    /// Creates a new builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
            actor_roles: HashMap::new(),
            rate_limit_rules: Vec::new(),
            redaction_config: RedactionConfig::default(),
            audit_max_size: DEFAULT_AUDIT_MAX_SIZE,
            approval_rules: Vec::new(),
        }
    }

    /// Adds a role to the engine.
    #[must_use]
    pub fn add_role(mut self, role: Role) -> Self {
        self.roles.insert(role.name().to_owned(), role);
        self
    }

    /// Assigns a role to an actor.
    ///
    /// [`build`](Self::build) and [`build_checked`](Self::build_checked) validate
    /// that every assigned role exists before constructing the engine.
    #[must_use]
    pub fn assign_role(
        mut self,
        actor_id: impl Into<String>,
        role_name: impl Into<String>,
    ) -> Self {
        self.actor_roles.insert(actor_id.into(), role_name.into());
        self
    }

    /// Adds a rate limit rule.
    #[must_use]
    pub fn rate_limit_rule(mut self, rule: RateLimitRule) -> Self {
        self.rate_limit_rules.push(rule);
        self
    }

    /// Sets the redaction configuration.
    #[must_use]
    pub fn redaction_config(mut self, config: RedactionConfig) -> Self {
        self.redaction_config = config;
        self
    }

    /// Sets the maximum number of audit records to retain.
    #[must_use]
    pub const fn audit_max_size(mut self, max_size: usize) -> Self {
        self.audit_max_size = max_size;
        self
    }

    /// Adds an approval requirement.
    #[must_use]
    pub fn require_approval(
        mut self,
        resource_type: Option<String>,
        action: Option<Action>,
    ) -> Self {
        self.approval_rules.push(ApprovalRule { resource_type, action });
        self
    }

    /// Builds the [`AuthzEngine`], returning an error when any assignment
    /// references a role that has not been added to the builder.
    pub fn build_checked(self) -> AuthzResult<AuthzEngine> {
        self.validate_assignments()?;
        Ok(self.build_unchecked())
    }

    /// Builds the [`AuthzEngine`].
    ///
    /// Panics when any assignment references a role that has not been added.
    /// Use [`build_checked`](Self::build_checked) to surface configuration
    /// errors without panicking.
    #[must_use]
    pub fn build(self) -> AuthzEngine {
        match self.build_checked() {
            Ok(engine) => engine,
            Err(err) => panic!("invalid AuthzEngineBuilder configuration: {err}"),
        }
    }

    fn validate_assignments(&self) -> AuthzResult<()> {
        if let Some(role_name) =
            self.actor_roles.values().find(|role_name| !self.roles.contains_key(role_name.as_str()))
        {
            return Err(AuthzError::invalid_role(role_name.clone()));
        }
        Ok(())
    }

    fn build_unchecked(self) -> AuthzEngine {
        let mut rate_limiter = RateLimiter::new();
        for rule in self.rate_limit_rules {
            rate_limiter.add_rule(rule);
        }

        AuthzEngine {
            roles: self.roles,
            actor_roles: self.actor_roles,
            rate_limiter,
            audit_log: AuditLog::new(self.audit_max_size),
            redaction_config: self.redaction_config,
            approval_required: self.approval_rules,
        }
    }
}

impl Default for AuthzEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::{PermissionLevel, RoleBuilder};
    use std::time::Duration;

    use super::*;

    fn basic_engine() -> AuthzEngine {
        AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .add_role(Role::viewer())
            .add_role(Role::none())
            .assign_role("alice", "admin")
            .assign_role("bob", "viewer")
            .assign_role("nobody", "none")
            .build()
    }

    // -- authorize --

    #[test]
    fn admin_can_do_everything() {
        let mut engine = basic_engine();
        for &action in Action::all() {
            let d = engine.authorize("alice", &Resource::new("orders"), &action);
            assert!(d.is_allowed(), "admin should allow {action}");
        }
    }

    #[test]
    fn viewer_can_only_read() {
        let mut engine = basic_engine();
        assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Read).is_allowed());
        assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Create).is_denied());
    }

    #[test]
    fn none_role_denied() {
        let mut engine = basic_engine();
        assert!(engine.authorize("nobody", &Resource::new("orders"), &Action::Read).is_denied());
    }

    #[test]
    fn unknown_actor_denied() {
        let mut engine = basic_engine();
        let d = engine.authorize("ghost", &Resource::new("orders"), &Action::Read);
        assert!(d.is_denied());
        assert!(d.reason().unwrap().contains("no assigned role"));
    }

    // -- Rate limiting --

    #[test]
    fn rate_limit_blocks_after_max() {
        let mut engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("alice", "admin")
            .rate_limit_rule(RateLimitRule::new("orders", 2, Duration::from_secs(60)))
            .build();

        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
        let d = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
        assert!(d.is_denied());
        assert!(d.reason().unwrap().contains("rate limit"));
    }

    // -- Approval required --

    #[test]
    fn approval_required_triggers() {
        let mut engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("alice", "admin")
            .require_approval(Some("orders".to_owned()), Some(Action::Delete))
            .build();

        // Delete requires approval even for admin
        let d = engine.authorize("alice", &Resource::new("orders"), &Action::Delete);
        assert!(d.requires_approval_check());
        assert!(d.reason().unwrap().contains("requires explicit approval"));

        // But read is fine
        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    }

    #[test]
    fn approval_required_wildcard_action() {
        let mut engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("alice", "admin")
            .require_approval(Some("payments".to_owned()), None)
            .build();

        // Any action on payments requires approval
        let d = engine.authorize("alice", &Resource::new("payments"), &Action::Read);
        assert!(d.requires_approval_check());
    }

    // -- Audit logging --

    #[test]
    fn authorize_records_audit() {
        let mut engine = basic_engine();
        engine.authorize("alice", &Resource::new("orders"), &Action::Read);

        assert_eq!(engine.audit_log().len(), 1);
        let records = engine.query_audit(&AuditFilter::new().actor("alice"));
        assert_eq!(records.len(), 1);
        assert!(records[0].decision().is_allowed());
    }

    #[test]
    fn denied_operations_also_audited() {
        let mut engine = basic_engine();
        engine.authorize("bob", &Resource::new("orders"), &Action::Delete);

        let records = engine.query_audit(&AuditFilter::new().actor("bob"));
        assert_eq!(records.len(), 1);
        assert!(records[0].decision().is_denied());
    }

    // -- Role management --

    #[test]
    fn add_role_at_runtime() {
        let mut engine = basic_engine();
        let custom = RoleBuilder::new("custom").default_level(PermissionLevel::Write).build();

        engine.add_role(custom);
        engine.assign_role("charlie", "custom").unwrap();

        assert!(
            engine.authorize("charlie", &Resource::new("orders"), &Action::Create).is_allowed()
        );
    }

    #[test]
    fn assign_invalid_role_returns_error() {
        let mut engine = basic_engine();
        let err = engine.assign_role("charlie", "nonexistent").unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
    }

    #[test]
    fn remove_role_denies_access() {
        let mut engine = basic_engine();
        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());

        engine.remove_role("alice");

        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_denied());
    }

    #[test]
    fn actor_role_accessor() {
        let engine = basic_engine();
        assert_eq!(engine.actor_role("alice"), Some("admin"));
        assert_eq!(engine.actor_role("ghost"), None);
    }

    // -- Redaction --

    #[test]
    fn engine_redact_uses_config() {
        let engine = AuthzEngineBuilder::new().build();
        let mut value = serde_json::json!({
            "name": "Test",
            "password": "secret"
        });

        engine.redact(&mut value);
        assert_eq!(value["password"], "[REDACTED]");
        assert_eq!(value["name"], "Test");
    }

    // -- Builder --

    #[test]
    fn builder_default() {
        let builder = AuthzEngineBuilder::default();
        let engine = builder.build();
        assert_eq!(engine.audit_log().max_size(), DEFAULT_AUDIT_MAX_SIZE);
    }

    #[test]
    fn builder_custom_audit_size() {
        let engine = AuthzEngineBuilder::new().audit_max_size(50).build();
        assert_eq!(engine.audit_log().max_size(), 50);
    }

    #[test]
    fn builder_custom_redaction() {
        let config = RedactionConfig::with_fields(["custom_field"]);
        let engine = AuthzEngineBuilder::new().redaction_config(config).build();

        assert!(engine.redaction_config().should_redact("custom_field"));
        assert!(!engine.redaction_config().should_redact("password"));
    }

    #[test]
    fn builder_build_checked_rejects_unknown_role_assignment() {
        let err = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("alice", "missing")
            .build_checked()
            .unwrap_err();

        assert_eq!(err, AuthzError::invalid_role("missing"));
    }

    // -- Full flow integration --

    #[test]
    fn full_flow_assign_authorize_audit() {
        let mut engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .add_role(Role::viewer())
            .assign_role("alice", "admin")
            .assign_role("bob", "viewer")
            .rate_limit_rule(RateLimitRule::new("orders", 10, Duration::from_secs(60)))
            .build();

        // Alice can create orders
        let d = engine.authorize("alice", &Resource::new("orders"), &Action::Create);
        assert!(d.is_allowed());

        // Bob cannot create orders
        let d = engine.authorize("bob", &Resource::new("orders"), &Action::Create);
        assert!(d.is_denied());

        // Both operations were audited
        assert_eq!(engine.audit_log().len(), 2);

        let alice_records = engine.query_audit(&AuditFilter::new().actor("alice"));
        assert_eq!(alice_records.len(), 1);
        assert!(alice_records[0].decision().is_allowed());

        let bob_records = engine.query_audit(&AuditFilter::new().actor("bob"));
        assert_eq!(bob_records.len(), 1);
        assert!(bob_records[0].decision().is_denied());
    }

    #[test]
    fn rate_limiting_integration() {
        let mut engine = AuthzEngineBuilder::new()
            .add_role(Role::admin())
            .assign_role("alice", "admin")
            .rate_limit_rule(RateLimitRule::new("orders", 3, Duration::from_secs(60)))
            .build();

        for _ in 0..3 {
            assert!(
                engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed()
            );
        }

        let d = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
        assert!(d.is_denied());

        // All 4 operations (3 allowed + 1 denied) are audited
        assert_eq!(engine.audit_log().len(), 4);
    }

    #[test]
    fn rate_limiter_accessors() {
        let mut engine = AuthzEngineBuilder::new()
            .rate_limit_rule(RateLimitRule::new("orders", 5, Duration::from_secs(60)))
            .build();

        assert_eq!(engine.rate_limiter().rule_count(), 1);

        // Add another via mutable accessor
        engine.rate_limiter_mut().add_rule(RateLimitRule::new(
            "customers",
            10,
            Duration::from_secs(60),
        ));
        assert_eq!(engine.rate_limiter().rule_count(), 2);
    }

    #[test]
    fn require_approval_at_runtime() {
        let mut engine =
            AuthzEngineBuilder::new().add_role(Role::admin()).assign_role("alice", "admin").build();

        // Initially allowed
        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Delete).is_allowed());

        // Add approval requirement
        engine.require_approval(Some("orders".to_owned()), Some(Action::Delete));

        // Now requires approval
        let d = engine.authorize("alice", &Resource::new("orders"), &Action::Delete);
        assert!(d.requires_approval_check());
    }
}
