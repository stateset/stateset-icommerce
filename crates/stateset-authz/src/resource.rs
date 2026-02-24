//! Resources and actions for authorization checks.
//!
//! A [`Resource`] identifies *what* is being accessed, while an [`Action`]
//! describes the operation being performed. Each action maps to a minimum
//! [`PermissionLevel`] via [`Action::required_permission`].

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::PermissionLevel;

/// A resource to be protected by authorization.
///
/// ```rust
/// use stateset_authz::Resource;
///
/// let orders = Resource::new("orders");
/// assert_eq!(orders.resource_type(), "orders");
/// assert!(orders.resource_id().is_none());
///
/// let order = Resource::with_id("orders", "ord_123");
/// assert_eq!(order.resource_id(), Some("ord_123"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resource {
    resource_type: String,
    resource_id: Option<String>,
}

impl Resource {
    /// Creates a resource with only a type (e.g., "orders").
    #[must_use]
    pub fn new(resource_type: impl Into<String>) -> Self {
        Self { resource_type: resource_type.into(), resource_id: None }
    }

    /// Creates a resource with a type and a specific ID.
    #[must_use]
    pub fn with_id(resource_type: impl Into<String>, resource_id: impl Into<String>) -> Self {
        Self { resource_type: resource_type.into(), resource_id: Some(resource_id.into()) }
    }

    /// Returns the resource type (e.g., "orders", "customers").
    #[must_use]
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    /// Returns the optional resource ID.
    #[must_use]
    pub fn resource_id(&self) -> Option<&str> {
        self.resource_id.as_deref()
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.resource_id {
            Some(id) => write!(f, "{}:{}", self.resource_type, id),
            None => f.write_str(&self.resource_type),
        }
    }
}

/// An action to be performed on a resource.
///
/// Each action maps to a minimum required [`PermissionLevel`] via
/// [`Action::required_permission`].
///
/// ```rust
/// use stateset_authz::{Action, PermissionLevel};
///
/// assert_eq!(Action::Read.required_permission(), PermissionLevel::Read);
/// assert_eq!(Action::Create.required_permission(), PermissionLevel::Write);
/// assert_eq!(Action::Delete.required_permission(), PermissionLevel::Delete);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Action {
    /// Create a new resource.
    Create,
    /// Read / retrieve a resource.
    Read,
    /// Update an existing resource.
    Update,
    /// Delete a resource.
    Delete,
    /// List / query multiple resources.
    List,
    /// Execute a tool or command.
    Execute,
}

impl Action {
    /// Returns the minimum [`PermissionLevel`] required for this action.
    ///
    /// ```rust
    /// use stateset_authz::{Action, PermissionLevel};
    ///
    /// assert_eq!(Action::List.required_permission(), PermissionLevel::Read);
    /// assert_eq!(Action::Execute.required_permission(), PermissionLevel::Write);
    /// ```
    #[must_use]
    pub const fn required_permission(self) -> PermissionLevel {
        match self {
            Self::Read | Self::List => PermissionLevel::Read,
            Self::Create | Self::Update | Self::Execute => PermissionLevel::Write,
            Self::Delete => PermissionLevel::Delete,
        }
    }

    /// Returns all action variants.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Create, Self::Read, Self::Update, Self::Delete, Self::List, Self::Execute]
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Create => "create",
            Self::Read => "read",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::List => "list",
            Self::Execute => "execute",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Resource --

    #[test]
    fn resource_new_has_no_id() {
        let r = Resource::new("orders");
        assert_eq!(r.resource_type(), "orders");
        assert!(r.resource_id().is_none());
    }

    #[test]
    fn resource_with_id() {
        let r = Resource::with_id("orders", "ord_123");
        assert_eq!(r.resource_type(), "orders");
        assert_eq!(r.resource_id(), Some("ord_123"));
    }

    #[test]
    fn resource_display_no_id() {
        let r = Resource::new("customers");
        assert_eq!(r.to_string(), "customers");
    }

    #[test]
    fn resource_display_with_id() {
        let r = Resource::with_id("customers", "cust_42");
        assert_eq!(r.to_string(), "customers:cust_42");
    }

    #[test]
    fn resource_equality() {
        let a = Resource::with_id("orders", "1");
        let b = Resource::with_id("orders", "1");
        let c = Resource::with_id("orders", "2");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn resource_serde_roundtrip() {
        let r = Resource::with_id("products", "prod_99");
        let json = serde_json::to_string(&r).unwrap();
        let parsed: Resource = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, r);
    }

    // -- Action --

    #[test]
    fn action_required_permission_read_ops() {
        assert_eq!(Action::Read.required_permission(), PermissionLevel::Read);
        assert_eq!(Action::List.required_permission(), PermissionLevel::Read);
    }

    #[test]
    fn action_required_permission_write_ops() {
        assert_eq!(Action::Create.required_permission(), PermissionLevel::Write);
        assert_eq!(Action::Update.required_permission(), PermissionLevel::Write);
        assert_eq!(Action::Execute.required_permission(), PermissionLevel::Write);
    }

    #[test]
    fn action_required_permission_delete() {
        assert_eq!(Action::Delete.required_permission(), PermissionLevel::Delete);
    }

    #[test]
    fn action_display() {
        assert_eq!(Action::Create.to_string(), "create");
        assert_eq!(Action::Read.to_string(), "read");
        assert_eq!(Action::Update.to_string(), "update");
        assert_eq!(Action::Delete.to_string(), "delete");
        assert_eq!(Action::List.to_string(), "list");
        assert_eq!(Action::Execute.to_string(), "execute");
    }

    #[test]
    fn action_all_returns_six() {
        assert_eq!(Action::all().len(), 6);
    }

    #[test]
    fn action_serde_roundtrip() {
        for &action in Action::all() {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: Action = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, action);
        }
    }
}
