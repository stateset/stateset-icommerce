//! Customer operational topology snapshot models
//!
//! A point-in-time, read-only snapshot of a tenant's operational landscape —
//! channel/warehouse/product counts, open-order backlog, and free-form health
//! signals. Snapshots are captured periodically and stored centrally; the
//! overall health grade is derived from the captured metrics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::TopologySnapshotId;
use strum::{Display, EnumString};

/// Overall operational health grade derived from a snapshot's metrics.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum HealthGrade {
    /// Insufficient data to assess.
    #[default]
    Unknown,
    /// Fully operational.
    Healthy,
    /// Operating with notable gaps.
    Degraded,
    /// A blocking gap (e.g. no active sales channel).
    Critical,
}

/// A captured operational topology snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologySnapshot {
    /// Unique snapshot ID.
    pub id: TopologySnapshotId,
    /// Total channels configured.
    pub channels_total: u64,
    /// Active (non-deleted, unpaused) channels.
    pub channels_active: u64,
    /// Total warehouses.
    pub warehouses_total: u64,
    /// Total products.
    pub products_total: u64,
    /// Open (unfulfilled) order backlog.
    pub open_orders: u64,
    /// Derived overall health grade.
    pub health: HealthGrade,
    /// Free-form health/diagnostic signals.
    pub signals: serde_json::Value,
    /// When the snapshot was captured.
    pub captured_at: DateTime<Utc>,
}

/// Input metrics for capturing a snapshot. Health is derived, not supplied.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptureTopologySnapshot {
    /// Total channels.
    pub channels_total: u64,
    /// Active channels.
    pub channels_active: u64,
    /// Total warehouses.
    pub warehouses_total: u64,
    /// Total products.
    pub products_total: u64,
    /// Open order backlog.
    pub open_orders: u64,
    /// Free-form signals.
    #[serde(default)]
    pub signals: serde_json::Value,
}

impl CaptureTopologySnapshot {
    /// Derive the overall health grade from the captured metrics.
    ///
    /// - `Critical` when there is no active sales channel (orders can't flow in).
    /// - `Degraded` when there are no warehouses or no products.
    /// - `Healthy` otherwise.
    /// - `Unknown` when nothing has been configured at all.
    #[must_use]
    pub const fn derive_health(&self) -> HealthGrade {
        let nothing_configured =
            self.channels_total == 0 && self.warehouses_total == 0 && self.products_total == 0;
        if nothing_configured {
            return HealthGrade::Unknown;
        }
        if self.channels_active == 0 {
            return HealthGrade::Critical;
        }
        if self.warehouses_total == 0 || self.products_total == 0 {
            return HealthGrade::Degraded;
        }
        HealthGrade::Healthy
    }
}

/// Filter for listing topology snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopologySnapshotFilter {
    /// Filter by health grade.
    pub health: Option<HealthGrade>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(
        channels: u64,
        active: u64,
        warehouses: u64,
        products: u64,
    ) -> CaptureTopologySnapshot {
        CaptureTopologySnapshot {
            channels_total: channels,
            channels_active: active,
            warehouses_total: warehouses,
            products_total: products,
            open_orders: 0,
            signals: serde_json::Value::Null,
        }
    }

    #[test]
    fn health_unknown_when_empty() {
        assert_eq!(metrics(0, 0, 0, 0).derive_health(), HealthGrade::Unknown);
    }

    #[test]
    fn health_critical_without_active_channel() {
        assert_eq!(metrics(2, 0, 1, 10).derive_health(), HealthGrade::Critical);
    }

    #[test]
    fn health_degraded_without_warehouse_or_products() {
        assert_eq!(metrics(1, 1, 0, 10).derive_health(), HealthGrade::Degraded);
        assert_eq!(metrics(1, 1, 2, 0).derive_health(), HealthGrade::Degraded);
    }

    #[test]
    fn health_healthy_when_complete() {
        assert_eq!(metrics(2, 1, 3, 100).derive_health(), HealthGrade::Healthy);
    }

    #[test]
    fn grade_roundtrip() {
        for g in [
            HealthGrade::Unknown,
            HealthGrade::Healthy,
            HealthGrade::Degraded,
            HealthGrade::Critical,
        ] {
            assert_eq!(g.to_string().parse::<HealthGrade>().unwrap(), g);
        }
    }
}
