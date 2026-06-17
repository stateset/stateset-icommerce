//! EDI document domain models
//!
//! Tracks Electronic Data Interchange documents exchanged with trading partners
//! — purchase orders (850), PO acknowledgements (855), advance ship notices
//! (856), invoices (810), etc. Each document records its direction, processing
//! status, partner, and raw payload, and rolls up into an aggregate summary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::EdiDocumentId;
use strum::{Display, EnumString};

/// Direction of an EDI document relative to this system.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum EdiDirection {
    /// Received from a trading partner.
    #[default]
    Inbound,
    /// Sent to a trading partner.
    Outbound,
}

/// Processing status of an EDI document.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum EdiStatus {
    /// Received/created, not yet processed or sent.
    #[default]
    Pending,
    /// Sent to the partner (outbound).
    Sent,
    /// Acknowledged by the partner (e.g. 997 functional ack).
    Acknowledged,
    /// Successfully processed into the system.
    Processed,
    /// Errored during processing or transmission.
    Error,
}

impl EdiStatus {
    /// Whether this status represents a failure.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }
}

/// An EDI document exchanged with a trading partner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdiDocument {
    /// Unique document ID.
    pub id: EdiDocumentId,
    /// EDI document type (e.g. `850`, `855`, `856`, `810`).
    pub document_type: String,
    /// Direction relative to this system.
    pub direction: EdiDirection,
    /// Processing status.
    pub status: EdiStatus,
    /// Trading partner name / id.
    pub partner: Option<String>,
    /// Related business reference (PO number, order number, etc.).
    pub reference: Option<String>,
    /// Raw EDI payload.
    pub payload: Option<String>,
    /// Error detail when `status = Error`.
    pub error_message: Option<String>,
    /// When the document was created/ingested.
    pub created_at: DateTime<Utc>,
    /// When the document was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating / ingesting an EDI document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEdiDocument {
    /// EDI document type.
    pub document_type: String,
    /// Direction (defaults to inbound).
    #[serde(default)]
    pub direction: EdiDirection,
    /// Trading partner.
    pub partner: Option<String>,
    /// Business reference.
    pub reference: Option<String>,
    /// Raw payload.
    pub payload: Option<String>,
}

/// Filter for listing EDI documents.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EdiDocumentFilter {
    /// Filter by document type.
    pub document_type: Option<String>,
    /// Filter by direction.
    pub direction: Option<EdiDirection>,
    /// Filter by status.
    pub status: Option<EdiStatus>,
    /// Filter by partner.
    pub partner: Option<String>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

/// A keyed count used in aggregate summaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdiCount {
    /// The group key (status or type).
    pub key: String,
    /// Number of documents in the group.
    pub count: u64,
}

/// Aggregate summary of EDI documents.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EdiAggregateSummary {
    /// Total document count.
    pub total: u64,
    /// Counts grouped by status.
    pub by_status: Vec<EdiCount>,
    /// Counts grouped by document type.
    pub by_type: Vec<EdiCount>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status() {
        assert!(EdiStatus::Error.is_error());
        assert!(!EdiStatus::Processed.is_error());
    }

    #[test]
    fn direction_roundtrip() {
        for d in [EdiDirection::Inbound, EdiDirection::Outbound] {
            assert_eq!(d.to_string().parse::<EdiDirection>().unwrap(), d);
        }
        assert_eq!(EdiDirection::default(), EdiDirection::Inbound);
    }

    #[test]
    fn status_roundtrip() {
        for s in [
            EdiStatus::Pending,
            EdiStatus::Sent,
            EdiStatus::Acknowledged,
            EdiStatus::Processed,
            EdiStatus::Error,
        ] {
            assert_eq!(s.to_string().parse::<EdiStatus>().unwrap(), s);
        }
    }
}
