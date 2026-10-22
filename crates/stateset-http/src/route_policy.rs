//! Route authorization policy — the single, reviewed classification of every
//! HTTP route the server mounts.
//!
//! Every `(method, path template)` pair registered by [`crate::routes`] has
//! exactly one [`RoutePolicy`] row in [`ROUTE_POLICIES`]. The row carries:
//!
//! - the [`ActionClass`] the role-based [`stateset_authz::AuthzEngine`] checks
//!   (an explicit table replaces the old verb-guessing classifier, so
//!   money-moving verbs such as `void`, `reverse`, `write-off`, `charge` and
//!   `settle` are [`ActionClass::Destructive`] rather than falling through to
//!   a plain write), and
//! - the [`Access`] class that decides what a **customer-scoped principal**
//!   may do: nothing on operator routes, only its own records on owned
//!   routes, and catalog reads on public routes.
//!
//! The table is enforced at request time by `require_authorization`
//! (role check) and `ownership::enforce_ownership` (object-level check), both
//! keyed by axum's [`axum::extract::MatchedPath`]. A matched route with no row
//! fails closed. The `route_policy` tests fail when a route in
//! `src/routes/*.rs` has no row, when a row names a route that does not
//! exist, and when `docs/src/http-authz.md` drifts from the table.

use stateset_authz::Action;

/// The operation class a route performs, as seen by the role engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionClass {
    /// Read one record.
    Read,
    /// List or query records.
    List,
    /// Create a record.
    Create,
    /// Update a record in place.
    Update,
    /// Delete, cancel, disable or revoke.
    Delete,
    /// A state transition or command that is neither destructive nor a delete.
    Execute,
    /// Irreversible or money-moving: void, reverse, write-off, charge, settle,
    /// refund, dispose, scrap, unapply, period close/lock/reopen, payment-run
    /// processing. Requires the `Delete` permission level (operator or above)
    /// and is always refused to customer-scoped principals.
    Destructive,
}

impl ActionClass {
    /// The authz-engine action this class is checked as.
    pub(crate) const fn to_authz(self) -> Action {
        match self {
            Self::Read => Action::Read,
            Self::List => Action::List,
            Self::Create => Action::Create,
            Self::Update => Action::Update,
            Self::Delete => Action::Delete,
            Self::Execute => Action::Execute,
            Self::Destructive => Action::Destructive,
        }
    }

    #[cfg(test)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::List => "list",
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Execute => "execute",
            Self::Destructive => "destructive",
        }
    }
}

/// A record family whose owner is resolvable to a single customer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OwnerKind {
    /// The customer record itself (a customer owns itself).
    Customer,
    /// `orders.customer_id`.
    Order,
    /// `carts.customer_id` (guest carts have no owner).
    Cart,
    /// `payments.customer_id`, else the payment's order's customer.
    Payment,
    /// `returns.customer_id`.
    Return,
    /// The shipment's order's customer.
    Shipment,
    /// `subscriptions.customer_id`.
    Subscription,
    /// `reviews.customer_id`.
    Review,
    /// `wishlists.customer_id`.
    Wishlist,
    /// `store_credits.customer_id`.
    StoreCredit,
    /// `invoices.customer_id`.
    Invoice,
    /// `warranties.customer_id`.
    Warranty,
    /// `loyalty_accounts.customer_id`.
    LoyaltyAccount,
}

impl OwnerKind {
    /// The noun handlers use in their 404 message, so a denied lookup is
    /// indistinguishable from a missing record.
    pub(crate) const fn noun(self) -> &'static str {
        match self {
            Self::Customer => "Customer",
            Self::Order => "Order",
            Self::Cart => "Cart",
            Self::Payment => "Payment",
            Self::Return => "Return",
            Self::Shipment => "Shipment",
            Self::Subscription => "Subscription",
            Self::Review => "Review",
            Self::Wishlist => "Wishlist",
            Self::StoreCredit => "Store credit",
            Self::Invoice => "Invoice",
            Self::Warranty => "Warranty",
            Self::LoyaltyAccount => "Loyalty account",
        }
    }
}

/// How a create request's body names its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyOwner {
    /// The body's customer field (named here) must be the principal's
    /// customer; when absent it is set to the principal's customer.
    Customer(&'static str),
    /// The body references an existing record (`field`) that the principal
    /// must own; an optional customer field is matched/forced as above.
    Reference { field: &'static str, kind: OwnerKind, customer_field: Option<&'static str> },
}

/// What a customer-scoped principal may do on a route. Operator principals
/// are governed by the role engine alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    /// Outside `/api/v1` (health, metrics, version). Not subject to the
    /// ownership layer.
    Infra,
    /// Catalog or reference data with no per-customer ownership.
    Public,
    /// Staff-only: customer-scoped principals are refused with 403.
    Operator,
    /// The path parameter `param` names a record of `kind` the principal must
    /// own; anything else is reported as 404.
    Owned { kind: OwnerKind, param: &'static str },
    /// A list endpoint whose `filter` query parameter is forced to the
    /// principal's customer before the handler (and its pagination) runs.
    OwnedList { filter: &'static str },
    /// A create endpoint whose body ownership is checked or forced.
    OwnedCreate { body: BodyOwner },
}

impl Access {
    #[cfg(test)]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Infra => "infra",
            Self::Public => "public",
            Self::Operator => "operator",
            Self::Owned { .. } => "owned",
            Self::OwnedList { .. } => "owned-list",
            Self::OwnedCreate { .. } => "owned-create",
        }
    }
}

/// One classified route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoutePolicy {
    /// Upper-case HTTP method.
    pub(crate) method: &'static str,
    /// The axum route template, e.g. `/api/v1/orders/{id}`.
    pub(crate) path: &'static str,
    pub(crate) action: ActionClass,
    pub(crate) access: Access,
}

impl RoutePolicy {
    /// The authz resource type: the first path segment under `/api/v1`
    /// (`orders`, `gl`, `fixed-assets`, ...), as the role engine has always
    /// keyed it.
    pub(crate) fn resource_type(&self) -> &'static str {
        let rest = self.path.strip_prefix("/api/v1/").unwrap_or(self.path);
        let first = rest.split('/').find(|segment| !segment.is_empty()).unwrap_or(rest);
        if first == "openapi.json" { "openapi" } else { first }
    }

    /// Value of the template parameter `name` within the concrete `path`.
    pub(crate) fn param<'p>(&self, path: &'p str, name: &str) -> Option<&'p str> {
        let wanted = format!("{{{name}}}");
        self.path
            .split('/')
            .zip(path.split('/'))
            .find(|(template, _)| *template == wanted)
            .map(|(_, value)| value)
            .filter(|value| !value.is_empty())
    }

    /// Value of the first template parameter within the concrete `path`.
    pub(crate) fn first_param<'p>(&self, path: &'p str) -> Option<&'p str> {
        self.path
            .split('/')
            .zip(path.split('/'))
            .find(|(template, _)| template.starts_with('{'))
            .map(|(_, value)| value)
            .filter(|value| !value.is_empty())
    }
}

/// Look up the policy row for a matched route template.
pub(crate) fn policy_for(method: &str, template: &str) -> Option<&'static RoutePolicy> {
    ROUTE_POLICIES.iter().find(|policy| policy.method == method && policy.path == template)
}

/// Every route the server mounts, classified. Keep sorted by module; the
/// `route_policy` tests enforce completeness against `src/routes/*.rs`.
#[rustfmt::skip]
pub(crate) static ROUTE_POLICIES: &[RoutePolicy] = &[
    RoutePolicy { method: "POST", path: "/api/v1/a2a/credit", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/a2a/credit", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/a2a/credit/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/a2a/credit/{id}/charge", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/a2a/credit/{id}/payment", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/a2a/credit/{id}/entries", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/a2a/messages", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/a2a/messages", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/a2a/messages/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/a2a/messages/{id}/acknowledge", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/a2a/messages/{id}/fail", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/bills", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ap/bills", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ap/bills/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/bills/{id}/approve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/bills/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/bills/{id}/dispute", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ap/bills/{id}/three-way-match", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payments", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payments/{id}/void", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payment-runs", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payment-runs/{id}/approve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payment-runs/{id}/process", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ap/payment-runs/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ap/aging", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/aging", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/aging/customers", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/aging/customers/{customer_id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/payment-applications", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/payment-applications/{id}/unapply", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/credit-memos", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/credit-memos", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/credit-memos/{id}/apply", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/write-offs", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/write-offs/{id}/reverse", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/collection-activities", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/collection-activities", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/ar/invoices/{invoice_id}/dunning", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/dunning/due", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/ar/customers/{customer_id}/statement", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/activity-logs", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/activity-logs", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/activity-logs/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/activity-logs/{subject_type}/{subject_id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/backorders", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/backorders", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/backorders/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/backorders/{id}/fulfill", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/backorders/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/boms", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/boms", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/boms/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/boms/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/boms/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/boms/{id}/activate", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/boms/{id}/components", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/boms/{id}/components", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/carts", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/carts", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/carts/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/carts/{id}/items", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "PUT", path: "/api/v1/carts/{id}/items/{item_id}", action: ActionClass::Update, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "DELETE", path: "/api/v1/carts/{id}/items/{item_id}", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/carts/{id}/shipping", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/carts/{id}/payment", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/carts/{id}/complete", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/carts/{id}/cancel", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Cart, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/channels", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/channels", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/channels/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/channels/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/channels/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/channels/{id}/lock", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/companies", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/companies", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/companies/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/companies/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/companies/{id}/contacts", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/companies/{id}/contacts", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/currencies/rates", action: ActionClass::List, access: Access::Public },
    RoutePolicy { method: "POST", path: "/api/v1/currencies/rates", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/currencies/convert", action: ActionClass::Execute, access: Access::Public },
    RoutePolicy { method: "POST", path: "/api/v1/customers", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/customers", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/customers/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Customer, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/customers/{id}", action: ActionClass::Update, access: Access::Owned { kind: OwnerKind::Customer, param: "id" } },
    RoutePolicy { method: "DELETE", path: "/api/v1/customers/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/edi-documents", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/edi-documents", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/edi-documents/summary", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/edi-documents/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/edi-documents/{id}/status", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/events/stream", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fixed-assets", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fixed-assets/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/fixed-assets/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets/{id}/place-in-service", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets/{id}/dispose", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets/{id}/write-off", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets/{id}/schedule", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fixed-assets/{id}/schedule", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fixed-assets/{id}/post-depreciation", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/waves", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/waves", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/waves/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/waves/{id}/release", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/picks", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/picks/{id}/assign", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/picks/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/packs", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/packs/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/packs/{id}/cartons", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/packs/{id}/cartons", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/fulfillment/ships", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/fulfillment/ships/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/accounts", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/accounts", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/accounts/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/journal-entries", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/journal-entries", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/journal-entries/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/journal-entries/{id}/post", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/journal-entries/{id}/void", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/journal-entries/{id}/reverse", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/revalue", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/close-month", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/trial-balance", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/balance-sheet", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/income-statement", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/periods", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gl/periods", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/periods/{id}/open", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/periods/{id}/close", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/periods/{id}/lock", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gl/periods/{id}/reopen", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gift-cards", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gift-cards", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/gift-cards/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gift-cards/{id}/charge", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gift-cards/{id}/refund", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/gift-cards/{id}/disable", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/health", action: ActionClass::Read, access: Access::Infra },
    RoutePolicy { method: "GET", path: "/health/ready", action: ActionClass::Read, access: Access::Infra },
    RoutePolicy { method: "GET", path: "/health/deep", action: ActionClass::Read, access: Access::Infra },
    RoutePolicy { method: "GET", path: "/metrics", action: ActionClass::Read, access: Access::Infra },
    RoutePolicy { method: "GET", path: "/version", action: ActionClass::Read, access: Access::Infra },
    RoutePolicy { method: "POST", path: "/api/v1/inbound-shipments", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inbound-shipments", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inbound-shipments/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inbound-shipments/{id}/in-transit", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inbound-shipments/{id}/arrived", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inbound-shipments/{id}/receive", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inbound-shipments/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/integration-field-mappings", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-field-mappings", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/integration-field-mappings/bulk", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/integration-field-mappings/bulk", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-field-mappings/groups", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-field-mappings/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/integration-field-mappings/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/integration-field-mappings/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/integration-mappings", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-mappings", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/integration-mappings/bulk", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-mappings/resolve", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/integration-mappings/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/integration-mappings/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/integration-mappings/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inventory", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inventory/reservations", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/reservations/expire", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inventory/reservations/{reservation_id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/reservations/{reservation_id}/release", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/reservations/{reservation_id}/confirm", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/sweeps/run", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/inventory/{sku}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/{sku}/adjust", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/inventory/{sku}/reservations", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/invoices", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/invoices", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/invoices/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Invoice, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/invoices/{id}/send", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/invoices/{id}/payments", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/kernel/audit", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/kernel/audit/checkpoint", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/lots", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/lots/expiring", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/lots/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots/{id}/consume", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots/{id}/reserve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots/reservations/{reservation_id}/release", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots/{id}/quarantine", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/lots/{id}/release-quarantine", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/lots/{id}/genealogy", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/loyalty/programs", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/loyalty/programs", action: ActionClass::List, access: Access::Public },
    RoutePolicy { method: "POST", path: "/api/v1/loyalty/enroll", action: ActionClass::Execute, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/loyalty/accounts/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::LoyaltyAccount, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/negotiations", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/negotiations/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/negotiations/{id}/counter-offer", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/negotiations/{id}/accept", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/negotiations/{id}/reject", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/orders", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/orders", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/orders/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Order, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/orders/{id}/cancel", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Order, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/orders/{id}/ship", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payment-obligations", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/payment-obligations", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/payment-obligations/dashboard", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/payment-obligations/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payment-obligations/{id}/payments", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payment-obligations/{id}/status", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payment-obligations/{id}/bills", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payments", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Reference { field: "order_id", kind: OwnerKind::Order, customer_field: Some("customer_id") } } },
    RoutePolicy { method: "GET", path: "/api/v1/payments", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/payments/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Payment, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/payments/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/payments/{id}/refund", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/prepayments", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/prepayments", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/prepayments/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/prepayments/{id}/apply", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/prepayments/{id}/refund", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/price-levels", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-levels", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-levels/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/price-levels/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/price-levels/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/price-levels/{id}/entries", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-levels/{id}/entries", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/price-schedules", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-schedules", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-schedules/resolve", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-schedules/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/price-schedules/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/price-schedules/{id}/entries", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/price-schedules/{id}/entries", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/print-stations", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/print-stations", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/print-stations/{id}/revoke", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/print-stations/{id}/jobs", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/print-stations/{id}/jobs", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/print-stations/{id}/jobs/next", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/print-jobs/{job_id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/production-batches", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/production-batches", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/production-batches/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/production-batches/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/production-batches/{id}/work-orders", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/products", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/products", action: ActionClass::List, access: Access::Public },
    RoutePolicy { method: "GET", path: "/api/v1/products/{id}", action: ActionClass::Read, access: Access::Public },
    RoutePolicy { method: "PATCH", path: "/api/v1/products/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/products/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/promotions", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/promotions", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/promotions/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PATCH", path: "/api/v1/promotions/{id}/activate", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "PATCH", path: "/api/v1/promotions/{id}/deactivate", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/purchase-orders", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/purchase-orders/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/purchase-orders/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/submit", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/approve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/send", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/acknowledge", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/hold", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purchase-orders/{id}/receive", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/purchase-orders/{id}/items", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/suppliers", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/suppliers", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/suppliers/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/suppliers/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/suppliers/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purgatory/orders", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/purgatory/orders", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/purgatory/orders/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/purgatory/orders/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purgatory/orders/{id}/post", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/purgatory/orders/{id}/lines/{line_id}", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/inspections", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/inspections", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/inspections/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/inspections/{id}/start", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/inspections/{id}/results", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/inspections/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/ncrs", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/ncrs", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/ncrs/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/ncrs/{id}/disposition", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/ncrs/{id}/close", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/holds", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/holds", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/quality/holds/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/quality/holds/{id}/release", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/receipts", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/receipts", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/receipts/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/receipts/{id}/items", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/receipts/{id}/start", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/receipts/{id}/receive", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/receipts/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/receipts/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/put-aways", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/put-aways", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/put-aways/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/put-aways/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reports/inventory-aging", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reports/sales-by-channel", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reports/transaction-cogs", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reports/close-the-books", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reports/consumption", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/returns", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Reference { field: "order_id", kind: OwnerKind::Order, customer_field: None } } },
    RoutePolicy { method: "GET", path: "/api/v1/returns", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/returns/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Return, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/returns/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "PATCH", path: "/api/v1/returns/{id}/approve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "PATCH", path: "/api/v1/returns/{id}/reject", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "PATCH", path: "/api/v1/returns/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/returns/{id}/items/{item_id}/disposition", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/revenue-contracts", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/revenue-contracts", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/revenue-contracts/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/revenue-contracts/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/revenue-contracts/{id}/obligations", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/revenue-obligations/{id}/schedule", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/revenue-obligations/{id}/schedule", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/revenue-obligations/{id}/recognize", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/reviews", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/reviews", action: ActionClass::List, access: Access::Public },
    RoutePolicy { method: "GET", path: "/api/v1/reviews/{id}", action: ActionClass::Read, access: Access::Public },
    RoutePolicy { method: "DELETE", path: "/api/v1/reviews/{id}", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Review, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/segments", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/segments", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/segments/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/segments/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/segments/{id}/members/{customer_id}", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/segments/{id}/members/{customer_id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/serials", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/serials/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials/{id}/reserve", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials/reservations/{reservation_id}/release", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials/{id}/ship", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials/{id}/return", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/serials/{id}/scrap", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/shipments", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/shipments", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/shipments/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Shipment, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/shipments/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/shipments/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/shipments/{id}/deliver", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/shipments/{id}/items", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/shipments/{id}/items/{item_id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/shipping-zones", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/shipping-zones", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/shipping-zones/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/shipping-zones/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/stock-snapshots", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/stock-snapshots", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/stock-snapshots/latest", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/stock-snapshots/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/stock-snapshots/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/store-credits", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/store-credits", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/store-credits/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::StoreCredit, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/store-credits/{id}/adjust", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/store-credits/{id}/apply", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/subscriptions", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/subscriptions", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/subscriptions/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Subscription, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/subscriptions/{id}/pause", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Subscription, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/subscriptions/{id}/resume", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Subscription, param: "id" } },
    RoutePolicy { method: "PATCH", path: "/api/v1/subscriptions/{id}/cancel", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Subscription, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/supplier-skus", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/supplier-skus", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/supplier-skus/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/supplier-skus/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/tax/exemptions", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/tax/exemptions", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/tax/exemptions/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/tax/exemptions/{id}/verify", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/topology-snapshots", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/topology-snapshots", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/topology-snapshots/latest", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/topology-snapshots/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/topology-snapshots/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/transfer-orders", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/transfer-orders", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/transfer-orders/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/transfer-orders/{id}/ship", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/transfer-orders/{id}/receive", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/transfer-orders/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/unit-classes", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/unit-classes", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/units-of-measure", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/units-of-measure", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/unit-conversion-rules", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/unit-conversion-rules", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-credits", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/vendor-credits", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/vendor-credits/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-credits/{id}/apply", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-credits/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-returns", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/vendor-returns", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/vendor-returns/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-returns/{id}/submit", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-returns/{id}/process", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/vendor-returns/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouses", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouses", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouses/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/warehouses/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/warehouses/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-locations", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-locations", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-locations/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-locations/{id}/inventory", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-inventory/adjust", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-inventory/move", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-bins", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-bins", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-bins/adjust", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warehouse-bins/move", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-bins/reconcile", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-bins/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/warehouse-bins/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "DELETE", path: "/api/v1/warehouse-bins/{id}", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/warehouse-bins/{id}/levels", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/cycle-counts", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/cycle-counts", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/cycle-counts/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/cycle-counts/{id}/start", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/cycle-counts/{id}/counts", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/cycle-counts/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/cycle-counts/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/warranties", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Reference { field: "order_id", kind: OwnerKind::Order, customer_field: None } } },
    RoutePolicy { method: "GET", path: "/api/v1/warranties", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/warranties/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Warranty, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/wishlists", action: ActionClass::Create, access: Access::OwnedCreate { body: BodyOwner::Customer("customer_id") } },
    RoutePolicy { method: "GET", path: "/api/v1/wishlists", action: ActionClass::List, access: Access::OwnedList { filter: "customer_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/wishlists/{id}", action: ActionClass::Read, access: Access::Owned { kind: OwnerKind::Wishlist, param: "id" } },
    RoutePolicy { method: "DELETE", path: "/api/v1/wishlists/{id}", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Wishlist, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/wishlists/{id}/items", action: ActionClass::Execute, access: Access::Owned { kind: OwnerKind::Wishlist, param: "id" } },
    RoutePolicy { method: "DELETE", path: "/api/v1/wishlists/{id}/items/{product_id}", action: ActionClass::Delete, access: Access::Owned { kind: OwnerKind::Wishlist, param: "id" } },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/work-orders", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/work-orders/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "PUT", path: "/api/v1/work-orders/{id}", action: ActionClass::Update, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/start", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/hold", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/resume", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/{id}/tasks", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/work-orders/{id}/tasks", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/tasks/{task_id}/start", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/work-orders/tasks/{task_id}/complete", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/x402/intents", action: ActionClass::Create, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/x402/intents", action: ActionClass::List, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/x402/intents/{id}", action: ActionClass::Read, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/x402/intents/{id}/sign", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/x402/intents/{id}/settle", action: ActionClass::Destructive, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/x402/intents/{id}/fail", action: ActionClass::Execute, access: Access::Operator },
    RoutePolicy { method: "POST", path: "/api/v1/x402/intents/{id}/cancel", action: ActionClass::Delete, access: Access::Operator },
    RoutePolicy { method: "GET", path: "/api/v1/x402/carts/{cart_id}/intents", action: ActionClass::List, access: Access::Owned { kind: OwnerKind::Cart, param: "cart_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/x402/orders/{order_id}/intents", action: ActionClass::List, access: Access::Owned { kind: OwnerKind::Order, param: "order_id" } },
    RoutePolicy { method: "GET", path: "/api/v1/openapi.json", action: ActionClass::List, access: Access::Public },
    RoutePolicy { method: "GET", path: "/api/v1/docs", action: ActionClass::List, access: Access::Public },
];

/// Marker lines delimiting the generated table in `docs/src/security/http-authz.md`.
#[cfg(test)]
pub(crate) const DOC_TABLE_BEGIN: &str = "<!-- BEGIN GENERATED ROUTE TABLE (route_policy.rs) -->";
#[cfg(test)]
pub(crate) const DOC_TABLE_END: &str = "<!-- END GENERATED ROUTE TABLE -->";

/// Render the route inventory as the Markdown table committed to the docs.
#[cfg(test)]
pub(crate) fn render_markdown_table() -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let count = |label: &str| ROUTE_POLICIES.iter().filter(|p| p.access.label() == label).count();
    let destructive =
        ROUTE_POLICIES.iter().filter(|p| p.action == ActionClass::Destructive).count();
    let _ = writeln!(
        out,
        "{} routes: {} owned, {} owned-list, {} owned-create, {} public, {} operator, {} infra \
         ({} destructive).\n",
        ROUTE_POLICIES.len(),
        count("owned"),
        count("owned-list"),
        count("owned-create"),
        count("public"),
        count("operator"),
        count("infra"),
        destructive,
    );
    out.push_str("| Method | Path | Action | Access | Ownership rule |\n");
    out.push_str("|---|---|---|---|---|\n");
    for policy in ROUTE_POLICIES {
        let rule = match policy.access {
            Access::Owned { kind, param } => {
                format!("`{{{param}}}` is a {} the customer owns", kind.noun().to_lowercase())
            }
            Access::OwnedList { filter } => format!("`{filter}` forced to the customer"),
            Access::OwnedCreate { body: BodyOwner::Customer(field) } => {
                format!("body `{field}` must be (or is set to) the customer")
            }
            Access::OwnedCreate { body: BodyOwner::Reference { field, kind, customer_field } } => {
                let mut rule = format!(
                    "body `{field}` must be a {} the customer owns",
                    kind.noun().to_lowercase()
                );
                if let Some(customer_field) = customer_field {
                    let _ =
                        write!(rule, "; `{customer_field}` must be (or is set to) the customer");
                }
                rule
            }
            Access::Operator => "customer principals refused (403)".to_string(),
            Access::Public => "any authenticated principal".to_string(),
            Access::Infra => "outside `/api/v1`".to_string(),
        };
        let _ = writeln!(
            out,
            "| {} | `{}` | {} | {} | {} |",
            policy.method,
            policy.path,
            policy.action.as_str(),
            policy.access.label(),
            rule
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    const METHODS: [&str; 5] = ["get", "post", "put", "patch", "delete"];

    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// Extract `(METHOD, template)` pairs from a router source file by scanning
    /// each `.route("...", <method router>)` call. Closure handlers
    /// (`post(move |..| ...)`, `get({ ... })`) are recognised too.
    fn routes_in_source(source: &str, prefix: &str) -> Vec<(String, String)> {
        let Some(start) = source.find("fn router") else { return Vec::new() };
        let body = &source[start..];
        let bytes = body.as_bytes();
        let mut found = Vec::new();
        let mut cursor = 0;
        while let Some(offset) = body[cursor..].find(".route(") {
            let open = cursor + offset + ".route(".len();
            let mut depth = 1usize;
            let mut end = open;
            while depth > 0 && end < bytes.len() {
                match bytes[end] {
                    b'(' => depth += 1,
                    b')' => depth -= 1,
                    _ => {}
                }
                end += 1;
            }
            let call = &body[open..end.saturating_sub(1)];
            cursor = end;
            let Some(first_quote) = call.find('"') else { continue };
            let rest = &call[first_quote + 1..];
            let Some(second_quote) = rest.find('"') else { continue };
            let path = &rest[..second_quote];
            let handlers = &rest[second_quote + 1..];
            let handler_bytes = handlers.as_bytes();
            for method in METHODS {
                let needle = format!("{method}(");
                let mut search = 0;
                while let Some(at) = handlers[search..].find(&needle) {
                    let index = search + at;
                    let preceded_by_ident = index > 0 && {
                        let prev = handler_bytes[index - 1];
                        prev.is_ascii_alphanumeric() || prev == b'_'
                    };
                    if !preceded_by_ident {
                        found.push((method.to_ascii_uppercase(), format!("{prefix}{path}")));
                    }
                    search = index + needle.len();
                }
            }
        }
        found
    }

    fn mounted_routes() -> BTreeSet<(String, String)> {
        let routes_dir = crate_dir().join("src/routes");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&routes_dir)
            .expect("read src/routes")
            .map(|entry| entry.expect("dir entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .filter(|path| path.file_name().is_some_and(|name| name != "mod.rs"))
            .collect();
        files.push(crate_dir().join("src/openapi.rs"));
        let mut mounted = BTreeSet::new();
        for file in files {
            let source = std::fs::read_to_string(&file).expect("read router source");
            let prefix = if file.file_name().is_some_and(|name| name == "health.rs") {
                ""
            } else {
                "/api/v1"
            };
            for route in routes_in_source(&source, prefix) {
                assert!(mounted.insert(route.clone()), "duplicate route {route:?} in {file:?}");
            }
        }
        mounted
    }

    fn classified() -> BTreeSet<(String, String)> {
        ROUTE_POLICIES
            .iter()
            .map(|policy| (policy.method.to_string(), policy.path.to_string()))
            .collect()
    }

    #[test]
    fn the_scanner_sees_the_whole_router() {
        let mounted = mounted_routes();
        assert!(mounted.len() > 400, "scanner found only {} routes", mounted.len());
        assert!(mounted.contains(&("POST".into(), "/api/v1/negotiations/{id}/reject".into())));
        assert!(mounted.contains(&("GET".into(), "/health".into())));
        // `mod.rs` must not mount routes outside a `router()` the scanner reads.
        let mod_rs = std::fs::read_to_string(crate_dir().join("src/routes/mod.rs")).unwrap();
        assert!(!mod_rs.contains(".route("), "routes/mod.rs mounts a route directly");
        assert!(!mod_rs.contains("route_service"), "routes/mod.rs mounts a raw service");
    }

    /// New routes must be classified: a route mounted in `src/routes` without
    /// a row in [`ROUTE_POLICIES`] fails here (and fails closed at runtime).
    #[test]
    fn every_mounted_route_is_classified() {
        let missing: Vec<_> = mounted_routes().difference(&classified()).cloned().collect();
        assert!(
            missing.is_empty(),
            "routes with no authorization classification (add a row to ROUTE_POLICIES in \
             src/route_policy.rs and regenerate docs/src/security/http-authz.md): {missing:#?}"
        );
    }

    #[test]
    fn every_policy_row_names_a_mounted_route() {
        let stale: Vec<_> = classified().difference(&mounted_routes()).cloned().collect();
        assert!(stale.is_empty(), "policy rows for routes that are not mounted: {stale:#?}");
    }

    #[test]
    fn policy_rows_are_unique() {
        assert_eq!(classified().len(), ROUTE_POLICIES.len(), "duplicate policy rows");
    }

    #[test]
    fn every_openapi_operation_is_classified() {
        use utoipa::OpenApi as _;
        let spec = serde_json::to_value(crate::openapi::ApiDoc::openapi()).expect("spec json");
        let mut missing = Vec::new();
        for (template, item) in spec["paths"].as_object().expect("paths") {
            for method in METHODS {
                if item.get(method).is_some()
                    && policy_for(&method.to_ascii_uppercase(), template).is_none()
                {
                    missing.push(format!("{} {template}", method.to_ascii_uppercase()));
                }
            }
        }
        assert!(missing.is_empty(), "documented but unclassified operations: {missing:#?}");
    }

    #[test]
    fn ownership_rules_are_well_formed() {
        for policy in ROUTE_POLICIES {
            match policy.access {
                Access::Owned { param, .. } => {
                    assert!(
                        policy.path.contains(&format!("{{{param}}}")),
                        "{} {} names missing param {param}",
                        policy.method,
                        policy.path
                    );
                }
                Access::OwnedList { .. } => assert_eq!(policy.method, "GET", "{}", policy.path),
                Access::OwnedCreate { .. } => assert_eq!(policy.method, "POST", "{}", policy.path),
                Access::Infra => assert!(!policy.path.starts_with("/api/v1"), "{}", policy.path),
                Access::Public | Access::Operator => {
                    assert!(policy.path.starts_with("/api/v1"), "{}", policy.path);
                }
            }
            // Customer principals never reach a destructive operation.
            if policy.action == ActionClass::Destructive {
                assert_eq!(
                    policy.access,
                    Access::Operator,
                    "{} {} is destructive but not operator-only",
                    policy.method,
                    policy.path
                );
            }
            // Public routes are read-only (currency conversion is a pure calculation).
            if policy.access == Access::Public && policy.method != "GET" {
                assert_eq!(policy.path, "/api/v1/currencies/convert");
            }
        }
    }

    #[test]
    fn money_moving_verbs_are_destructive() {
        for (method, path) in [
            ("POST", "/api/v1/gl/journal-entries/{id}/void"),
            ("POST", "/api/v1/gl/journal-entries/{id}/reverse"),
            ("POST", "/api/v1/ar/write-offs"),
            ("POST", "/api/v1/ar/write-offs/{id}/reverse"),
            ("POST", "/api/v1/fixed-assets/{id}/write-off"),
            ("POST", "/api/v1/gift-cards/{id}/charge"),
            ("POST", "/api/v1/a2a/credit/{id}/charge"),
            ("POST", "/api/v1/x402/intents/{id}/settle"),
            ("POST", "/api/v1/ap/payments/{id}/void"),
            ("POST", "/api/v1/payments/{id}/refund"),
            ("POST", "/api/v1/gl/close-month"),
        ] {
            let policy = policy_for(method, path).expect(path);
            assert_eq!(policy.action, ActionClass::Destructive, "{method} {path}");
            assert_eq!(policy.action.to_authz(), Action::Destructive);
        }
    }

    #[test]
    fn param_extraction_follows_the_template() {
        let policy = policy_for("DELETE", "/api/v1/carts/{id}/items/{item_id}").unwrap();
        let path = "/api/v1/carts/abc/items/def";
        assert_eq!(policy.param(path, "id"), Some("abc"));
        assert_eq!(policy.param(path, "item_id"), Some("def"));
        assert_eq!(policy.first_param(path), Some("abc"));
        assert_eq!(policy.resource_type(), "carts");
        let openapi = policy_for("GET", "/api/v1/openapi.json").unwrap();
        assert_eq!(openapi.resource_type(), "openapi");
    }

    fn doc_path() -> PathBuf {
        crate_dir().join("../../docs/src/security/http-authz.md")
    }

    fn splice(doc: &str, table: &str) -> Option<String> {
        let begin = doc.find(DOC_TABLE_BEGIN)? + DOC_TABLE_BEGIN.len();
        let end = doc.find(DOC_TABLE_END)?;
        Some(format!("{}\n\n{}\n{}", &doc[..begin], table, &doc[end..]))
    }

    /// The committed inventory must match the table. Regenerate with
    /// `STATESET_BLESS_HTTP_AUTHZ_DOC=1 cargo test -p stateset-http route_policy`.
    #[test]
    fn committed_route_inventory_matches_the_policy_table() {
        let path = doc_path();
        let doc = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let expected = splice(&doc, &render_markdown_table()).expect("doc has table markers");
        if std::env::var_os("STATESET_BLESS_HTTP_AUTHZ_DOC").is_some() {
            std::fs::write(&path, &expected).expect("bless doc");
            return;
        }
        assert!(
            doc == expected,
            "{} is stale; rerun with STATESET_BLESS_HTTP_AUTHZ_DOC=1",
            path.display()
        );
    }
}
