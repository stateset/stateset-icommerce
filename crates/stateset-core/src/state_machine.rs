//! One vocabulary for every status lifecycle in the engine.
//!
//! Each status enum grew its own transition API over time: some expose
//! `can_transition_to`, some `allowed_transitions`, some only `is_terminal`,
//! and many have no table at all — their rules live in repository guards.
//! That made a simple question ("where can this order go next?") impossible to
//! ask generically, for a person, a program or an agent.
//!
//! [`StateMachine`] is the common surface. It is implemented *on top of* each
//! enum's existing `can_transition_to`, so it cannot disagree with the rule the
//! repositories already enforce and it changes no behaviour:
//!
//! ```
//! use stateset_core::{OrderStatus, StateMachine};
//!
//! assert_eq!(
//!     OrderStatus::Pending.next_states(),
//!     vec![OrderStatus::Confirmed, OrderStatus::Cancelled]
//! );
//! assert!(OrderStatus::Refunded.is_terminal_state());
//!
//! let refused = OrderStatus::Pending.check_transition(OrderStatus::Delivered).unwrap_err();
//! assert_eq!(
//!     refused.to_string(),
//!     "order cannot move from pending to delivered; allowed next states: confirmed, cancelled"
//! );
//! ```
//!
//! [`catalog`] lists every machine as plain data (see
//! [`StateMachineDescriptor`]) so bindings, HTTP, the CLI and MCP tools can
//! publish the same graph without re-typing it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::models::{
    A2APaymentStatus, A2AQuoteStatus, BillingCycleStatus, CartStatus, ClaimStatus, CustomerStatus,
    CycleCountStatus, FixedAssetStatus, LotStatus, OrderStatus, PaymentRequestStatus,
    PaymentTransactionStatus, PlanStatus, ProductStatus, PurchaseOrderStatus, PurchaseStatus,
    QuoteStatus, RefundStatus, ReturnStatus, RevenueContractStatus, RevenueEntryStatus,
    SerialStatus, ShipmentStatus, SubscriptionStatus, WorkOrderStatus,
};

/// A status enum whose legal transitions are written down in one place.
///
/// Implementations delegate [`permits`](Self::permits) to the enum's own
/// transition rule; everything else is derived from it and from
/// [`STATES`](Self::STATES), so the derived answers can never drift from the
/// rule the engine enforces.
pub trait StateMachine: Copy + Eq + fmt::Display + 'static {
    /// Stable machine name, e.g. `"order"`. Unique across [`catalog`].
    const NAME: &'static str;
    /// Tool/documentation domain the machine belongs to, e.g. `"orders"`.
    const DOMAIN: &'static str;
    /// Every state, in declaration order.
    const STATES: &'static [Self];

    /// The state a new record takes unless the creating operation says
    /// otherwise.
    fn initial_state() -> Self;

    /// Whether moving from `self` to `next` is allowed.
    ///
    /// This is exactly the enum's own transition rule. Some machines accept a
    /// no-op move to the current state (see [`is_reentrant`](Self::is_reentrant)).
    fn permits(self, next: Self) -> bool;

    /// The states reachable from `self` in one step, excluding `self`.
    fn next_states(self) -> Vec<Self> {
        Self::STATES.iter().copied().filter(|&s| s != self && self.permits(s)).collect()
    }

    /// Whether no other state is reachable from `self`.
    fn is_terminal_state(self) -> bool {
        self.next_states().is_empty()
    }

    /// Whether a no-op move to the current state is accepted (idempotent
    /// status updates) rather than refused.
    fn is_reentrant(self) -> bool {
        self.permits(self)
    }

    /// `Ok(())` if the move is allowed, otherwise a [`TransitionRefused`]
    /// that says what *is* allowed.
    ///
    /// # Errors
    /// Returns [`TransitionRefused`] when [`permits`](Self::permits) is false.
    fn check_transition(self, next: Self) -> Result<(), TransitionRefused> {
        if self.permits(next) {
            return Ok(());
        }
        Err(TransitionRefused {
            machine: Self::NAME,
            from: self.to_string(),
            to: next.to_string(),
            allowed: self.next_states().iter().map(ToString::to_string).collect(),
        })
    }

    /// This machine as plain data.
    fn descriptor() -> StateMachineDescriptor {
        StateMachineDescriptor {
            name: Self::NAME.to_owned(),
            domain: Self::DOMAIN.to_owned(),
            rust_type: short_type_name::<Self>().to_owned(),
            initial: Self::initial_state().to_string(),
            states: Self::STATES
                .iter()
                .map(|&s| {
                    let next: Vec<String> =
                        s.next_states().iter().map(ToString::to_string).collect();
                    StateDescriptor {
                        name: s.to_string(),
                        terminal: next.is_empty(),
                        reentrant: s.is_reentrant(),
                        next,
                    }
                })
                .collect(),
        }
    }
}

fn short_type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    full.rsplit("::").next().unwrap_or(full)
}

/// A refused transition, carrying the moves that would have been accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRefused {
    /// [`StateMachine::NAME`] of the machine that refused.
    pub machine: &'static str,
    /// Current state.
    pub from: String,
    /// Requested state.
    pub to: String,
    /// States that *are* reachable from `from`. Empty when `from` is terminal.
    pub allowed: Vec<String>,
}

impl fmt::Display for TransitionRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} cannot move from {} to {}; ", self.machine, self.from, self.to)?;
        if self.allowed.is_empty() {
            write!(f, "{} is a terminal state", self.from)
        } else {
            write!(f, "allowed next states: {}", self.allowed.join(", "))
        }
    }
}

impl std::error::Error for TransitionRefused {}

/// One state of a [`StateMachineDescriptor`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDescriptor {
    /// Wire name of the state (the enum's `Display`).
    pub name: String,
    /// States reachable in one step, excluding this one.
    pub next: Vec<String>,
    /// No other state is reachable.
    pub terminal: bool,
    /// A no-op update to this same state is accepted.
    pub reentrant: bool,
}

/// A state machine as plain, serialisable data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMachineDescriptor {
    /// Stable machine name, e.g. `"order"`.
    pub name: String,
    /// Domain the machine belongs to, e.g. `"orders"`.
    pub domain: String,
    /// The Rust enum that owns the rule, e.g. `"OrderStatus"`.
    pub rust_type: String,
    /// State a new record takes unless the creating operation says otherwise.
    pub initial: String,
    /// Every state with its outgoing transitions, in declaration order.
    pub states: Vec<StateDescriptor>,
}

impl StateMachineDescriptor {
    /// Look up a state by wire name.
    #[must_use]
    pub fn state(&self, name: &str) -> Option<&StateDescriptor> {
        self.states.iter().find(|s| s.name == name)
    }

    /// The states reachable from `from` in one step, or `None` if `from` is
    /// not a state of this machine.
    #[must_use]
    pub fn next_states(&self, from: &str) -> Option<&[String]> {
        self.state(from).map(|s| s.next.as_slice())
    }

    /// Render as a Mermaid `stateDiagram-v2` block body.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("stateDiagram-v2\n");
        out.push_str(&format!("    [*] --> {}\n", self.initial));
        for state in &self.states {
            for next in &state.next {
                out.push_str(&format!("    {} --> {}\n", state.name, next));
            }
            if state.terminal {
                out.push_str(&format!("    {} --> [*]\n", state.name));
            }
        }
        out
    }
}

/// Implements [`StateMachine`] for an enum that already has an inherent
/// `can_transition_to`, and fails to compile if a variant is left out.
macro_rules! state_machine {
    ($ty:ident, $name:literal, $domain:literal, [$($variant:ident),+ $(,)?]) => {
        impl StateMachine for $ty {
            const NAME: &'static str = $name;
            const DOMAIN: &'static str = $domain;
            const STATES: &'static [Self] = &[$($ty::$variant),+];

            fn initial_state() -> Self {
                Self::default()
            }

            fn permits(self, next: Self) -> bool {
                self.can_transition_to(next)
            }
        }

        // Exhaustiveness guard: adding a variant without listing it above is
        // a compile error, so STATES can never silently miss a state.
        const _: fn($ty) = |state| match state {
            $($ty::$variant)|+ => {}
        };
    };
}

/// Declares every registered machine once: the trait impls and [`catalog`].
macro_rules! state_machines {
    ($(($ty:ident, $name:literal, $domain:literal, [$($variant:ident),+ $(,)?])),+ $(,)?) => {
        $(state_machine!($ty, $name, $domain, [$($variant),+]);)+

        /// Every registered state machine, as plain data, in a stable order.
        #[must_use]
        pub fn catalog() -> Vec<StateMachineDescriptor> {
            vec![$(<$ty as StateMachine>::descriptor()),+]
        }
    };
}

state_machines![
    (
        OrderStatus,
        "order",
        "orders",
        [Pending, Confirmed, Processing, PartiallyShipped, Shipped, Delivered, Cancelled, Refunded]
    ),
    (
        CartStatus,
        "cart",
        "carts",
        [Active, ReadyForPayment, PaymentPending, Completed, Abandoned, Cancelled, Expired]
    ),
    (
        PaymentTransactionStatus,
        "payment",
        "payments",
        [
            Pending,
            Processing,
            RequiresAction,
            Completed,
            Failed,
            Cancelled,
            Refunded,
            PartiallyRefunded,
            Disputed
        ]
    ),
    (RefundStatus, "refund", "payments", [Pending, Processing, Completed, Failed, Cancelled]),
    (
        ReturnStatus,
        "return",
        "returns",
        [Requested, Approved, Rejected, InTransit, Received, Inspecting, Completed, Cancelled]
    ),
    (
        ShipmentStatus,
        "shipment",
        "shipments",
        [
            Pending,
            Processing,
            ReadyToShip,
            Shipped,
            InTransit,
            OutForDelivery,
            Delivered,
            Failed,
            Returned,
            Cancelled,
            OnHold
        ]
    ),
    (CustomerStatus, "customer", "customers", [Active, Inactive, Suspended, Deleted]),
    (ProductStatus, "product", "products", [Draft, Active, Archived]),
    (
        SubscriptionStatus,
        "subscription",
        "subscriptions",
        [Trial, Active, Paused, PastDue, Cancelled, Expired, Pending]
    ),
    (PlanStatus, "subscription_plan", "subscriptions", [Draft, Active, Archived]),
    (
        BillingCycleStatus,
        "billing_cycle",
        "subscriptions",
        [Scheduled, Processing, Paid, Failed, Skipped, Refunded, Voided]
    ),
    (
        PurchaseOrderStatus,
        "purchase_order",
        "purchase_orders",
        [
            Draft,
            PendingApproval,
            Approved,
            Sent,
            Acknowledged,
            PartiallyReceived,
            Received,
            Completed,
            Cancelled,
            OnHold
        ]
    ),
    (
        WorkOrderStatus,
        "work_order",
        "manufacturing",
        [Planned, InProgress, Completed, PartiallyCompleted, Cancelled, OnHold]
    ),
    (CycleCountStatus, "cycle_count", "inventory", [Draft, InProgress, Completed, Cancelled]),
    (LotStatus, "lot", "lots", [Active, Quarantine, Expired, Consumed, OnHold, Recalled, Scrapped]),
    (
        SerialStatus,
        "serial",
        "serials",
        [
            InProduction,
            Available,
            Reserved,
            Shipped,
            Sold,
            Returned,
            InService,
            InWarranty,
            Quarantined,
            Scrapped,
            Recalled,
            Lost,
            Transferred
        ]
    ),
    (
        FixedAssetStatus,
        "fixed_asset",
        "fixed_assets",
        [Draft, InService, FullyDepreciated, Disposed, WrittenOff]
    ),
    (
        RevenueContractStatus,
        "revenue_contract",
        "revenue_recognition",
        [Draft, Active, Completed, Cancelled]
    ),
    (RevenueEntryStatus, "revenue_entry", "revenue_recognition", [Deferred, Recognized]),
    (
        A2APaymentStatus,
        "a2a_payment",
        "a2a",
        [Pending, Submitted, Completed, Failed, Cancelled, Refunded]
    ),
    (
        PaymentRequestStatus,
        "a2a_payment_request",
        "a2a",
        [Pending, Viewed, Processing, Paid, Declined, Expired, Cancelled]
    ),
    (
        A2AQuoteStatus,
        "a2a_quote",
        "a2a",
        [Requested, Quoted, CounterOffered, Accepted, Declined, Expired, Fulfilled, Cancelled]
    ),
    (
        ClaimStatus,
        "warranty_claim",
        "warranties",
        [Submitted, UnderReview, InfoRequested, Approved, Denied, InProgress, Completed, Cancelled]
    ),
    (QuoteStatus, "skill_quote", "a2a", [Pending, Quoted, Accepted, Rejected, Expired, Purchased]),
    (
        PurchaseStatus,
        "skill_purchase",
        "a2a",
        [
            Initiated,
            PaymentPending,
            Paid,
            Fulfilling,
            Shipped,
            Delivered,
            Completed,
            Cancelled,
            Disputed
        ]
    ),
];

/// Look up one machine by [`StateMachine::NAME`].
#[must_use]
pub fn describe(name: &str) -> Option<StateMachineDescriptor> {
    catalog().into_iter().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;

    #[test]
    fn names_are_unique() {
        let names: Vec<String> = catalog().into_iter().map(|m| m.name).collect();
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate machine name in {names:?}");
    }

    #[test]
    fn every_edge_points_at_a_known_state_and_initial_exists() {
        for machine in catalog() {
            let states: BTreeSet<&str> = machine.states.iter().map(|s| s.name.as_str()).collect();
            assert_eq!(
                states.len(),
                machine.states.len(),
                "{}: duplicate state name",
                machine.name
            );
            assert!(
                states.contains(machine.initial.as_str()),
                "{}: initial {} is not a state",
                machine.name,
                machine.initial
            );
            for state in &machine.states {
                assert_eq!(state.terminal, state.next.is_empty());
                for next in &state.next {
                    assert!(states.contains(next.as_str()), "{}: {next} unknown", machine.name);
                    assert_ne!(next, &state.name, "{}: self-loop listed", machine.name);
                }
            }
        }
    }

    /// The derived graph is exactly the inherent rule — the trait adds no
    /// rule of its own.
    #[test]
    fn derived_graph_matches_the_inherent_rule() {
        fn check<M: StateMachine>(inherent: impl Fn(M, M) -> bool) {
            for &from in M::STATES {
                for &to in M::STATES {
                    assert_eq!(from.permits(to), inherent(from, to), "{} {from}->{to}", M::NAME);
                    if from != to {
                        assert_eq!(from.next_states().contains(&to), inherent(from, to));
                        assert_eq!(from.check_transition(to).is_ok(), inherent(from, to));
                    }
                }
            }
        }
        check::<OrderStatus>(OrderStatus::can_transition_to);
        check::<PaymentTransactionStatus>(PaymentTransactionStatus::can_transition_to);
        check::<ReturnStatus>(ReturnStatus::can_transition_to);
        check::<ShipmentStatus>(|a, b| a.can_transition_to(b));
        check::<SerialStatus>(SerialStatus::can_transition_to);
    }

    /// Where an enum already publishes `is_terminal`, the derived answer
    /// agrees with it (one pinned exception).
    #[test]
    fn derived_terminality_matches_inherent_is_terminal() {
        let mut disagreements: Vec<String> = Vec::new();
        macro_rules! agree {
            ($($ty:ident),+) => {$(
                for &s in <$ty as StateMachine>::STATES {
                    if s.is_terminal_state() != s.is_terminal() {
                        disagreements.push(format!(
                            "{} {s}: is_terminal()={} but next={:?}",
                            <$ty as StateMachine>::NAME,
                            s.is_terminal(),
                            s.next_states().iter().map(ToString::to_string).collect::<Vec<_>>()
                        ));
                    }
                }
            )+};
        }
        agree!(
            PaymentTransactionStatus,
            RefundStatus,
            ReturnStatus,
            ShipmentStatus,
            CustomerStatus,
            ProductStatus,
            SubscriptionStatus,
            PlanStatus,
            BillingCycleStatus,
            PurchaseOrderStatus,
            WorkOrderStatus,
            CycleCountStatus,
            SerialStatus,
            FixedAssetStatus,
            RevenueContractStatus,
            A2APaymentStatus,
            PaymentRequestStatus,
            A2AQuoteStatus
        );
        // Known, pinned: `ShipmentStatus::is_terminal` calls Delivered terminal
        // (no further carrier movement) while the transition rule still lets
        // a delivered parcel be Returned. The catalog reports the rule. Any
        // NEW disagreement fails here.
        assert_eq!(
            disagreements,
            ["shipment delivered: is_terminal()=true but next=[\"returned\"]"],
            "is_terminal() and the transition rule disagree"
        );
    }

    /// The three rules lifted out of the repositories, written out edge by
    /// edge. These were private, duplicated tables in the SQLite and Postgres
    /// backends; this pins that the shared rule is the same table.
    #[test]
    fn lifted_repository_rules_are_unchanged() {
        fn graph<M: StateMachine>(expected: &[(&str, &[&str])]) {
            let actual: Vec<(String, Vec<String>)> = M::STATES
                .iter()
                .map(|s| (s.to_string(), s.next_states().iter().map(ToString::to_string).collect()))
                .collect();
            let expected: Vec<(String, Vec<String>)> = expected
                .iter()
                .map(|(from, next)| {
                    ((*from).to_owned(), next.iter().map(|n| (*n).to_owned()).collect())
                })
                .collect();
            assert_eq!(actual, expected, "{}", M::NAME);
            for &state in M::STATES {
                assert!(state.is_reentrant(), "{} {state}: same-status is a no-op", M::NAME);
            }
        }

        graph::<ClaimStatus>(&[
            ("submitted", &["under_review", "info_requested", "approved", "denied", "cancelled"]),
            ("under_review", &["info_requested", "approved", "denied", "in_progress", "cancelled"]),
            ("info_requested", &["under_review", "approved", "denied", "cancelled"]),
            ("approved", &["in_progress", "completed", "cancelled"]),
            ("denied", &[]),
            ("in_progress", &["completed", "cancelled"]),
            ("completed", &[]),
            ("cancelled", &[]),
        ]);
        graph::<QuoteStatus>(&[
            ("pending", &["quoted", "rejected", "expired"]),
            ("quoted", &["accepted", "rejected", "expired", "purchased"]),
            ("accepted", &["purchased"]),
            ("rejected", &[]),
            ("expired", &[]),
            ("purchased", &[]),
        ]);
        graph::<PurchaseStatus>(&[
            ("initiated", &["payment_pending", "cancelled", "disputed"]),
            ("payment_pending", &["paid", "shipped", "cancelled", "disputed"]),
            ("paid", &["fulfilling", "shipped", "completed", "cancelled", "disputed"]),
            ("fulfilling", &["shipped", "cancelled", "disputed"]),
            ("shipped", &["delivered", "completed", "cancelled", "disputed"]),
            ("delivered", &["completed", "cancelled", "disputed"]),
            ("completed", &[]),
            ("cancelled", &[]),
            ("disputed", &[]),
        ]);
    }

    #[test]
    fn refusal_names_the_allowed_moves() {
        let refused = OrderStatus::Pending.check_transition(OrderStatus::Delivered).unwrap_err();
        assert_eq!(refused.allowed, ["confirmed", "cancelled"]);
        assert_eq!(
            refused.to_string(),
            "order cannot move from pending to delivered; allowed next states: confirmed, cancelled"
        );

        let terminal = OrderStatus::Refunded.check_transition(OrderStatus::Pending).unwrap_err();
        assert!(terminal.allowed.is_empty());
        assert_eq!(
            terminal.to_string(),
            "order cannot move from refunded to pending; refunded is a terminal state"
        );
    }

    #[test]
    fn describe_and_mermaid() {
        let order = describe("order").expect("order machine");
        assert_eq!(order.rust_type, "OrderStatus");
        assert_eq!(order.initial, "pending");
        assert_eq!(order.next_states("shipped"), Some(&["delivered".to_owned()][..]));
        assert_eq!(order.next_states("nope"), None);
        let diagram = order.to_mermaid();
        assert!(diagram.starts_with("stateDiagram-v2\n    [*] --> pending\n"));
        assert!(diagram.contains("    pending --> confirmed\n"));
        assert!(diagram.contains("    refunded --> [*]\n"));
        assert!(describe("missing").is_none());
    }
}
