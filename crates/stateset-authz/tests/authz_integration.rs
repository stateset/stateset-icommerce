//! Integration tests for the `stateset-authz` crate.
//!
//! These tests exercise the public API of the authorization engine end-to-end,
//! verifying role hierarchy, permission inheritance, rate limiting, audit
//! logging, field redaction, and the builder pattern.

use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use stateset_authz::{
    Action, AuditFilter, AuthzEngineBuilder, PermissionLevel, RateLimitRule, RedactionConfig,
    Resource, Role, RoleBuilder, redact_string, redact_value,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Builds an engine with all four built-in roles and three assigned actors.
fn standard_engine() -> stateset_authz::AuthzEngine {
    AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::operator())
        .add_role(Role::viewer())
        .add_role(Role::none())
        .assign_role("alice", "admin")
        .assign_role("bob", "viewer")
        .assign_role("carol", "operator")
        .assign_role("dan", "none")
        .build()
}

// ===========================================================================
// 1. Role Hierarchy and Permission Inheritance
// ===========================================================================

#[test]
fn admin_can_perform_all_actions() {
    let mut engine = standard_engine();
    for &action in Action::all() {
        let d = engine.authorize("alice", &Resource::new("orders"), &action);
        assert!(d.is_allowed(), "admin should be allowed to {action} on orders");
    }
}

#[test]
fn viewer_can_only_read_and_list() {
    let mut engine = standard_engine();

    // Read and List require PermissionLevel::Read — viewer should pass.
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::List).is_allowed());

    // Write-level actions must be denied.
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Create).is_denied());
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Update).is_denied());
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Execute).is_denied());

    // Delete-level action must be denied.
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Delete).is_denied());
}

#[test]
fn operator_can_create_read_update_and_delete() {
    let mut engine = standard_engine();

    // Operator has PermissionLevel::Delete, which covers Read, Write, and Delete.
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::Create).is_allowed());
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::Update).is_allowed());
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::Delete).is_allowed());
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::List).is_allowed());
    assert!(engine.authorize("carol", &Resource::new("orders"), &Action::Execute).is_allowed());
}

#[test]
fn no_role_assigned_denies_everything() {
    let mut engine = standard_engine();

    // "ghost" has no role assignment at all.
    for &action in Action::all() {
        let d = engine.authorize("ghost", &Resource::new("orders"), &action);
        assert!(d.is_denied(), "unassigned actor should be denied for {action}");
        assert!(
            d.reason().unwrap().contains("no assigned role"),
            "denial reason should mention missing role assignment"
        );
    }
}

#[test]
fn none_role_denies_everything() {
    let mut engine = standard_engine();

    for &action in Action::all() {
        let d = engine.authorize("dan", &Resource::new("orders"), &action);
        assert!(d.is_denied(), "'none' role should deny {action}");
    }
}

#[test]
fn custom_role_with_per_resource_overrides() {
    let custom = RoleBuilder::new("order-manager")
        .default_level(PermissionLevel::Read)
        .allow("orders", PermissionLevel::Admin)
        .allow("customers", PermissionLevel::Write)
        .allow("inventory", PermissionLevel::None)
        .build();

    let mut engine =
        AuthzEngineBuilder::new().add_role(custom).assign_role("emma", "order-manager").build();

    // Admin-level on orders — everything allowed.
    assert!(engine.authorize("emma", &Resource::new("orders"), &Action::Delete).is_allowed());

    // Write-level on customers — Create/Update allowed, Delete denied.
    assert!(engine.authorize("emma", &Resource::new("customers"), &Action::Create).is_allowed());
    assert!(engine.authorize("emma", &Resource::new("customers"), &Action::Delete).is_denied());

    // None-level on inventory — everything denied.
    assert!(engine.authorize("emma", &Resource::new("inventory"), &Action::Read).is_denied());

    // Default Read-level on unlisted resources.
    assert!(engine.authorize("emma", &Resource::new("shipments"), &Action::Read).is_allowed());
    assert!(engine.authorize("emma", &Resource::new("shipments"), &Action::Create).is_denied());
}

// ===========================================================================
// 2. Permission Level Ordering
// ===========================================================================

#[test]
fn write_level_allows_create_and_update_but_not_delete() {
    let role = RoleBuilder::new("writer").default_level(PermissionLevel::Write).build();

    assert!(role.check("orders", &Action::Create).is_allowed());
    assert!(role.check("orders", &Action::Update).is_allowed());
    assert!(role.check("orders", &Action::Read).is_allowed());
    assert!(role.check("orders", &Action::List).is_allowed());
    assert!(role.check("orders", &Action::Execute).is_allowed());
    // Delete requires PermissionLevel::Delete, which is above Write.
    assert!(role.check("orders", &Action::Delete).is_denied());
}

#[test]
fn delete_level_allows_everything_write_does_plus_delete() {
    let role = RoleBuilder::new("deleter").default_level(PermissionLevel::Delete).build();

    assert!(role.check("orders", &Action::Create).is_allowed());
    assert!(role.check("orders", &Action::Update).is_allowed());
    assert!(role.check("orders", &Action::Delete).is_allowed());
    assert!(role.check("orders", &Action::Read).is_allowed());
}

#[test]
fn admin_level_allows_everything() {
    let role = RoleBuilder::new("full-admin").default_level(PermissionLevel::Admin).build();

    for &action in Action::all() {
        assert!(
            role.check("any_resource", &action).is_allowed(),
            "admin level should allow {action}"
        );
    }
}

#[test]
fn read_level_does_not_allow_create_update_delete() {
    let role = RoleBuilder::new("reader").default_level(PermissionLevel::Read).build();

    assert!(role.check("orders", &Action::Read).is_allowed());
    assert!(role.check("orders", &Action::List).is_allowed());
    assert!(role.check("orders", &Action::Create).is_denied());
    assert!(role.check("orders", &Action::Update).is_denied());
    assert!(role.check("orders", &Action::Delete).is_denied());
}

#[test]
fn permission_level_ordering_is_strict() {
    // None < Read < Preview < Write < Delete < Admin
    assert!(PermissionLevel::None < PermissionLevel::Read);
    assert!(PermissionLevel::Read < PermissionLevel::Preview);
    assert!(PermissionLevel::Preview < PermissionLevel::Write);
    assert!(PermissionLevel::Write < PermissionLevel::Delete);
    assert!(PermissionLevel::Delete < PermissionLevel::Admin);
}

// ===========================================================================
// 3. Rate Limit Enforcement
// ===========================================================================

#[test]
fn under_limit_returns_allowed_with_remaining() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .rate_limit_rule(RateLimitRule::new("orders", 5, Duration::from_secs(60)))
        .build();

    let d = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    assert!(d.is_allowed());

    // The rate limiter internally tracks remaining; we verify allowed status.
    let d2 = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    assert!(d2.is_allowed());
}

#[test]
fn at_limit_returns_denied_with_rate_limit_reason() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .rate_limit_rule(RateLimitRule::new("orders", 2, Duration::from_secs(60)))
        .build();

    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());

    let d = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    assert!(d.is_denied());
    assert!(d.reason().unwrap().contains("rate limit"), "denial should mention rate limit");
}

#[test]
fn different_resources_have_independent_rate_limits() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .rate_limit_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)))
        .rate_limit_rule(RateLimitRule::new("customers", 1, Duration::from_secs(60)))
        .build();

    // Exhaust orders limit.
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_denied());

    // Customers should still be available.
    assert!(engine.authorize("alice", &Resource::new("customers"), &Action::Read).is_allowed());
}

#[test]
fn different_actors_tracked_independently() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .assign_role("bob", "admin")
        .rate_limit_rule(RateLimitRule::new("orders", 1, Duration::from_secs(60)))
        .build();

    // Alice exhausts her limit.
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_denied());

    // Bob is independent and still has quota.
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Read).is_allowed());
}

#[test]
fn no_rate_limit_rule_means_unlimited() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        // No rate limit rules added.
        .build();

    for _ in 0..100 {
        assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());
    }
}

// ===========================================================================
// 4. Audit Trail
// ===========================================================================

#[test]
fn authorize_creates_audit_record() {
    let mut engine = standard_engine();
    engine.authorize("alice", &Resource::new("orders"), &Action::Read);

    assert_eq!(engine.audit_log().len(), 1);
}

#[test]
fn audit_record_contains_expected_fields() {
    let mut engine = standard_engine();
    engine.authorize("alice", &Resource::new("orders"), &Action::Create);

    let records = engine.query_audit(&AuditFilter::new());
    assert_eq!(records.len(), 1);

    let rec = records[0];
    assert_eq!(rec.actor_id(), "alice");
    assert_eq!(*rec.action(), Action::Create);
    assert_eq!(rec.resource().resource_type(), "orders");
    assert!(rec.decision().is_allowed());
    // Timestamp should be recent (within the last second).
    let elapsed = Utc::now() - rec.timestamp();
    assert!(elapsed.num_seconds() < 2);
}

#[test]
fn filter_audit_by_actor_id() {
    let mut engine = standard_engine();
    engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    engine.authorize("bob", &Resource::new("orders"), &Action::Read);
    engine.authorize("alice", &Resource::new("customers"), &Action::Read);

    let alice_records = engine.query_audit(&AuditFilter::new().actor("alice"));
    assert_eq!(alice_records.len(), 2);

    let bob_records = engine.query_audit(&AuditFilter::new().actor("bob"));
    assert_eq!(bob_records.len(), 1);
}

#[test]
fn filter_audit_by_resource_type() {
    let mut engine = standard_engine();
    engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    engine.authorize("alice", &Resource::new("customers"), &Action::Read);
    engine.authorize("alice", &Resource::new("orders"), &Action::Create);

    let orders = engine.query_audit(&AuditFilter::new().resource_type("orders"));
    assert_eq!(orders.len(), 2);

    let customers = engine.query_audit(&AuditFilter::new().resource_type("customers"));
    assert_eq!(customers.len(), 1);
}

#[test]
fn filter_audit_by_time_range() {
    let mut engine =
        AuthzEngineBuilder::new().add_role(Role::admin()).assign_role("alice", "admin").build();

    // Perform several operations; all timestamps will be "now".
    engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    engine.authorize("alice", &Resource::new("orders"), &Action::Create);

    let now = Utc::now();
    let an_hour_ago = now - chrono::Duration::hours(1);

    // All records should be within the last hour.
    let recent = engine.query_audit(&AuditFilter::new().since(an_hour_ago));
    assert_eq!(recent.len(), 2);

    // Nothing should exist before an hour ago.
    let old = engine.query_audit(&AuditFilter::new().until(an_hour_ago));
    assert_eq!(old.len(), 0);
}

#[test]
fn audit_log_ordering_is_insertion_order() {
    let mut engine = standard_engine();
    engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    engine.authorize("bob", &Resource::new("customers"), &Action::Read);
    engine.authorize("carol", &Resource::new("inventory"), &Action::Create);

    let all = engine.audit_log().all();
    assert_eq!(all.len(), 3);
    // Insertion order is preserved: alice, bob, carol.
    assert_eq!(all[0].actor_id(), "alice");
    assert_eq!(all[1].actor_id(), "bob");
    assert_eq!(all[2].actor_id(), "carol");
}

#[test]
fn denied_operations_are_also_audited() {
    let mut engine = standard_engine();
    let d = engine.authorize("bob", &Resource::new("orders"), &Action::Delete);
    assert!(d.is_denied());

    let records = engine.query_audit(&AuditFilter::new().actor("bob"));
    assert_eq!(records.len(), 1);
    assert!(records[0].decision().is_denied());
}

#[test]
fn audit_log_truncates_when_full() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .audit_max_size(3)
        .build();

    for _ in 0..5 {
        engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    }

    // Only the most recent 3 entries should remain.
    assert_eq!(engine.audit_log().len(), 3);
}

// ===========================================================================
// 5. Field Redaction
// ===========================================================================

#[test]
fn default_sensitive_fields_are_redacted() {
    let config = RedactionConfig::default();
    let mut data = json!({
        "name": "Alice",
        "password": "s3cr3t",
        "token": "abc-def-ghi",
        "secret": "my_secret_value",
        "credit_card": "4111-1111-1111-1111"
    });

    redact_value(&mut data, &config);

    assert_eq!(data["name"], "Alice");
    assert_eq!(data["password"], "[REDACTED]");
    assert_eq!(data["token"], "[REDACTED]");
    assert_eq!(data["secret"], "[REDACTED]");
    assert_eq!(data["credit_card"], "[REDACTED]");
}

#[test]
fn partial_masking_with_redact_string() {
    // Strings longer than 6 chars: keep first 3 and last 3, replace middle with ***.
    assert_eq!(redact_string("secret123"), "sec***123");
    assert_eq!(redact_string("abcdefghij"), "abc***hij");

    // Strings of 6 or fewer chars: entirely replaced.
    assert_eq!(redact_string("short"), "***");
    assert_eq!(redact_string("ab"), "***");
    assert_eq!(redact_string(""), "***");
}

#[test]
fn nested_json_objects_redacted_recursively() {
    let config = RedactionConfig::default();
    let mut data = json!({
        "user": {
            "name": "Bob",
            "auth": {
                "token": "bearer_xyz",
                "provider": "oauth"
            }
        },
        "metadata": {
            "api_key": "key_123"
        }
    });

    redact_value(&mut data, &config);

    assert_eq!(data["user"]["name"], "Bob");
    assert_eq!(data["user"]["auth"]["token"], "[REDACTED]");
    assert_eq!(data["user"]["auth"]["provider"], "oauth");
    assert_eq!(data["metadata"]["api_key"], "[REDACTED]");
}

#[test]
fn non_sensitive_fields_left_untouched() {
    let config = RedactionConfig::default();
    let mut data = json!({
        "order_id": "ord_123",
        "customer_name": "Charlie",
        "total_amount": 99.95,
        "items": ["widget", "gadget"],
        "active": true
    });

    let original = data.clone();
    redact_value(&mut data, &config);

    assert_eq!(data, original);
}

#[test]
fn custom_redaction_config() {
    let config = RedactionConfig::with_fields(["social_security", "driver_license"]);
    let mut data = json!({
        "name": "Dave",
        "social_security": "123-45-6789",
        "driver_license": "DL-999",
        "password": "should_not_be_redacted_with_custom_config"
    });

    redact_value(&mut data, &config);

    assert_eq!(data["name"], "Dave");
    assert_eq!(data["social_security"], "[REDACTED]");
    assert_eq!(data["driver_license"], "[REDACTED]");
    // "password" is not in the custom config, so it stays.
    assert_eq!(data["password"], "should_not_be_redacted_with_custom_config");
}

#[test]
fn redaction_via_engine_uses_configured_config() {
    let config = RedactionConfig::with_fields(["custom_secret"]);
    let engine = AuthzEngineBuilder::new().redaction_config(config).build();

    let mut data = json!({
        "custom_secret": "hidden",
        "password": "visible_here"
    });

    engine.redact(&mut data);

    assert_eq!(data["custom_secret"], "[REDACTED]");
    assert_eq!(data["password"], "visible_here");
}

#[test]
fn redaction_with_pattern_matching() {
    let mut config = RedactionConfig::empty();
    config.add_pattern("key");

    let mut data = json!({
        "api_key": "k1",
        "secret_key_value": "k2",
        "name": "ok"
    });

    redact_value(&mut data, &config);

    assert_eq!(data["api_key"], "[REDACTED]");
    assert_eq!(data["secret_key_value"], "[REDACTED]");
    assert_eq!(data["name"], "ok");
}

#[test]
fn redaction_handles_arrays_of_objects() {
    let config = RedactionConfig::default();
    let mut data = json!([
        { "name": "Alice", "token": "t1" },
        { "name": "Bob", "token": "t2" }
    ]);

    redact_value(&mut data, &config);

    assert_eq!(data[0]["name"], "Alice");
    assert_eq!(data[0]["token"], "[REDACTED]");
    assert_eq!(data[1]["name"], "Bob");
    assert_eq!(data[1]["token"], "[REDACTED]");
}

// ===========================================================================
// 6. Builder Pattern
// ===========================================================================

#[test]
fn builder_fluent_api_constructs_working_engine() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .assign_role("alice", "admin")
        .assign_role("bob", "viewer")
        .rate_limit_rule(RateLimitRule::new("orders", 100, Duration::from_secs(60)))
        .redaction_config(RedactionConfig::default())
        .audit_max_size(500)
        .build();

    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Delete).is_allowed());
    assert!(engine.authorize("bob", &Resource::new("orders"), &Action::Delete).is_denied());
    assert_eq!(engine.audit_log().max_size(), 500);
}

#[test]
fn builder_multiple_roles_registered() {
    let role_a = RoleBuilder::new("role-a").default_level(PermissionLevel::Read).build();
    let role_b = RoleBuilder::new("role-b").default_level(PermissionLevel::Write).build();
    let role_c = RoleBuilder::new("role-c").default_level(PermissionLevel::Admin).build();

    let mut engine = AuthzEngineBuilder::new()
        .add_role(role_a)
        .add_role(role_b)
        .add_role(role_c)
        .assign_role("actor-a", "role-a")
        .assign_role("actor-b", "role-b")
        .assign_role("actor-c", "role-c")
        .build();

    // actor-a (Read) cannot create.
    assert!(engine.authorize("actor-a", &Resource::new("orders"), &Action::Create).is_denied());
    // actor-b (Write) can create but not delete.
    assert!(engine.authorize("actor-b", &Resource::new("orders"), &Action::Create).is_allowed());
    assert!(engine.authorize("actor-b", &Resource::new("orders"), &Action::Delete).is_denied());
    // actor-c (Admin) can do everything.
    assert!(engine.authorize("actor-c", &Resource::new("orders"), &Action::Delete).is_allowed());
}

#[test]
fn builder_multiple_actors_on_same_role() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::viewer())
        .assign_role("user1", "viewer")
        .assign_role("user2", "viewer")
        .assign_role("user3", "viewer")
        .build();

    for user in &["user1", "user2", "user3"] {
        assert!(engine.authorize(user, &Resource::new("orders"), &Action::Read).is_allowed());
        assert!(engine.authorize(user, &Resource::new("orders"), &Action::Create).is_denied());
    }
}

#[test]
fn invalid_role_assignment_at_runtime_returns_error() {
    let mut engine = AuthzEngineBuilder::new().add_role(Role::admin()).build();

    let result = engine.assign_role("alice", "nonexistent");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("nonexistent"), "error should mention the invalid role name");
}

#[test]
fn builder_default_creates_empty_engine() {
    let engine = AuthzEngineBuilder::default().build();
    assert!(engine.audit_log().is_empty());
    assert_eq!(engine.rate_limiter().rule_count(), 0);
}

#[test]
fn builder_with_approval_rules() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .assign_role("alice", "admin")
        .require_approval(Some("payments".to_owned()), Some(Action::Delete))
        .build();

    // Delete on payments requires approval even for admin.
    let d = engine.authorize("alice", &Resource::new("payments"), &Action::Delete);
    assert!(d.requires_approval_check());

    // Read on payments is fine.
    assert!(engine.authorize("alice", &Resource::new("payments"), &Action::Read).is_allowed());

    // Delete on orders (not in the approval rule) is also fine.
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Delete).is_allowed());
}

// ===========================================================================
// Bonus: Cross-cutting integration scenarios
// ===========================================================================

#[test]
fn full_lifecycle_authorize_audit_redact() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .assign_role("alice", "admin")
        .assign_role("bob", "viewer")
        .rate_limit_rule(RateLimitRule::new("orders", 10, Duration::from_secs(60)))
        .build();

    // Alice creates an order — allowed.
    let d1 = engine.authorize("alice", &Resource::new("orders"), &Action::Create);
    assert!(d1.is_allowed());

    // Bob tries to delete — denied.
    let d2 = engine.authorize("bob", &Resource::new("orders"), &Action::Delete);
    assert!(d2.is_denied());

    // Both decisions are audited.
    assert_eq!(engine.audit_log().len(), 2);

    let alice_audit = engine.query_audit(&AuditFilter::new().actor("alice"));
    assert_eq!(alice_audit.len(), 1);
    assert!(alice_audit[0].decision().is_allowed());

    let bob_audit = engine.query_audit(&AuditFilter::new().actor("bob"));
    assert_eq!(bob_audit.len(), 1);
    assert!(bob_audit[0].decision().is_denied());

    // Redaction works on the same engine.
    let mut payload = json!({ "order_id": "ord_1", "api_key": "secret_key_value" });
    engine.redact(&mut payload);
    assert_eq!(payload["order_id"], "ord_1");
    assert_eq!(payload["api_key"], "[REDACTED]");
}

#[test]
fn role_reassignment_changes_permissions() {
    let mut engine = AuthzEngineBuilder::new()
        .add_role(Role::admin())
        .add_role(Role::viewer())
        .assign_role("alice", "viewer")
        .build();

    // Initially viewer — cannot create.
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Create).is_denied());

    // Reassign to admin.
    engine.assign_role("alice", "admin").unwrap();

    // Now she can create.
    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Create).is_allowed());
}

#[test]
fn remove_role_denies_all_subsequent_access() {
    let mut engine =
        AuthzEngineBuilder::new().add_role(Role::admin()).assign_role("alice", "admin").build();

    assert!(engine.authorize("alice", &Resource::new("orders"), &Action::Read).is_allowed());

    engine.remove_role("alice");

    let d = engine.authorize("alice", &Resource::new("orders"), &Action::Read);
    assert!(d.is_denied());
    assert!(d.reason().unwrap().contains("no assigned role"));
}
