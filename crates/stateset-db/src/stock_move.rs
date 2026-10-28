//! Shared description of a physical stock posting. Backend implementations
//! execute every ledger effect inside the caller's document transaction.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, MovementType, Result};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LotEffect {
    /// Receive unplaced lot units first; issues reduce the lot's remainder.
    Track,
    /// Transfer custody is in transit; only the placement changes.
    PlacementOnly,
    /// Lost owned units in transit, with no remaining physical placement.
    TransitLoss,
    /// The lot repository already updates the lot and placement in this transaction.
    AlreadyRecorded,
    /// The lot repository changed lot totals; placement still follows the move.
    MetadataRecorded,
}

#[derive(Clone)]
pub(crate) struct StockMove<'a> {
    pub id: Uuid,
    pub sku: &'a str,
    pub lot_id: Option<Uuid>,
    pub from: Option<i32>,
    pub to: Option<i32>,
    pub quantity: Decimal,
    /// Release exactly this much of the source's existing allocation.
    pub release_reserved: Decimal,
    /// Reserve this much of the destination's received quantity.
    pub reserve_received: Decimal,
    pub movement_type: MovementType,
    pub lot_effect: LotEffect,
    pub check_sellable: bool,
    pub reference_type: Option<&'a str>,
    pub reference_id: Option<Uuid>,
    pub reference_label: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub performed_by: Option<&'a str>,
}

impl StockMove<'_> {
    pub(crate) fn reference_text(&self) -> Option<String> {
        self.reference_label
            .map(str::to_owned)
            .or_else(|| self.reference_id.map(|id| id.to_string()))
    }
    pub(crate) fn ledger_quantity(&self) -> Decimal {
        if self.to.is_none()
            && matches!(self.movement_type, MovementType::Adjustment | MovementType::CycleCount)
        {
            -self.quantity
        } else {
            self.quantity
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.sku.trim().is_empty()
            || self.quantity <= Decimal::ZERO
            || (self.from.is_none()
                && self.to.is_none()
                && self.lot_effect != LotEffect::TransitLoss)
            || (self.from.is_some() && self.from == self.to)
            || self.release_reserved < Decimal::ZERO
            || self.release_reserved > self.quantity
            || self.reserve_received < Decimal::ZERO
            || self.reserve_received > self.quantity
            || (self.from.is_none() && !self.release_reserved.is_zero())
            || (self.to.is_none() && !self.reserve_received.is_zero())
        {
            return Err(CommerceError::ValidationError("Invalid physical stock movement".into()));
        }
        Ok(())
    }
}
