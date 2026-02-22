//! Role-based access control.
//!
//! A [`Role`] maps resource types to [`PermissionLevel`]s. Built-in roles
//! cover common patterns; use [`RoleBuilder`] for custom configurations.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{AccessDecision, Action, PermissionLevel};

/// A named role with per-resource permission levels and a default fallback.
///
/// ```rust
/// use stateset_authz::{Role, Action, PermissionLevel, AccessDecision};
///
/// let viewer = Role::viewer();
/// assert_eq!(viewer.name(), "viewer");
///
/// // Viewer can read orders
/// let decision = viewer.check("orders", &Action::Read);
/// assert!(decision.is_allowed());
///
/// // Viewer cannot create orders
/// let decision = viewer.check("orders", &Action::Create);
/// assert!(decision.is_denied());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    name: String,
    permissions: HashMap<String, PermissionLevel>,
    default_level: PermissionLevel,
}

impl Role {
    /// Creates a new role with explicit fields.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        permissions: HashMap<String, PermissionLevel>,
        default_level: PermissionLevel,
    ) -> Self {
        Self {
            name: name.into(),
            permissions,
            default_level,
        }
    }

    /// Returns the role name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the permission map.
    #[must_use]
    pub const fn permissions(&self) -> &HashMap<String, PermissionLevel> {
        &self.permissions
    }

    /// Returns the default permission level for unlisted resource types.
    #[must_use]
    pub const fn default_level(&self) -> PermissionLevel {
        self.default_level
    }

    /// Looks up the effective permission level for a given resource type.
    #[must_use]
    pub fn effective_level(&self, resource_type: &str) -> PermissionLevel {
        self.permissions
            .get(resource_type)
            .copied()
            .unwrap_or(self.default_level)
    }

    /// Checks whether this role allows the given action on the given resource type.
    ///
    /// ```rust
    /// use stateset_authz::{Role, Action};
    ///
    /// let admin = Role::admin();
    /// assert!(admin.check("anything", &Action::Delete).is_allowed());
    /// ```
    #[must_use]
    pub fn check(&self, resource_type: &str, action: &Action) -> AccessDecision {
        let effective = self.effective_level(resource_type);
        let required = action.required_permission();

        if effective.has_at_least(required) {
            AccessDecision::Allowed
        } else {
            AccessDecision::denied(format!(
                "role '{}' has '{effective}' permission on '{resource_type}', \
                 but '{action}' requires '{required}'",
                self.name,
            ))
        }
    }

    // -- Built-in roles --

    /// Full access to everything.
    ///
    /// ```rust
    /// use stateset_authz::{Role, Action};
    ///
    /// let admin = Role::admin();
    /// assert!(admin.check("orders", &Action::Delete).is_allowed());
    /// ```
    #[must_use]
    pub fn admin() -> Self {
        Self {
            name: "admin".to_owned(),
            permissions: HashMap::new(),
            default_level: PermissionLevel::Admin,
        }
    }

    /// Read + write + delete, but not admin bulk operations.
    ///
    /// ```rust
    /// use stateset_authz::{Role, Action, PermissionLevel};
    ///
    /// let op = Role::operator();
    /// assert!(op.check("orders", &Action::Delete).is_allowed());
    /// assert_eq!(op.default_level(), PermissionLevel::Delete);
    /// ```
    #[must_use]
    pub fn operator() -> Self {
        Self {
            name: "operator".to_owned(),
            permissions: HashMap::new(),
            default_level: PermissionLevel::Delete,
        }
    }

    /// Read-only access.
    ///
    /// ```rust
    /// use stateset_authz::{Role, Action};
    ///
    /// let viewer = Role::viewer();
    /// assert!(viewer.check("orders", &Action::Read).is_allowed());
    /// assert!(viewer.check("orders", &Action::Create).is_denied());
    /// ```
    #[must_use]
    pub fn viewer() -> Self {
        Self {
            name: "viewer".to_owned(),
            permissions: HashMap::new(),
            default_level: PermissionLevel::Read,
        }
    }

    /// No access at all.
    ///
    /// ```rust
    /// use stateset_authz::{Role, Action};
    ///
    /// let none = Role::none();
    /// assert!(none.check("orders", &Action::Read).is_denied());
    /// ```
    #[must_use]
    pub fn none() -> Self {
        Self {
            name: "none".to_owned(),
            permissions: HashMap::new(),
            default_level: PermissionLevel::None,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(default={})", self.name, self.default_level)
    }
}

/// Builder for constructing custom [`Role`] instances.
///
/// ```rust
/// use stateset_authz::{RoleBuilder, PermissionLevel, Action};
///
/// let role = RoleBuilder::new("order-manager")
///     .default_level(PermissionLevel::Read)
///     .allow("orders", PermissionLevel::Admin)
///     .allow("customers", PermissionLevel::Write)
///     .build();
///
/// assert!(role.check("orders", &Action::Delete).is_allowed());
/// assert!(role.check("customers", &Action::Create).is_allowed());
/// assert!(role.check("inventory", &Action::Create).is_denied());
/// ```
#[derive(Debug)]
pub struct RoleBuilder {
    name: String,
    permissions: HashMap<String, PermissionLevel>,
    default_level: PermissionLevel,
}

impl RoleBuilder {
    /// Creates a builder with the given role name and `None` as the default level.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            permissions: HashMap::new(),
            default_level: PermissionLevel::None,
        }
    }

    /// Sets the default permission level for unlisted resource types.
    #[must_use]
    pub const fn default_level(mut self, level: PermissionLevel) -> Self {
        self.default_level = level;
        self
    }

    /// Grants a specific permission level for a resource type.
    #[must_use]
    pub fn allow(mut self, resource_type: impl Into<String>, level: PermissionLevel) -> Self {
        self.permissions.insert(resource_type.into(), level);
        self
    }

    /// Builds the [`Role`].
    #[must_use]
    pub fn build(self) -> Role {
        Role {
            name: self.name,
            permissions: self.permissions,
            default_level: self.default_level,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Built-in roles --

    #[test]
    fn admin_allows_everything() {
        let admin = Role::admin();
        for &action in Action::all() {
            let decision = admin.check("anything", &action);
            assert!(decision.is_allowed(), "admin should allow {action}");
        }
    }

    #[test]
    fn operator_allows_up_to_delete() {
        let op = Role::operator();
        assert!(op.check("orders", &Action::Read).is_allowed());
        assert!(op.check("orders", &Action::Create).is_allowed());
        assert!(op.check("orders", &Action::Delete).is_allowed());
        // Operator cannot do admin-level operations... but actually Delete is the
        // highest required_permission for any Action, so operator can do everything
        // that Action maps to. The gap is only when resource-specific overrides
        // restrict below Delete.
    }

    #[test]
    fn viewer_read_only() {
        let viewer = Role::viewer();
        assert!(viewer.check("orders", &Action::Read).is_allowed());
        assert!(viewer.check("orders", &Action::List).is_allowed());
        assert!(viewer.check("orders", &Action::Create).is_denied());
        assert!(viewer.check("orders", &Action::Update).is_denied());
        assert!(viewer.check("orders", &Action::Delete).is_denied());
        assert!(viewer.check("orders", &Action::Execute).is_denied());
    }

    #[test]
    fn none_denies_everything() {
        let none = Role::none();
        for &action in Action::all() {
            let decision = none.check("anything", &action);
            assert!(decision.is_denied(), "none should deny {action}");
        }
    }

    // -- Custom roles --

    #[test]
    fn custom_role_with_overrides() {
        let role = RoleBuilder::new("custom")
            .default_level(PermissionLevel::Read)
            .allow("orders", PermissionLevel::Write)
            .build();

        assert!(role.check("orders", &Action::Create).is_allowed());
        assert!(role.check("orders", &Action::Delete).is_denied());
        assert!(role.check("customers", &Action::Read).is_allowed());
        assert!(role.check("customers", &Action::Create).is_denied());
    }

    #[test]
    fn role_builder_multiple_resources() {
        let role = RoleBuilder::new("multi")
            .default_level(PermissionLevel::None)
            .allow("orders", PermissionLevel::Admin)
            .allow("customers", PermissionLevel::Read)
            .allow("inventory", PermissionLevel::Write)
            .build();

        assert!(role.check("orders", &Action::Delete).is_allowed());
        assert!(role.check("customers", &Action::Read).is_allowed());
        assert!(role.check("customers", &Action::Create).is_denied());
        assert!(role.check("inventory", &Action::Update).is_allowed());
        assert!(role.check("inventory", &Action::Delete).is_denied());
        assert!(role.check("unknown", &Action::Read).is_denied());
    }

    // -- effective_level --

    #[test]
    fn effective_level_uses_specific_over_default() {
        let role = RoleBuilder::new("test")
            .default_level(PermissionLevel::Read)
            .allow("orders", PermissionLevel::Admin)
            .build();

        assert_eq!(role.effective_level("orders"), PermissionLevel::Admin);
        assert_eq!(role.effective_level("customers"), PermissionLevel::Read);
    }

    // -- Display --

    #[test]
    fn role_display() {
        let admin = Role::admin();
        assert_eq!(admin.to_string(), "admin(default=admin)");
    }

    // -- Serde --

    #[test]
    fn role_serde_roundtrip() {
        let role = RoleBuilder::new("test-role")
            .default_level(PermissionLevel::Read)
            .allow("orders", PermissionLevel::Write)
            .build();

        let json = serde_json::to_string(&role).unwrap();
        let parsed: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, role);
    }

    // -- check denial reason --

    #[test]
    fn check_denial_includes_details() {
        let viewer = Role::viewer();
        let decision = viewer.check("orders", &Action::Create);
        if let AccessDecision::Denied { reason } = decision {
            assert!(reason.contains("viewer"), "should mention role name");
            assert!(reason.contains("orders"), "should mention resource");
            assert!(reason.contains("create"), "should mention action");
        } else {
            panic!("expected denied");
        }
    }

    // -- name accessor --

    #[test]
    fn role_name_accessor() {
        assert_eq!(Role::admin().name(), "admin");
        assert_eq!(Role::operator().name(), "operator");
        assert_eq!(Role::viewer().name(), "viewer");
        assert_eq!(Role::none().name(), "none");
    }
}
