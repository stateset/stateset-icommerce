//! Agent credit terms for net payment between trusted agents.
//!
//! Manages credit lines between agent pairs with configurable payment
//! terms (Net15/30/60/90 or Prepaid), balance tracking, charge/payment
//! recording, overdue detection, and lifecycle management.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{A2AError, A2AResult};

// ---------------------------------------------------------------------------
// PaymentTerms
// ---------------------------------------------------------------------------

/// Standard payment terms governing when invoices are due.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentTerms {
    /// Payment due within 15 days.
    Net15,
    /// Payment due within 30 days.
    Net30,
    /// Payment due within 60 days.
    Net60,
    /// Payment due within 90 days.
    Net90,
    /// Payment required before or at time of charge.
    Prepaid,
}

impl PaymentTerms {
    /// Number of days until payment is due.
    ///
    /// Returns `0` for [`Prepaid`](Self::Prepaid).
    #[must_use]
    pub const fn days(self) -> u32 {
        match self {
            Self::Net15 => 15,
            Self::Net30 => 30,
            Self::Net60 => 60,
            Self::Net90 => 90,
            Self::Prepaid => 0,
        }
    }
}

impl std::fmt::Display for PaymentTerms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Net15 => write!(f, "net_15"),
            Self::Net30 => write!(f, "net_30"),
            Self::Net60 => write!(f, "net_60"),
            Self::Net90 => write!(f, "net_90"),
            Self::Prepaid => write!(f, "prepaid"),
        }
    }
}

// ---------------------------------------------------------------------------
// CreditStatus
// ---------------------------------------------------------------------------

/// Lifecycle status of a credit line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditStatus {
    /// Credit line is open and available for charges.
    Active,
    /// Temporarily frozen — no new charges allowed.
    Suspended,
    /// Permanently closed (balance must be zero).
    Closed,
    /// Debtor has defaulted on outstanding obligations.
    Defaulted,
}

impl std::fmt::Display for CreditStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Closed => write!(f, "closed"),
            Self::Defaulted => write!(f, "defaulted"),
        }
    }
}

// ---------------------------------------------------------------------------
// CreditTransaction
// ---------------------------------------------------------------------------

/// Type of credit transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditTxType {
    /// A new charge against the credit line.
    Charge,
    /// A payment reducing the outstanding balance.
    Payment,
    /// A manual adjustment (positive or negative).
    Adjustment,
}

impl std::fmt::Display for CreditTxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Charge => write!(f, "charge"),
            Self::Payment => write!(f, "payment"),
            Self::Adjustment => write!(f, "adjustment"),
        }
    }
}

/// A single transaction against a credit line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditTransaction {
    /// Unique transaction identifier.
    pub id: Uuid,
    /// The credit terms this transaction belongs to.
    pub credit_terms_id: Uuid,
    /// Transaction amount (always positive).
    pub amount: Decimal,
    /// Type of transaction.
    pub tx_type: CreditTxType,
    /// External reference (e.g. invoice ID, order ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    /// Date on which payment is due (for charges).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<DateTime<Utc>>,
    /// Timestamp when this transaction was actually paid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid_at: Option<DateTime<Utc>>,
    /// Free-form notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// When this transaction was created.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// CreditTerms
// ---------------------------------------------------------------------------

/// A credit line between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditTerms {
    /// Unique credit terms identifier.
    pub id: Uuid,
    /// Agent extending credit.
    pub creditor_agent_id: String,
    /// Agent receiving credit.
    pub debtor_agent_id: String,
    /// Maximum amount of outstanding credit.
    pub credit_limit: Decimal,
    /// Current outstanding balance owed by the debtor.
    pub outstanding_balance: Decimal,
    /// Currency code (e.g. "USD", "EUR").
    pub currency: String,
    /// Payment terms governing due dates.
    pub payment_terms: PaymentTerms,
    /// Current lifecycle status.
    pub status: CreditStatus,
    /// Minimum trust tier the debtor must hold.
    pub min_trust_tier: String,
    /// When this credit line was created.
    pub created_at: DateTime<Utc>,
    /// When this credit line was last modified.
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// CreditManager
// ---------------------------------------------------------------------------

/// Stateless manager for agent credit operations.
///
/// All methods consume and return owned values so the caller controls
/// persistence (database, in-memory store, etc.).
#[derive(Debug)]
pub struct CreditManager;

impl CreditManager {
    /// Create a new credit line between two agents.
    #[must_use]
    pub fn create_terms(
        creditor_agent_id: impl Into<String>,
        debtor_agent_id: impl Into<String>,
        credit_limit: Decimal,
        currency: impl Into<String>,
        payment_terms: PaymentTerms,
        min_trust_tier: impl Into<String>,
    ) -> CreditTerms {
        let now = Utc::now();
        CreditTerms {
            id: Uuid::new_v4(),
            creditor_agent_id: creditor_agent_id.into(),
            debtor_agent_id: debtor_agent_id.into(),
            credit_limit,
            outstanding_balance: Decimal::ZERO,
            currency: currency.into(),
            payment_terms,
            status: CreditStatus::Active,
            min_trust_tier: min_trust_tier.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Charge against an active credit line.
    ///
    /// Validates that the credit line is active, and that the charge does
    /// not exceed the available credit. The due date is computed from the
    /// payment terms.
    ///
    /// # Errors
    ///
    /// - [`A2AError::Validation`] if the credit line is not active.
    /// - [`A2AError::SpendingLimitExceeded`] if the charge exceeds available credit.
    pub fn charge(
        terms: &mut CreditTerms,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> A2AResult<CreditTransaction> {
        if terms.status != CreditStatus::Active {
            return Err(A2AError::validation(format!(
                "credit line is {}, charges require active status",
                terms.status,
            )));
        }

        let available = Self::available_credit(terms);
        if amount > available {
            return Err(A2AError::SpendingLimitExceeded {
                limit_type: "credit_limit".into(),
                limit: terms.credit_limit,
                attempted: amount,
            });
        }

        let now = Utc::now();
        let due_date = now + Duration::days(i64::from(terms.payment_terms.days()));

        terms.outstanding_balance += amount;
        terms.updated_at = now;

        Ok(CreditTransaction {
            id: Uuid::new_v4(),
            credit_terms_id: terms.id,
            amount,
            tx_type: CreditTxType::Charge,
            reference_id,
            due_date: Some(due_date),
            paid_at: None,
            notes: None,
            created_at: now,
        })
    }

    /// Record a payment against a credit line, reducing the outstanding balance.
    ///
    /// # Errors
    ///
    /// - [`A2AError::Validation`] if the payment exceeds the outstanding balance.
    pub fn record_payment(
        terms: &mut CreditTerms,
        amount: Decimal,
        reference_id: Option<String>,
    ) -> A2AResult<CreditTransaction> {
        if amount > terms.outstanding_balance {
            return Err(A2AError::validation(format!(
                "payment of {} exceeds outstanding balance of {}",
                amount, terms.outstanding_balance,
            )));
        }

        let now = Utc::now();
        terms.outstanding_balance -= amount;
        terms.updated_at = now;

        Ok(CreditTransaction {
            id: Uuid::new_v4(),
            credit_terms_id: terms.id,
            amount,
            tx_type: CreditTxType::Payment,
            reference_id,
            due_date: None,
            paid_at: Some(now),
            notes: None,
            created_at: now,
        })
    }

    /// Available credit remaining on the line.
    #[must_use]
    pub fn available_credit(terms: &CreditTerms) -> Decimal {
        terms.credit_limit - terms.outstanding_balance
    }

    /// Whether a charge transaction is overdue (past its due date and unpaid).
    #[must_use]
    pub fn is_overdue(transaction: &CreditTransaction) -> bool {
        if transaction.paid_at.is_some() {
            return false;
        }
        transaction.due_date.is_some_and(|due| Utc::now() > due)
    }

    /// Return all overdue (unpaid and past-due) transactions from a list.
    #[must_use]
    pub fn get_overdue_transactions(transactions: &[CreditTransaction]) -> Vec<&CreditTransaction> {
        let now = Utc::now();
        transactions
            .iter()
            .filter(|tx| tx.paid_at.is_none() && tx.due_date.is_some_and(|due| now > due))
            .collect()
    }

    /// Suspend a credit line, preventing new charges.
    pub fn suspend(terms: &mut CreditTerms) {
        terms.status = CreditStatus::Suspended;
        terms.updated_at = Utc::now();
    }

    /// Close a credit line permanently.
    ///
    /// # Errors
    ///
    /// - [`A2AError::Validation`] if the outstanding balance is not zero.
    pub fn close(terms: &mut CreditTerms) -> A2AResult<()> {
        if terms.outstanding_balance != Decimal::ZERO {
            return Err(A2AError::validation(format!(
                "cannot close credit line with outstanding balance of {}",
                terms.outstanding_balance,
            )));
        }
        terms.status = CreditStatus::Closed;
        terms.updated_at = Utc::now();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn create_and_charge() {
        let mut terms = CreditManager::create_terms(
            "agent_creditor",
            "agent_debtor",
            dec!(10000),
            "USD",
            PaymentTerms::Net30,
            "verified",
        );

        assert_eq!(terms.status, CreditStatus::Active);
        assert_eq!(terms.outstanding_balance, Decimal::ZERO);
        assert_eq!(CreditManager::available_credit(&terms), dec!(10000));

        let tx = CreditManager::charge(&mut terms, dec!(2500), Some("inv_001".into()))
            .expect("charge should succeed");

        assert_eq!(tx.tx_type, CreditTxType::Charge);
        assert_eq!(tx.amount, dec!(2500));
        assert_eq!(tx.reference_id, Some("inv_001".into()));
        assert!(tx.due_date.is_some());
        assert_eq!(terms.outstanding_balance, dec!(2500));
        assert_eq!(CreditManager::available_credit(&terms), dec!(7500));
    }

    #[test]
    fn charge_exceeding_limit_fails() {
        let mut terms = CreditManager::create_terms(
            "agent_creditor",
            "agent_debtor",
            dec!(5000),
            "USD",
            PaymentTerms::Net60,
            "verified",
        );

        let result = CreditManager::charge(&mut terms, dec!(5001), None);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, A2AError::SpendingLimitExceeded { .. }),
            "expected SpendingLimitExceeded, got: {err}",
        );
        // Balance should remain unchanged after failed charge.
        assert_eq!(terms.outstanding_balance, Decimal::ZERO);
    }

    #[test]
    fn payment_reduces_balance() {
        let mut terms = CreditManager::create_terms(
            "agent_creditor",
            "agent_debtor",
            dec!(10000),
            "EUR",
            PaymentTerms::Net90,
            "trusted",
        );

        CreditManager::charge(&mut terms, dec!(4000), None).expect("charge should succeed");
        assert_eq!(terms.outstanding_balance, dec!(4000));

        let tx = CreditManager::record_payment(&mut terms, dec!(1500), Some("pay_001".into()))
            .expect("payment should succeed");

        assert_eq!(tx.tx_type, CreditTxType::Payment);
        assert_eq!(tx.amount, dec!(1500));
        assert!(tx.paid_at.is_some());
        assert_eq!(terms.outstanding_balance, dec!(2500));
        assert_eq!(CreditManager::available_credit(&terms), dec!(7500));
    }

    #[test]
    fn close_with_outstanding_fails() {
        let mut terms = CreditManager::create_terms(
            "agent_creditor",
            "agent_debtor",
            dec!(10000),
            "USD",
            PaymentTerms::Net30,
            "verified",
        );

        CreditManager::charge(&mut terms, dec!(100), None).expect("charge should succeed");

        let result = CreditManager::close(&mut terms);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)), "expected Validation error, got: {err}",);
        // Status should remain unchanged after failed close.
        assert_ne!(terms.status, CreditStatus::Closed);
    }
}
