use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::SyncError;
use crate::event::{PolicyCheckpoint, SyncEvent};

/// Budget reservation request evaluated by the local transaction kernel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetAuthorization {
    /// Local or remote budget identifier.
    pub budget_id: String,
    /// Requested spend or reservation amount in minor units.
    pub requested_amount_minor: u64,
    /// Currently available amount in the same minor-unit currency.
    pub available_amount_minor: u64,
    /// ISO-style currency code for the reservation.
    pub currency: String,
}

impl BudgetAuthorization {
    /// Create a new local budget authorization request.
    #[must_use]
    pub fn new(
        budget_id: impl Into<String>,
        requested_amount_minor: u64,
        available_amount_minor: u64,
        currency: impl Into<String>,
    ) -> Self {
        Self {
            budget_id: budget_id.into(),
            requested_amount_minor,
            available_amount_minor,
            currency: currency.into(),
        }
    }

    /// Return the remaining budget after this reservation, saturating at zero.
    #[must_use]
    pub const fn remaining_amount_minor(&self) -> u64 {
        self.available_amount_minor.saturating_sub(self.requested_amount_minor)
    }
}

/// A local transaction-kernel request that can enforce policy and budget checks
/// before recording an event in the outbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelTransaction {
    /// Local event to record if policy and budget checks succeed.
    pub event: SyncEvent,
    /// Optional policy checkpoint to enforce and attach to the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicyCheckpoint>,
    /// Optional budget authorization to enforce and attach to the event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetAuthorization>,
}

impl KernelTransaction {
    /// Create a new kernel transaction around a local sync event.
    #[must_use]
    pub const fn new(event: SyncEvent) -> Self {
        Self { event, policy: None, budget: None }
    }

    /// Attach a policy checkpoint to enforce before recording the event.
    #[must_use]
    pub fn with_policy_checkpoint(mut self, policy: PolicyCheckpoint) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Attach a budget authorization to enforce before recording the event.
    #[must_use]
    pub fn with_budget_authorization(mut self, budget: BudgetAuthorization) -> Self {
        self.budget = Some(budget);
        self
    }
}

/// Local execution errors raised by the transaction kernel before an event is recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KernelExecutionError {
    /// The local policy layer denied the mutation.
    PolicyDenied {
        domain: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The local policy layer requires explicit approval before proceeding.
    ApprovalRequired {
        domain: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The requested budget reservation exceeded the available local budget.
    BudgetExceeded {
        budget_id: String,
        requested_amount_minor: u64,
        available_amount_minor: u64,
        currency: String,
    },
    /// Recording the event failed for an underlying sync-engine reason.
    Sync { message: String },
}

impl From<SyncError> for KernelExecutionError {
    fn from(error: SyncError) -> Self {
        Self::Sync { message: error.to_string() }
    }
}

impl fmt::Display for KernelExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyDenied { domain, reason } => match reason {
                Some(reason) => write!(f, "policy denied in domain `{domain}`: {reason}"),
                None => write!(f, "policy denied in domain `{domain}`"),
            },
            Self::ApprovalRequired { domain, reason } => match reason {
                Some(reason) => {
                    write!(f, "approval required in domain `{domain}`: {reason}")
                }
                None => write!(f, "approval required in domain `{domain}`"),
            },
            Self::BudgetExceeded {
                budget_id,
                requested_amount_minor,
                available_amount_minor,
                currency,
            } => write!(
                f,
                "budget `{budget_id}` exceeded: requested {requested_amount_minor} {currency} minor units with {available_amount_minor} available"
            ),
            Self::Sync { message } => f.write_str(message),
        }
    }
}

impl Error for KernelExecutionError {}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn budget_authorization_computes_remaining_amount() {
        let budget = BudgetAuthorization::new("budget-1", 250, 1000, "USD");
        assert_eq!(budget.remaining_amount_minor(), 750);
    }

    #[test]
    fn kernel_transaction_builders_attach_policy_and_budget() {
        let transaction = KernelTransaction::new(SyncEvent::new(
            "order.created",
            "order",
            "ORD-1",
            json!({"total": 25}),
        ))
        .with_policy_checkpoint(PolicyCheckpoint::new(
            "orders",
            crate::event::PolicyDecision::Allowed,
        ))
        .with_budget_authorization(BudgetAuthorization::new("budget-1", 2500, 5000, "USD"));

        assert_eq!(
            transaction.policy.as_ref().map(|policy| policy.domain.as_str()),
            Some("orders")
        );
        assert_eq!(
            transaction
                .budget
                .as_ref()
                .map(|budget| (budget.budget_id.as_str(), budget.remaining_amount_minor())),
            Some(("budget-1", 2500))
        );
    }
}
