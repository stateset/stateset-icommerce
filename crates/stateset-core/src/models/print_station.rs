//! Print station domain models
//!
//! A print station is a paired agent (e.g. a warehouse PC driving label
//! printers) that long-polls for print jobs. Pairing issues a bearer token —
//! returned exactly once — of which only a hash is stored. Jobs are enqueued by
//! the platform and picked up by the station's agent.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{PrintJobId, PrintStationId};
use strum::{Display, EnumString};

/// A paired print station. The pairing token's hash is never exposed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintStation {
    /// Unique station ID.
    pub id: PrintStationId,
    /// Human-readable station name.
    pub name: String,
    /// Printers available at this station.
    pub printers: Vec<String>,
    /// Whether the station's token has been revoked.
    pub revoked: bool,
    /// When the station was last seen (long-poll / job pickup).
    pub last_seen_at: Option<DateTime<Utc>>,
    /// When the station was paired.
    pub created_at: DateTime<Utc>,
    /// When the station was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Result of pairing a station: the station plus its one-time bearer token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairStationResult {
    /// The newly-paired station.
    pub station: PrintStation,
    /// The plaintext bearer token — shown only once; only a hash is persisted.
    pub token: String,
}

/// Kind of print payload.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PrintPayloadKind {
    /// Raw ZPL command stream.
    #[default]
    Zpl,
    /// Base64-encoded PDF bytes.
    Pdf,
}

/// Lifecycle status of a print job.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PrintJobStatus {
    /// Waiting to be picked up by the station agent.
    #[default]
    Queued,
    /// Picked up by the agent, printing in progress.
    PickedUp,
    /// Printed successfully.
    Printed,
    /// Printing failed.
    Failed,
}

impl PrintJobStatus {
    /// Whether the job is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Printed | Self::Failed)
    }
}

/// A print job queued to a station.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJob {
    /// Unique job ID.
    pub id: PrintJobId,
    /// Station the job is queued to.
    pub station_id: PrintStationId,
    /// Target printer name (optional; defaults to the station default).
    pub printer_name: Option<String>,
    /// Payload kind.
    pub payload_kind: PrintPayloadKind,
    /// Payload (raw ZPL text or base64-encoded PDF bytes).
    pub payload: String,
    /// Lifecycle status.
    pub status: PrintJobStatus,
    /// When the job was enqueued.
    pub created_at: DateTime<Utc>,
    /// When the job was picked up by the agent.
    pub picked_up_at: Option<DateTime<Utc>>,
}

/// Input for pairing a print station.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePrintStation {
    /// Station name.
    pub name: String,
    /// Available printers.
    #[serde(default)]
    pub printers: Vec<String>,
}

/// Input for enqueuing a print job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueuePrintJob {
    /// Target printer (optional).
    pub printer_name: Option<String>,
    /// Payload kind.
    pub payload_kind: PrintPayloadKind,
    /// Payload contents.
    pub payload: String,
}

/// Filter for listing print jobs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrintJobFilter {
    /// Filter by status.
    pub status: Option<PrintJobStatus>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_terminal_states() {
        assert!(PrintJobStatus::Printed.is_terminal());
        assert!(PrintJobStatus::Failed.is_terminal());
        assert!(!PrintJobStatus::Queued.is_terminal());
        assert!(!PrintJobStatus::PickedUp.is_terminal());
    }

    #[test]
    fn payload_kind_roundtrip() {
        for k in [PrintPayloadKind::Zpl, PrintPayloadKind::Pdf] {
            assert_eq!(k.to_string().parse::<PrintPayloadKind>().unwrap(), k);
        }
        assert_eq!(PrintPayloadKind::default(), PrintPayloadKind::Zpl);
    }

    #[test]
    fn job_status_roundtrip() {
        for s in [
            PrintJobStatus::Queued,
            PrintJobStatus::PickedUp,
            PrintJobStatus::Printed,
            PrintJobStatus::Failed,
        ] {
            assert_eq!(s.to_string().parse::<PrintJobStatus>().unwrap(), s);
        }
    }
}
