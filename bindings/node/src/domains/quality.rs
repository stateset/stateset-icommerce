//! Quality Control API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Quality Control API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInspectionInput {
    #[napi(ts_type = "InspectionTypeInput")]
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    pub warehouse_id: Option<i32>,
    pub assigned_to: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InspectionOutput {
    pub id: String,
    pub inspection_number: String,
    #[napi(ts_type = "InspectionType")]
    pub inspection_type: String,
    pub reference_type: String,
    pub reference_id: String,
    #[napi(ts_type = "InspectionStatus")]
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Inspection> for InspectionOutput {
    fn from(i: stateset_core::Inspection) -> Self {
        Self {
            id: i.id.to_string(),
            inspection_number: i.inspection_number,
            inspection_type: format!("{:?}", i.inspection_type),
            reference_type: i.reference_type,
            reference_id: i.reference_id.to_string(),
            status: format!("{:?}", i.status),
            created_at: i.created_at.to_rfc3339(),
            updated_at: i.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateNcrInput {
    #[napi(ts_type = "NcrSourceInput")]
    pub source: String,
    #[napi(ts_type = "NcrSeverityInput")]
    pub severity: String,
    pub sku: String,
    pub quantity_affected: f64,
    pub description: String,
    pub lot_number: Option<String>,
    pub location_id: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct NcrOutput {
    pub id: String,
    pub ncr_number: String,
    #[napi(ts_type = "NcrSource")]
    pub source: String,
    #[napi(ts_type = "NcrSeverity")]
    pub severity: String,
    pub sku: String,
    pub quantity_affected: f64,
    #[napi(ts_type = "NcrStatus")]
    pub status: String,
    pub description: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::NonConformance> for NcrOutput {
    type Error = Error;

    fn try_from(n: stateset_core::NonConformance) -> Result<Self> {
        Ok(Self {
            id: n.id.to_string(),
            ncr_number: n.ncr_number,
            source: format!("{:?}", n.source),
            severity: format!("{:?}", n.severity),
            sku: n.sku,
            quantity_affected: to_f64_checked(n.quantity_affected, "ncr quantity affected")?,
            status: format!("{:?}", n.status),
            description: n.description,
            created_at: n.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateQualityHoldInput {
    pub sku: String,
    pub lot_number: Option<String>,
    pub quantity_held: f64,
    pub reason: String,
    #[napi(ts_type = "QualityHoldTypeInput")]
    pub hold_type: String,
    pub placed_by: Option<String>,
    pub location_id: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct QualityHoldOutput {
    pub id: String,
    pub sku: String,
    pub lot_number: Option<String>,
    pub quantity_held: f64,
    pub reason: String,
    #[napi(ts_type = "QualityHoldType")]
    pub hold_type: String,
    #[napi(ts_type = "QualityHoldStatus")]
    pub status: String,
    pub placed_at: String,
}

impl TryFrom<stateset_core::QualityHold> for QualityHoldOutput {
    type Error = Error;

    fn try_from(h: stateset_core::QualityHold) -> Result<Self> {
        Ok(Self {
            id: h.id.to_string(),
            sku: h.sku,
            lot_number: h.lot_number,
            quantity_held: to_f64_checked(h.quantity_held, "quality hold quantity held")?,
            reason: h.reason,
            hold_type: format!("{:?}", h.hold_type),
            status: if h.released_at.is_some() {
                "released".to_string()
            } else {
                "held".to_string()
            },
            placed_at: h.placed_at.to_rfc3339(),
        })
    }
}

pub(crate) fn parse_inspection_type(s: &str) -> Result<stateset_core::InspectionType> {
    Ok(match s.to_lowercase().as_str() {
        "incoming" => stateset_core::InspectionType::Incoming,
        "receiving" => stateset_core::InspectionType::Receiving,
        "in_process" | "inprocess" => stateset_core::InspectionType::InProcess,
        "final" => stateset_core::InspectionType::Final,
        "random" => stateset_core::InspectionType::Random,
        "return" => stateset_core::InspectionType::Return,
        _ => {
            return Err(unknown_variant(
                "inspection type",
                s,
                &["incoming", "receiving", "in_process", "final", "random", "return"],
            ));
        }
    })
}

pub(crate) fn parse_ncr_source(s: &str) -> Result<stateset_core::NonConformanceSource> {
    Ok(match s.to_lowercase().as_str() {
        "inspection" => stateset_core::NonConformanceSource::Inspection,
        "production" | "production_defect" => stateset_core::NonConformanceSource::ProductionDefect,
        "customer" | "customer_complaint" => stateset_core::NonConformanceSource::CustomerComplaint,
        "supplier" | "supplier_issue" => stateset_core::NonConformanceSource::SupplierIssue,
        "internal_audit" => stateset_core::NonConformanceSource::InternalAudit,
        "shipping_damage" => stateset_core::NonConformanceSource::ShippingDamage,
        _ => {
            return Err(unknown_variant(
                "non-conformance source",
                s,
                &[
                    "inspection",
                    "production",
                    "production_defect",
                    "customer",
                    "customer_complaint",
                    "supplier",
                    "supplier_issue",
                    "internal_audit",
                    "shipping_damage",
                ],
            ));
        }
    })
}

pub(crate) fn parse_severity(s: &str) -> Result<stateset_core::Severity> {
    Ok(match s.to_lowercase().as_str() {
        "critical" => stateset_core::Severity::Critical,
        "major" => stateset_core::Severity::Major,
        "minor" => stateset_core::Severity::Minor,
        "observation" => stateset_core::Severity::Observation,
        _ => {
            return Err(unknown_variant(
                "severity",
                s,
                &["critical", "major", "minor", "observation"],
            ));
        }
    })
}

pub(crate) fn parse_hold_type(s: &str) -> Result<stateset_core::HoldType> {
    Ok(match s.to_lowercase().as_str() {
        "quality_inspection" | "qualityinspection" => stateset_core::HoldType::QualityInspection,
        "damage" | "damaged" => stateset_core::HoldType::Damaged,
        "regulatory" | "regulatory_hold" => stateset_core::HoldType::RegulatoryHold,
        "customer_return" | "customerreturn" => stateset_core::HoldType::CustomerReturn,
        "recall" => stateset_core::HoldType::Recall,
        "expired" => stateset_core::HoldType::Expired,
        "quarantine" => stateset_core::HoldType::Quarantine,
        "investigation" | "investigation_hold" => stateset_core::HoldType::InvestigationHold,
        _ => {
            return Err(unknown_variant(
                "hold type",
                s,
                &[
                    "quality_inspection",
                    "damaged",
                    "damage",
                    "regulatory_hold",
                    "regulatory",
                    "customer_return",
                    "recall",
                    "expired",
                    "quarantine",
                    "investigation_hold",
                    "investigation",
                ],
            ));
        }
    })
}

/// Optional filters for `Quality.listHolds`. No argument lists every hold.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct QualityHoldFilterInput {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    /// The rendered form (`RegulatoryHold`) or the engine's snake_case (`regulatory_hold`).
    #[napi(ts_type = "QualityHoldTypeFilter")]
    pub hold_type: Option<String>,
    pub location_id: Option<i32>,
    /// Only holds that have not been released.
    pub active_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<QualityHoldFilterInput> for stateset_core::QualityHoldFilter {
    type Error = Error;

    fn try_from(f: QualityHoldFilterInput) -> Result<Self> {
        Ok(Self {
            sku: f.sku,
            lot_number: f.lot_number,
            hold_type: parse_optional_enum(f.hold_type, "hold type")?,
            location_id: f.location_id,
            active_only: f.active_only,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct Quality {
    pub(crate) commerce: Handle,
}

#[napi]
impl Quality {
    /// Create a new inspection
    #[napi]
    pub async fn create_inspection(
        &self,
        input: CreateInspectionInput,
    ) -> Result<InspectionOutput> {
        let commerce = self.commerce.get()?;
        let reference_id = input
            .reference_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid reference UUID"))?;

        let inspection = commerce
            .quality()
            .create_inspection(stateset_core::CreateInspection {
                inspection_type: parse_inspection_type(&input.inspection_type)?,
                reference_type: input.reference_type,
                reference_id,
                inspector_id: input.assigned_to,
                scheduled_at: None,
                notes: input.notes,
                items: vec![],
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create inspection", e))?;

        Ok(inspection.into())
    }

    /// Get an inspection by ID
    #[napi]
    pub async fn get_inspection(&self, id: String) -> Result<Option<InspectionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .get_inspection(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get inspection", e))?;
        Ok(inspection.map(|i| i.into()))
    }

    /// List inspections, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (server default page size).
    #[napi]
    pub async fn list_inspections(
        &self,
        filter: Option<InspectionFilterInput>,
    ) -> Result<Vec<InspectionOutput>> {
        let commerce = self.commerce.get()?;
        let filter = inspection_filter_from_input(filter)?;
        let inspections = commerce
            .quality()
            .list_inspections(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list inspections", e))?;
        Ok(inspections.into_iter().map(|i| i.into()).collect())
    }

    /// Start an inspection
    #[napi]
    pub async fn start_inspection(&self, id: String) -> Result<InspectionOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .start_inspection(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to start inspection", e))?;
        Ok(inspection.into())
    }

    /// Complete an inspection
    #[napi]
    pub async fn complete_inspection(&self, id: String) -> Result<InspectionOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let inspection = commerce
            .quality()
            .complete_inspection(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete inspection", e))?;
        Ok(inspection.into())
    }

    /// Create a non-conformance report
    #[napi]
    pub async fn create_ncr(&self, input: CreateNcrInput) -> Result<NcrOutput> {
        let commerce = self.commerce.get()?;
        let ncr = commerce
            .quality()
            .create_ncr(stateset_core::CreateNonConformance {
                inspection_id: None,
                source: parse_ncr_source(&input.source)?,
                severity: parse_severity(&input.severity)?,
                sku: input.sku,
                lot_number: input.lot_number,
                serial_number: None,
                quantity_affected: decimal_from_f64(
                    input.quantity_affected,
                    "ncr quantity affected",
                )?,
                description: input.description,
                assigned_to: None,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create NCR", e))?;
        convert_output(ncr)
    }

    /// Get an NCR by ID
    #[napi]
    pub async fn get_ncr(&self, id: String) -> Result<Option<NcrOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ncr = commerce
            .quality()
            .get_ncr(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get NCR", e))?;
        convert_optional_output(ncr)
    }

    /// List NCRs, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (server default page size).
    #[napi]
    pub async fn list_ncrs(&self, filter: Option<NcrFilterInput>) -> Result<Vec<NcrOutput>> {
        let commerce = self.commerce.get()?;
        let filter = ncr_filter_from_input(filter)?;
        let ncrs = commerce
            .quality()
            .list_ncrs(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list NCRs", e))?;
        convert_outputs(ncrs)
    }

    /// Close an NCR
    #[napi]
    pub async fn close_ncr(&self, id: String) -> Result<NcrOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ncr = commerce
            .quality()
            .close_ncr(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to close NCR", e))?;
        convert_output(ncr)
    }

    /// Create a quality hold
    #[napi]
    pub async fn create_hold(&self, input: CreateQualityHoldInput) -> Result<QualityHoldOutput> {
        let commerce = self.commerce.get()?;
        let hold = commerce
            .quality()
            .create_hold(stateset_core::CreateQualityHold {
                sku: input.sku,
                lot_number: input.lot_number,
                serial_number: None,
                location_id: input.location_id,
                quantity: decimal_from_f64(input.quantity_held, "quality hold quantity")?,
                reason: input.reason,
                hold_type: parse_hold_type(&input.hold_type)?,
                ncr_id: None,
                inspection_id: None,
                placed_by: input.placed_by.unwrap_or_default(),
                expires_at: None,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create hold", e))?;
        convert_output(hold)
    }

    /// Get a quality hold by ID
    #[napi]
    pub async fn get_hold(&self, id: String) -> Result<Option<QualityHoldOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let hold = commerce
            .quality()
            .get_hold(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get hold", e))?;
        convert_optional_output(hold)
    }

    /// List quality holds, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_holds(
        &self,
        filter: Option<QualityHoldFilterInput>,
    ) -> Result<Vec<QualityHoldOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::QualityHoldFilter = filter.unwrap_or_default().try_into()?;
        let holds = commerce
            .quality()
            .list_holds(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list holds", e))?;
        convert_outputs(holds)
    }

    /// Release a quality hold
    #[napi]
    pub async fn release_hold(
        &self,
        id: String,
        released_by: String,
        notes: Option<String>,
    ) -> Result<QualityHoldOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let hold = commerce
            .quality()
            .release_hold(
                uuid,
                stateset_core::ReleaseQualityHold { released_by, release_notes: notes },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release hold", e))?;
        convert_output(hold)
    }

    /// Get all active holds
    #[napi]
    pub async fn get_active_holds(&self) -> Result<Vec<QualityHoldOutput>> {
        let commerce = self.commerce.get()?;
        let holds = commerce
            .quality()
            .get_active_holds()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get active holds", e))?;
        convert_outputs(holds)
    }

    /// Count active holds
    #[napi]
    pub async fn count_active_holds(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .quality()
            .count_active_holds()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count holds", e))?;
        Ok(count as u32)
    }
}
