//! Inventory commitments for A2A quote acceptance.
//!
//! When a buyer accepts a quote, inventory is locked for a time window.
//! If the purchase completes, the commitment converts to a reservation.
//! If it expires, the stock is automatically released.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an inventory commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentStatus {
    /// Stock is reserved pending purchase completion.
    Reserved,
    /// Purchase completed; commitment fulfilled.
    Fulfilled,
    /// Commitment expired without purchase.
    Expired,
    /// Commitment manually released.
    Released,
}

/// An inventory commitment tying a quote to locked stock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryCommitment {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub purchase_id: Option<Uuid>,
    pub sku: String,
    pub quantity: u32,
    pub status: CommitmentStatus,
    pub reserved_by_agent: String,
    pub expires_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Create a new inventory commitment when a quote is accepted.
#[must_use]
pub fn create_commitment(
    quote_id: Uuid,
    sku: &str,
    quantity: u32,
    agent_id: &str,
    hold_duration: Duration,
) -> InventoryCommitment {
    let now = Utc::now();
    InventoryCommitment {
        id: Uuid::new_v4(),
        quote_id,
        purchase_id: None,
        sku: sku.to_string(),
        quantity,
        status: CommitmentStatus::Reserved,
        reserved_by_agent: agent_id.to_string(),
        expires_at: now + hold_duration,
        released_at: None,
        created_at: now,
    }
}

/// Fulfill a commitment when purchase completes.
pub fn fulfill(commitment: &mut InventoryCommitment, purchase_id: Uuid) {
    commitment.purchase_id = Some(purchase_id);
    commitment.status = CommitmentStatus::Fulfilled;
}

/// Release a commitment (manual or on expiry).
pub fn release(commitment: &mut InventoryCommitment) {
    commitment.status = CommitmentStatus::Released;
    commitment.released_at = Some(Utc::now());
}

/// Check if a commitment has expired.
#[must_use]
pub fn is_expired(commitment: &InventoryCommitment) -> bool {
    commitment.status == CommitmentStatus::Reserved && Utc::now() > commitment.expires_at
}

/// Expire a commitment if past its deadline.
pub fn expire_if_needed(commitment: &mut InventoryCommitment) -> bool {
    if is_expired(commitment) {
        commitment.status = CommitmentStatus::Expired;
        commitment.released_at = Some(Utc::now());
        true
    } else {
        false
    }
}

/// Calculate total committed quantity for a SKU across active commitments.
#[must_use]
pub fn total_committed(commitments: &[InventoryCommitment], sku: &str) -> Decimal {
    commitments
        .iter()
        .filter(|c| c.sku == sku && c.status == CommitmentStatus::Reserved)
        .map(|c| Decimal::from(c.quantity))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_fulfill_commitment() {
        let quote_id = Uuid::new_v4();
        let mut c = create_commitment(quote_id, "SKU-001", 10, "agent-buyer", Duration::hours(24));
        assert_eq!(c.status, CommitmentStatus::Reserved);
        assert_eq!(c.quantity, 10);

        let purchase_id = Uuid::new_v4();
        fulfill(&mut c, purchase_id);
        assert_eq!(c.status, CommitmentStatus::Fulfilled);
        assert_eq!(c.purchase_id, Some(purchase_id));
    }

    #[test]
    fn release_commitment() {
        let mut c = create_commitment(Uuid::new_v4(), "SKU-002", 5, "agent-1", Duration::hours(1));
        release(&mut c);
        assert_eq!(c.status, CommitmentStatus::Released);
        assert!(c.released_at.is_some());
    }

    #[test]
    fn expire_past_deadline() {
        let mut c = create_commitment(Uuid::new_v4(), "SKU-003", 3, "agent-2", Duration::seconds(-1));
        assert!(expire_if_needed(&mut c));
        assert_eq!(c.status, CommitmentStatus::Expired);
    }

    #[test]
    fn total_committed_for_sku() {
        let commitments = vec![
            create_commitment(Uuid::new_v4(), "SKU-A", 10, "a1", Duration::hours(1)),
            create_commitment(Uuid::new_v4(), "SKU-A", 5, "a2", Duration::hours(1)),
            create_commitment(Uuid::new_v4(), "SKU-B", 20, "a3", Duration::hours(1)),
        ];
        assert_eq!(total_committed(&commitments, "SKU-A"), Decimal::from(15));
        assert_eq!(total_committed(&commitments, "SKU-B"), Decimal::from(20));
        assert_eq!(total_committed(&commitments, "SKU-C"), Decimal::from(0));
    }
}
