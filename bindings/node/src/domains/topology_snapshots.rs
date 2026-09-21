//! Topology Snapshots.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Topology Snapshots
// ============================================================================

pub(crate) fn parse_health_grade(s: &str) -> Result<stateset_core::HealthGrade> {
    s.parse::<stateset_core::HealthGrade>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid health grade: {s}")))
}

pub(crate) fn parse_u64_str(s: &str, field: &str) -> Result<u64> {
    s.parse::<u64>().map_err(|_| coded(ErrCode::Validation, format!("Invalid {field} count")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CaptureTopologySnapshotInput {
    pub channels_total: String,
    pub channels_active: String,
    pub warehouses_total: String,
    pub products_total: String,
    pub open_orders: String,
    /// JSON string
    pub signals: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TopologySnapshotFilterInput {
    /// Snake-case health grade: `unknown`, `healthy`, `degraded`, `critical`
    #[napi(ts_type = "HealthGrade")]
    pub health: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TopologySnapshotOutput {
    pub id: String,
    pub channels_total: String,
    pub channels_active: String,
    pub warehouses_total: String,
    pub products_total: String,
    pub open_orders: String,
    /// Snake-case health grade
    #[napi(ts_type = "HealthGrade")]
    pub health: String,
    /// JSON string
    pub signals: String,
    pub captured_at: String,
}

impl From<stateset_core::TopologySnapshot> for TopologySnapshotOutput {
    fn from(s: stateset_core::TopologySnapshot) -> Self {
        Self {
            id: s.id.to_string(),
            channels_total: s.channels_total.to_string(),
            channels_active: s.channels_active.to_string(),
            warehouses_total: s.warehouses_total.to_string(),
            products_total: s.products_total.to_string(),
            open_orders: s.open_orders.to_string(),
            health: s.health.to_string(),
            signals: serde_json::to_string(&s.signals).unwrap_or_else(|_| "null".to_string()),
            captured_at: s.captured_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct TopologySnapshots {
    pub(crate) commerce: Handle,
}

#[napi]
impl TopologySnapshots {
    /// Whether the topology-snapshots backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.topology_snapshots().is_supported())
    }

    /// Capture a new snapshot; health is derived from the supplied metrics.
    #[napi]
    pub async fn capture(
        &self,
        input: CaptureTopologySnapshotInput,
    ) -> Result<TopologySnapshotOutput> {
        let commerce = self.commerce.get()?;
        let signals = match input.signals {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| wrap(ErrCode::Validation, "Invalid signals JSON", e))?,
            None => serde_json::Value::Null,
        };
        let snapshot = commerce
            .topology_snapshots()
            .capture(stateset_core::CaptureTopologySnapshot {
                channels_total: parse_u64_str(&input.channels_total, "channels_total")?,
                channels_active: parse_u64_str(&input.channels_active, "channels_active")?,
                warehouses_total: parse_u64_str(&input.warehouses_total, "warehouses_total")?,
                products_total: parse_u64_str(&input.products_total, "products_total")?,
                open_orders: parse_u64_str(&input.open_orders, "open_orders")?,
                signals,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to capture topology snapshot", e))?;
        Ok(snapshot.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<TopologySnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "topology_snapshot")?;
        let snapshot = commerce
            .topology_snapshots()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get topology snapshot", e))?;
        Ok(snapshot.map(Into::into))
    }

    /// Most recent snapshot, if any.
    #[napi]
    pub async fn latest(&self) -> Result<Option<TopologySnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let snapshot = commerce
            .topology_snapshots()
            .latest()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get latest topology snapshot", e))?;
        Ok(snapshot.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<TopologySnapshotFilterInput>,
    ) -> Result<Vec<TopologySnapshotOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::TopologySnapshotFilter::default()),
            |f| -> Result<stateset_core::TopologySnapshotFilter> {
                Ok(stateset_core::TopologySnapshotFilter {
                    health: f.health.as_deref().map(parse_health_grade).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let snapshots = commerce
            .topology_snapshots()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list topology snapshots", e))?;
        Ok(snapshots.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "topology_snapshot")?;
        commerce
            .topology_snapshots()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete topology snapshot", e))
    }
}
