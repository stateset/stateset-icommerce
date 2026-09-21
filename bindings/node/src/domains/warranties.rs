//! Warranties API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Warranties API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarrantyInput {
    pub customer_id: String,
    pub product_id: Option<String>,
    pub order_id: Option<String>,
    /// Anything other than a recognised tier uses the engine default (`standard`).
    #[napi(ts_type = "WarrantyTypeInput")]
    pub warranty_type: Option<String>,
    pub duration_months: Option<i32>,
    pub serial_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarrantyOutput {
    pub id: String,
    pub warranty_number: String,
    pub customer_id: String,
    pub product_id: Option<String>,
    pub order_id: Option<String>,
    #[napi(ts_type = "WarrantyStatus")]
    pub status: String,
    #[napi(ts_type = "WarrantyType")]
    pub warranty_type: String,
    pub start_date: String,
    pub end_date: String,
    pub created_at: String,
}

impl From<stateset_core::Warranty> for WarrantyOutput {
    fn from(w: stateset_core::Warranty) -> Self {
        Self {
            id: w.id.to_string(),
            warranty_number: w.warranty_number,
            customer_id: w.customer_id.to_string(),
            product_id: w.product_id.map(|id| id.to_string()),
            order_id: w.order_id.map(|id| id.to_string()),
            status: format!("{}", w.status),
            warranty_type: format!("{}", w.warranty_type),
            start_date: w.start_date.to_rfc3339(),
            end_date: w.end_date.map(|d| d.to_rfc3339()).unwrap_or_default(),
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateWarrantyClaimInput {
    pub warranty_id: String,
    pub issue_description: String,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct WarrantyClaimOutput {
    pub id: String,
    pub claim_number: String,
    pub warranty_id: String,
    #[napi(ts_type = "WarrantyClaimStatus")]
    pub status: String,
    pub issue_description: String,
    #[napi(ts_type = "WarrantyClaimResolution")]
    pub resolution: String,
    pub created_at: String,
}

impl From<stateset_core::WarrantyClaim> for WarrantyClaimOutput {
    fn from(c: stateset_core::WarrantyClaim) -> Self {
        Self {
            id: c.id.to_string(),
            claim_number: c.claim_number,
            warranty_id: c.warranty_id.to_string(),
            status: format!("{}", c.status),
            issue_description: c.issue_description,
            resolution: format!("{}", c.resolution),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Warranties {
    pub(crate) commerce: Handle,
}

#[napi]
impl Warranties {
    #[napi]
    pub async fn create(&self, input: CreateWarrantyInput) -> Result<WarrantyOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;

        let product_id = input
            .product_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;

        let order_id = input
            .order_id
            .map(|id| id.parse())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;

        let warranty_type = input
            .warranty_type
            .map(|t| match t.to_lowercase().as_str() {
                "standard" => Ok(stateset_core::WarrantyType::Standard),
                "extended" => Ok(stateset_core::WarrantyType::Extended),
                "limited" => Ok(stateset_core::WarrantyType::Limited),
                "lifetime" => Ok(stateset_core::WarrantyType::Lifetime),
                _ => Err(unknown_variant(
                    "warranty type",
                    &t,
                    &["standard", "extended", "limited", "lifetime"],
                )),
            })
            .transpose()?;

        let warranty = commerce
            .warranties()
            .create(stateset_core::CreateWarranty {
                customer_id,
                product_id,
                order_id,
                warranty_type,
                duration_months: input.duration_months,
                serial_number: input.serial_number,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create warranty", e))?;

        Ok(warranty.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<WarrantyOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let warranty = commerce
            .warranties()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get warranty", e))?;

        Ok(warranty.map(|w| w.into()))
    }

    /// List warranties, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every warranty).
    #[napi]
    pub async fn list(&self, filter: Option<WarrantyFilterInput>) -> Result<Vec<WarrantyOutput>> {
        let commerce = self.commerce.get()?;
        let filter = warranty_filter_from_input(filter)?;
        let warranties = commerce
            .warranties()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list warranties", e))?;

        Ok(warranties.into_iter().map(|w| w.into()).collect())
    }

    #[napi]
    pub async fn create_claim(
        &self,
        input: CreateWarrantyClaimInput,
    ) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.get()?;
        let warranty_id = input
            .warranty_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid warranty UUID"))?;

        let claim = commerce
            .warranties()
            .create_claim(stateset_core::CreateWarrantyClaim {
                warranty_id,
                issue_description: input.issue_description,
                contact_email: input.contact_email,
                contact_phone: input.contact_phone,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create claim", e))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn approve_claim(&self, id: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .approve_claim(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to approve claim", e))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn deny_claim(&self, id: String, reason: String) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let claim = commerce
            .warranties()
            .deny_claim(uuid, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to deny claim", e))?;

        Ok(claim.into())
    }

    /// An unrecognised resolution is refused with `VALIDATION`.
    #[napi(ts_args_type = "id: string, resolution: WarrantyClaimResolutionInput")]
    pub async fn complete_claim(
        &self,
        id: String,
        resolution: String,
    ) -> Result<WarrantyClaimOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let res = match resolution.to_lowercase().as_str() {
            "none" => stateset_core::ClaimResolution::None,
            "repair" => stateset_core::ClaimResolution::Repair,
            "replacement" => stateset_core::ClaimResolution::Replacement,
            "refund" => stateset_core::ClaimResolution::Refund,
            "store_credit" | "storecredit" => stateset_core::ClaimResolution::StoreCredit,
            "denied" => stateset_core::ClaimResolution::Denied,
            _ => {
                return Err(unknown_variant(
                    "warranty claim resolution",
                    &resolution,
                    &["none", "repair", "replacement", "refund", "store_credit", "denied"],
                ));
            }
        };

        let claim = commerce
            .warranties()
            .complete_claim(uuid, res)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete claim", e))?;

        Ok(claim.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .warranties()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count warranties", e))?;

        Ok(count as u32)
    }
}
