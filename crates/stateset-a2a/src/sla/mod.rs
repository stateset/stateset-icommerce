//! Service Level Agreements for A2A services.
//!
//! Defines SLA metrics, compliance checking, violation severity, and
//! penalty calculation for agent-to-agent service agreements.

pub mod compliance;
pub mod metrics;
pub mod violations;

pub use compliance::{ComplianceResult, check_compliance};
pub use metrics::{PenaltyType, SlaDefinition, SlaMetricType};
pub use violations::{SlaViolation, ViolationSeverity, compute_penalty_amount};
