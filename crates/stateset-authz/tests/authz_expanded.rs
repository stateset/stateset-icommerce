//! Expanded authz tests covering role hierarchy, rate limiting,
//! audit log, field redaction, and custom role builder.

use std::time::Duration;

use serde_json::json;
use stateset_authz::*;

// ---------------------------------------------------------------------------
// 1. Role hierarchy: admin > operator > viewer > none
// ---------------------------------------------------------------------------

#[test]
fn role_hierarchy_admin_allows_all_actions() {
    let admin = Role::admin();
    for &action in Action::all() {
        assert!(
            admin.check("orders", &action).is_allowed(),
            "admin should allow {action} on orders"
        );
        assert!(
            admin.check("customers", &action).is_allowed(),
            "admin should allow {action} on customers"
        );
    }
}

#[test]
fn role_hierarchy_operator_allows_crud_and_delete() {
    let op = Role::operator();
    assert!(op.check("orders", &Action::Create).is_allowed());
    assert!(op.check("orders", &Action::Read).is_allowed());
    assert!(op.check("orders", &Action::Update).is_allowed());
    assert!(op.check("orders", &Action::Delete).is_allowed());
    assert!(op.check("orders", &Action::List).is_allowed());
    assert!(op.check("orders", &Action::Execute).is_allowed());
}

#[test]
fn role_hierarchy_viewer_read_only() {
    let viewer = Role::viewer();
    assert!(viewer.check("orders", &Action::Read).is_allowed());
    assert!(viewer.check("orders", &Action::List).is_allowed());
    assert!(viewer.check("orders", &Action::Create).is_denied());
    assert!(viewer.check("orders", &Action::Update).is_denied());
    assert!(viewer.check("orders", &Action::Delete).is_denied());
    assert!(viewer.check("orders", &Action::Execute).is_denied());
}

#[test]
fn role_hierarchy_none_denies_everything() {
    let none = Role::none();
    for &action in Action::all() {
        assert!(none.check("anything", &action).is_denied(), "none should deny {action}");
    }
}

#[test]
fn permission_level_ordering_is_correct() {
    assert!(PermissionLevel::None < PermissionLevel::Read);
    assert!(PermissionLevel::Read < PermissionLevel::Preview);
    assert!(PermissionLevel::Preview < PermissionLevel::Write);
    assert!(PermissionLevel::Write < PermissionLevel::Delete);
    assert!(PermissionLevel::Delete < PermissionLevel::Admin);
}

#[test]
fn permission_level_has_at_least_reflexive() {
    for &level in PermissionLevel::all() {
        assert!(level.has_at_least(level));
    }
}

#[test]
fn permission_level_has_at_least_transitive() {
    assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::None));
    assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::Read));
    assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::Write));
    assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::Delete));
    assert!(!PermissionLevel::None.has_at_least(PermissionLevel::Read));
    assert!(!PermissionLevel::Read.has_at_least(PermissionLevel::Write));
}

#[test]
fn permission_level_from_str_roundtrip() {
    for &level in PermissionLevel::all() {
        let s = level.to_string();
        let parsed: PermissionLevel = s.parse().unwrap();
        assert_eq!(parsed, level);
    }
}

#[test]
fn permission_level_from_str_case_insensitive() {
    assert_eq!("ADMIN".parse::<PermissionLevel>().unwrap(), PermissionLevel::Admin);
    assert_eq!("Read".parse::<PermissionLevel>().unwrap(), PermissionLevel::Read);
    assert_eq!("DELETE".parse::<PermissionLevel>().unwrap(), PermissionLevel::Delete);
}

// ---------------------------------------------------------------------------
// 2. Rate limit enforcement (window-based)
// ---------------------------------------------------------------------------

#[test]
fn rate_limit_allows_under_limit() {
    let mut limiter = RateLimiter::new();
    limiter.add_rule(RateLimitRule::new("orders", 5, Duration::from_secs(60)));
    for _ in 0..5 {
        assert!(limiter.check_and_record("actor-1", "orders").is_allowed());
    }
}

#[test]
fn rate_limit_blocks_at_limit() {
    let mut limiter = RateLimiter::new();
    limiter.add_rule(RateLimitRule::new("orders", 3, Duration::from_secs(60)));
    assert!(limiter.check_and_record("a", "orders").is_allowed());
    assert!(limiter.check_and_record("a", "orders").is_allowed());
    assert!(limiter.check_and_record("a", "orders").is_allowed());
    let d = limiter.check_and_record("a", "orders");
    assert!(!d.is_allowed());
}

#[test]
fn rate_limit_different_actors_independent() {
    let mut limiter = RateLimiter::new();
    limiter.add_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)));
    assert!(limiter.check_and_record("alice", "orders").is_allowed());
    // alice is now at limit
    assert!(!limiter.check_and_record("alice", "orders").is_allowed());
    // bob still has quota
    assert!(limiter.check_and_record("bob", "orders").is_allowed());
}

#[test]
fn rate_limit_different_resources_independent() {
    let mut limiter = RateLimiter::new();
    limiter.add_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)));
    limiter.add_rule(RateLimitRule::new("customers", 1, Duration::from_secs(60)));
    assert!(limiter.check_and_record("a", "orders").is_allowed());
    assert!(!limiter.check_and_record("a", "orders").is_allowed());
    // customers still has quota
    assert!(limiter.check_and_record("a", "customers").is_allowed());
}

#[test]
fn rate_limit_no_rule_means_unlimited() {
    let mut limiter = RateLimiter::new();
    let d = limiter.check("anyone", "anything");
    assert!(d.is_allowed());
    if let RateLimitDecision::Allowed { remaining } = d {
        assert_eq!(remaining, u32::MAX);
    }
}

#[test]
fn rate_limit_rule_accessors() {
    let rule = RateLimitRule::new("orders", 100, Duration::from_secs(60));
    assert_eq!(rule.resource_type(), "orders");
    assert_eq!(rule.max_requests(), 100);
    assert_eq!(rule.window(), Duration::from_secs(60));
}

#[test]
fn rate_limit_decision_display() {
    let allowed = RateLimitDecision::Allowed { remaining: 5 };
    assert!(allowed.to_string().contains("5"));
    let exceeded = RateLimitDecision::Exceeded { retry_after: Duration::from_secs(30) };
    assert!(exceeded.to_string().contains("30000"));
}

#[test]
fn rate_limit_serde_roundtrip() {
    let rule = RateLimitRule::new("orders", 100, Duration::from_secs(60));
    let json = serde_json::to_string(&rule).unwrap();
    let parsed: RateLimitRule = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, rule);
}

// ---------------------------------------------------------------------------
// 3. Audit log recording and filtering
// ---------------------------------------------------------------------------

#[test]
fn audit_log_records_and_queries() {
    let mut log = AuditLog::new(100);
    log.record(AuditRecord::new(
        "alice",
        Action::Read,
        Resource::new("orders"),
        AccessDecision::Allowed,
    ));
    log.record(AuditRecord::new(
        "bob",
        Action::Create,
        Resource::new("orders"),
        AccessDecision::denied("no"),
    ));
    log.record(AuditRecord::new(
        "alice",
        Action::Delete,
        Resource::new("customers"),
        AccessDecision::Allowed,
    ));

    assert_eq!(log.len(), 3);

    let alice = log.query(&AuditFilter::new().actor("alice"));
    assert_eq!(alice.len(), 2);

    let orders = log.query(&AuditFilter::new().resource_type("orders"));
    assert_eq!(orders.len(), 2);

    let alice_orders = log.query(&AuditFilter::new().actor("alice").resource_type("orders"));
    assert_eq!(alice_orders.len(), 1);
}

#[test]
fn audit_log_truncation() {
    let mut log = AuditLog::new(3);
    for i in 0..5 {
        log.record(AuditRecord::new(
            format!("actor-{i}"),
            Action::Read,
            Resource::new("orders"),
            AccessDecision::Allowed,
        ));
    }
    assert_eq!(log.len(), 3);
    let all = log.all();
    assert_eq!(all[0].actor_id(), "actor-2");
    assert_eq!(all[2].actor_id(), "actor-4");
}

#[test]
fn audit_log_zero_capacity_drops_all() {
    let mut log = AuditLog::new(0);
    log.record(AuditRecord::new(
        "alice",
        Action::Read,
        Resource::new("orders"),
        AccessDecision::Allowed,
    ));
    assert!(log.is_empty());
}

#[test]
fn audit_log_clear() {
    let mut log = AuditLog::new(100);
    log.record(AuditRecord::new(
        "alice",
        Action::Read,
        Resource::new("orders"),
        AccessDecision::Allowed,
    ));
    assert_eq!(log.len(), 1);
    log.clear();
    assert!(log.is_empty());
}

#[test]
fn audit_log_limit_query() {
    let mut log = AuditLog::new(100);
    for i in 0..10 {
        log.record(AuditRecord::new(
            format!("a-{i}"),
            Action::Read,
            Resource::new("orders"),
            AccessDecision::Allowed,
        ));
    }
    let limited = log.query(&AuditFilter::new().limit(3));
    assert_eq!(limited.len(), 3);
}

#[test]
fn audit_record_metadata() {
    let rec =
        AuditRecord::new("alice", Action::Read, Resource::new("orders"), AccessDecision::Allowed)
            .with_metadata("ip", "10.0.0.1")
            .with_metadata("session_id", "abc");
    assert_eq!(rec.metadata().get("ip"), Some(&"10.0.0.1".to_owned()));
    assert_eq!(rec.metadata().get("session_id"), Some(&"abc".to_owned()));
}

#[test]
fn audit_record_accessors() {
    let rec = AuditRecord::new(
        "bob",
        Action::Create,
        Resource::with_id("orders", "ord_123"),
        AccessDecision::Allowed,
    );
    assert_eq!(rec.actor_id(), "bob");
    assert_eq!(*rec.action(), Action::Create);
    assert_eq!(rec.resource().resource_type(), "orders");
    assert_eq!(rec.resource().resource_id(), Some("ord_123"));
    assert!(rec.decision().is_allowed());
}

// ---------------------------------------------------------------------------
// 4. Field redaction (sensitive fields masked)
// ---------------------------------------------------------------------------

#[test]
fn redaction_default_fields() {
    let config = RedactionConfig::default();
    assert!(config.should_redact("password"));
    assert!(config.should_redact("secret"));
    assert!(config.should_redact("token"));
    assert!(config.should_redact("api_key"));
    assert!(config.should_redact("authorization"));
    assert!(config.should_redact("credit_card"));
    assert!(config.should_redact("ssn"));
}

#[test]
fn redaction_case_insensitive() {
    let config = RedactionConfig::default();
    assert!(config.should_redact("PASSWORD"));
    assert!(config.should_redact("Token"));
    assert!(config.should_redact("API_KEY"));
    assert!(config.should_redact("Ssn"));
}

#[test]
fn redaction_does_not_match_normal_fields() {
    let config = RedactionConfig::default();
    assert!(!config.should_redact("name"));
    assert!(!config.should_redact("email"));
    assert!(!config.should_redact("order_id"));
    assert!(!config.should_redact("status"));
}

#[test]
fn redaction_custom_fields() {
    let config = RedactionConfig::with_fields(["custom_secret", "internal_key"]);
    assert!(config.should_redact("custom_secret"));
    assert!(config.should_redact("internal_key"));
    assert!(!config.should_redact("password")); // not in custom set
}

#[test]
fn redaction_pattern_matching() {
    let mut config = RedactionConfig::empty();
    config.add_pattern("key");
    assert!(config.should_redact("api_key"));
    assert!(config.should_redact("secret_key_id"));
    assert!(config.should_redact("KEY_VALUE"));
    assert!(!config.should_redact("name"));
}

#[test]
fn redact_value_flat_object() {
    let config = RedactionConfig::default();
    let mut value = json!({
        "name": "Alice",
        "password": "s3cr3t",
        "token": "abc123"
    });
    redact_value(&mut value, &config);
    assert_eq!(value["name"], "Alice");
    assert_eq!(value["password"], "[REDACTED]");
    assert_eq!(value["token"], "[REDACTED]");
}

#[test]
fn redact_value_nested_object() {
    let config = RedactionConfig::default();
    let mut value = json!({
        "user": {
            "name": "Bob",
            "auth": { "secret": "xyz", "level": "admin" }
        }
    });
    redact_value(&mut value, &config);
    assert_eq!(value["user"]["name"], "Bob");
    assert_eq!(value["user"]["auth"]["secret"], "[REDACTED]");
    assert_eq!(value["user"]["auth"]["level"], "admin");
}

#[test]
fn redact_value_array_of_objects() {
    let config = RedactionConfig::default();
    let mut value = json!([
        { "name": "A", "password": "x" },
        { "name": "B", "token": "y" }
    ]);
    redact_value(&mut value, &config);
    assert_eq!(value[0]["name"], "A");
    assert_eq!(value[0]["password"], "[REDACTED]");
    assert_eq!(value[1]["token"], "[REDACTED]");
}

#[test]
fn redact_string_normal() {
    assert_eq!(redact_string("secret123"), "sec***123");
}

#[test]
fn redact_string_short() {
    assert_eq!(redact_string("abc"), "***");
    assert_eq!(redact_string(""), "***");
}

#[test]
fn redact_string_exactly_seven() {
    assert_eq!(redact_string("1234567"), "123***567");
}

// ---------------------------------------------------------------------------
// 5. Custom role builder
// ---------------------------------------------------------------------------

#[test]
fn role_builder_default_none() {
    let role = RoleBuilder::new("custom").build();
    assert_eq!(role.name(), "custom");
    assert_eq!(role.default_level(), PermissionLevel::None);
    // Default None means all actions denied
    assert!(role.check("orders", &Action::Read).is_denied());
}

#[test]
fn role_builder_with_overrides() {
    let role = RoleBuilder::new("order-manager")
        .default_level(PermissionLevel::Read)
        .allow("orders", PermissionLevel::Admin)
        .allow("customers", PermissionLevel::Write)
        .build();

    // Orders: admin level
    assert!(role.check("orders", &Action::Delete).is_allowed());
    assert!(role.check("orders", &Action::Create).is_allowed());

    // Customers: write level
    assert!(role.check("customers", &Action::Create).is_allowed());
    assert!(role.check("customers", &Action::Update).is_allowed());
    assert!(role.check("customers", &Action::Delete).is_denied());

    // Other resources: read level
    assert!(role.check("inventory", &Action::Read).is_allowed());
    assert!(role.check("inventory", &Action::Create).is_denied());
}

#[test]
fn role_builder_multiple_resources() {
    let role = RoleBuilder::new("multi")
        .default_level(PermissionLevel::None)
        .allow("orders", PermissionLevel::Admin)
        .allow("customers", PermissionLevel::Read)
        .allow("inventory", PermissionLevel::Write)
        .allow("payments", PermissionLevel::Delete)
        .build();

    assert!(role.check("orders", &Action::Delete).is_allowed());
    assert!(role.check("customers", &Action::Read).is_allowed());
    assert!(role.check("customers", &Action::Create).is_denied());
    assert!(role.check("inventory", &Action::Update).is_allowed());
    assert!(role.check("inventory", &Action::Delete).is_denied());
    assert!(role.check("payments", &Action::Delete).is_allowed());
    assert!(role.check("unknown", &Action::Read).is_denied());
}

// ---------------------------------------------------------------------------
// 6. AuthzEngine integration
// ---------------------------------------------------------------------------

#[test]
fn engine_full_lifecycle() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .add_role(Role::none())
        .assign_role("alice", "admin")
        .assign_role("bob", "viewer")
        .assign_role("charlie", "none")
        .rate_limit_rule(RateLimitRule::new("orders", 10, Duration::from_secs(60)))
        .build();

    // Admin can do everything
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Delete).is_allowed());
    // Viewer can only read
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Create).is_denied());
    // None denied
    assert!(engine.authorize("charlie", &Resource::new("orders"), &Action::Read).is_denied());
    // Unknown actor denied
    let d = engine.authorize("ghost", &Resource::new("orders"), &Action::Read);
    assert!(d.is_denied());

    // All operations audited
    assert_eq!(engine.audit_log().len(), 5);
}

#[test]
fn engine_approval_required() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .require_approval(Some("orders".to_owned()), Some(Action::Delete))
        .build();

    let d = engine.authorize("alice", &Resource::new("orders"), &Action::Delete);
    assert!(d.requires_approval_check());
    // But read is fine
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
}

#[test]
fn engine_assign_invalid_role_error() {
    let mut engine = AuthzEngineBuilder::new().add_role(Role::admin()).build();
    let err = engine.assign_role("bob", "nonexistent");
    assert!(err.is_err());
}

#[test]
fn engine_remove_role_denies_access() {
    let mut engine =
        AuthzEngineBuilder::new().add_role(Role::admin()).assign_role("alice", "admin").build();

    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    engine.remove_role("alice");
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_denied());
}

#[test]
fn engine_redaction_integration() {
    let engine = AuthzEngineBuilder::new().build();
    let mut data = json!({
        "name": "Test",
        "password": "secret",
        "nested": { "token": "abc" }
    });
    engine.redact(&mut data);
    assert_eq!(data["name"], "Test");
    assert_eq!(data["password"], "[REDACTED]");
    assert_eq!(data["nested"]["token"], "[REDACTED]");
}

#[test]
fn engine_builder_build_checked_rejects_unknown_role() {
    let result = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "missing")
        .build_checked();
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 7. Error types
// ---------------------------------------------------------------------------

#[test]
fn error_variants_display() {
    assert!(AuthzError::unauthorized("missing token").to_string().contains("missing token"));
    assert!(AuthzError::forbidden("no access").to_string().contains("no access"));
    assert!(AuthzError::rate_limited("too many").to_string().contains("too many"));
    assert!(AuthzError::invalid_role("fake").to_string().contains("fake"));
    assert!(AuthzError::invalid_resource("nope").to_string().contains("nope"));
}

#[test]
fn error_is_helpers() {
    assert!(AuthzError::unauthorized("x").is_unauthorized());
    assert!(!AuthzError::unauthorized("x").is_forbidden());
    assert!(AuthzError::forbidden("x").is_forbidden());
    assert!(AuthzError::rate_limited("x").is_rate_limited());
}

#[test]
fn access_decision_variants() {
    let allowed = AccessDecision::Allowed;
    assert!(allowed.is_allowed());
    assert!(!allowed.is_denied());
    assert!(!allowed.requires_approval_check());
    assert!(allowed.reason().is_none());

    let denied = AccessDecision::denied("no access");
    assert!(denied.is_denied());
    assert_eq!(denied.reason(), Some("no access"));

    let approval = AccessDecision::requires_approval("high value");
    assert!(approval.requires_approval_check());
    assert_eq!(approval.reason(), Some("high value"));
}

#[test]
fn resource_display() {
    let r1 = Resource::new("orders");
    assert_eq!(r1.to_string(), "orders");
    let r2 = Resource::with_id("orders", "ord_123");
    assert_eq!(r2.to_string(), "orders:ord_123");
}

#[test]
fn action_required_permissions() {
    assert_eq!(Action::Read.required_permission(), PermissionLevel::Read);
    assert_eq!(Action::List.required_permission(), PermissionLevel::Read);
    assert_eq!(Action::Create.required_permission(), PermissionLevel::Write);
    assert_eq!(Action::Update.required_permission(), PermissionLevel::Write);
    assert_eq!(Action::Execute.required_permission(), PermissionLevel::Write);
    assert_eq!(Action::Delete.required_permission(), PermissionLevel::Delete);
}
