//! Pure planning for durable economic-budget commitments.

use crate::kernel::GuardRejection;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use stateset_core::{CommandEnvelope, CurrencyCode};

/// Locked budget state supplied by a database backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetSnapshot {
    pub budget_id: String,
    pub principal_id: String,
    pub tenant_id: Option<String>,
    pub store_id: Option<String>,
    pub limit: Decimal,
    pub committed: Decimal,
    pub currency: CurrencyCode,
    pub valid_from: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Exact debit accepted by the shared budget plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetDebit {
    pub budget_id: String,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub committed_after: Decimal,
    pub available_after: Decimal,
}

/// Validate a declared budget against locked durable state.
///
/// `Ok(None)` means the command did not name a budget. Policy decides whether
/// that is allowed. A successful apply persists the returned debit in the same
/// transaction as the domain mutation and receipt.
pub fn plan_budget<C>(
    command: &CommandEnvelope<C>,
    amount: Decimal,
    currency: CurrencyCode,
    snapshot: Option<&BudgetSnapshot>,
    now: DateTime<Utc>,
) -> Result<Option<BudgetDebit>, GuardRejection> {
    let Some(budget_id) =
        command.commitment.as_ref().and_then(|commitment| commitment.budget_id.as_deref())
    else {
        return Ok(None);
    };
    let Some(snapshot) = snapshot else {
        return Err(GuardRejection::never(
            "kernel.budget_not_found",
            format!("economic budget `{budget_id}` does not exist"),
        ));
    };
    if snapshot.budget_id != budget_id {
        return Err(GuardRejection::never(
            "kernel.budget_identity_mismatch",
            "loaded budget does not match the declared budget",
        ));
    }
    if snapshot.principal_id != command.principal.id {
        return Err(GuardRejection::never(
            "kernel.budget_principal_mismatch",
            "economic budget belongs to a different principal",
        ));
    }
    if snapshot.tenant_id != command.principal.tenant_id {
        return Err(GuardRejection::never(
            "kernel.budget_tenant_mismatch",
            "economic budget belongs to a different tenant",
        ));
    }
    if snapshot.store_id != command.store_id {
        return Err(GuardRejection::never(
            "kernel.budget_store_mismatch",
            "economic budget belongs to a different store",
        ));
    }
    if snapshot.valid_from > now {
        return Err(GuardRejection::never(
            "kernel.budget_not_yet_valid",
            "economic budget validity window has not started",
        ));
    }
    if snapshot.expires_at <= now {
        return Err(GuardRejection::never("kernel.budget_expired", "economic budget has expired"));
    }
    if snapshot.currency != currency {
        return Err(GuardRejection::never(
            "kernel.budget_currency_mismatch",
            "economic budget currency does not match the command",
        ));
    }
    let Some(committed_after) = snapshot.committed.checked_add(amount) else {
        return Err(GuardRejection::never(
            "kernel.budget_amount_overflow",
            "economic budget commitment overflowed exact decimal arithmetic",
        ));
    };
    if committed_after > snapshot.limit {
        return Err(GuardRejection::never(
            "kernel.budget_exceeded",
            format!(
                "economic budget `{budget_id}` has {} {currency} available but the command requires {amount} {currency}",
                snapshot.limit - snapshot.committed
            ),
        ));
    }
    Ok(Some(BudgetDebit {
        budget_id: budget_id.into(),
        amount,
        currency,
        committed_after,
        available_after: snapshot.limit - committed_after,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::{EconomicCommitment, KernelPrincipal, Money, PrincipalKind};

    fn command(amount: Decimal) -> CommandEnvelope<()> {
        let mut command = CommandEnvelope::preview(
            "payments.create",
            "budget-test",
            KernelPrincipal {
                id: "agent:buyer".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant:one".into()),
                delegated_by: Some("user:one".into()),
                capabilities: vec![],
            },
            (),
        );
        command.store_id = Some("store:one".into());
        command.commitment = Some(EconomicCommitment::for_money(
            "budget:one",
            Money::new(amount, CurrencyCode::USD),
        ));
        command
    }

    #[test]
    fn exact_budget_plan_accepts_boundary_and_rejects_overrun() {
        let now = Utc::now();
        let snapshot = BudgetSnapshot {
            budget_id: "budget:one".into(),
            principal_id: "agent:buyer".into(),
            tenant_id: Some("tenant:one".into()),
            store_id: Some("store:one".into()),
            limit: Decimal::new(10000, 2),
            committed: Decimal::new(7500, 2),
            currency: CurrencyCode::USD,
            valid_from: now - chrono::Duration::hours(1),
            expires_at: now + chrono::Duration::hours(1),
        };
        let debit = plan_budget(
            &command(Decimal::new(2500, 2)),
            Decimal::new(2500, 2),
            CurrencyCode::USD,
            Some(&snapshot),
            now,
        )
        .expect("boundary accepted")
        .expect("budget debit");
        assert_eq!(debit.available_after, Decimal::ZERO);

        let rejection = plan_budget(
            &command(Decimal::new(2501, 2)),
            Decimal::new(2501, 2),
            CurrencyCode::USD,
            Some(&snapshot),
            now,
        )
        .expect_err("overrun rejected");
        assert_eq!(rejection.code, "kernel.budget_exceeded");
    }
}
