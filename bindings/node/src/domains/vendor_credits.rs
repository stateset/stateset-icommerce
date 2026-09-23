//! Vendor Credits  (supplier-owed credits).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Vendor Credits  (supplier-owed credits)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorCreditInput {
    pub supplier_id: String,
    pub vendor_return_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    pub memo: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyVendorCreditInput {
    /// bill or payment_obligation
    #[napi(ts_type = "VendorCreditTargetType")]
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditFilterInput {
    pub supplier_id: Option<String>,
    /// open, applied, cancelled
    #[napi(ts_type = "VendorCreditStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub vendor_return_id: Option<String>,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub remaining: String,
    pub currency: String,
    /// open, applied, cancelled
    #[napi(ts_type = "VendorCreditStatus")]
    pub status: String,
    pub memo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::VendorCredit> for VendorCreditOutput {
    fn from(c: stateset_core::VendorCredit) -> Self {
        Self {
            id: c.id.to_string(),
            number: c.number,
            supplier_id: c.supplier_id.to_string(),
            vendor_return_id: c.vendor_return_id.map(|id| id.to_string()),
            amount: c.amount.to_string(),
            remaining: c.remaining.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            memo: c.memo,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorCreditApplicationOutput {
    pub id: String,
    pub vendor_credit_id: String,
    /// bill or payment_obligation
    #[napi(ts_type = "VendorCreditTargetType")]
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
    pub reversed: bool,
    pub created_at: String,
}

impl From<stateset_core::VendorCreditApplication> for VendorCreditApplicationOutput {
    fn from(a: stateset_core::VendorCreditApplication) -> Self {
        Self {
            id: a.id.to_string(),
            vendor_credit_id: a.vendor_credit_id.to_string(),
            target_type: format!("{}", a.target_type),
            target_id: a.target_id.to_string(),
            amount: a.amount.to_string(),
            reversed: a.reversed,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct VendorCredits {
    pub(crate) commerce: Handle,
}

#[napi]
impl VendorCredits {
    /// Whether the vendor-credits backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.vendor_credits().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateVendorCreditInput) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.get()?;
        let credit = commerce
            .vendor_credits()
            .create(stateset_core::CreateVendorCredit {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                vendor_return_id: parse_optional_uuid(input.vendor_return_id, "vendor_return_id")?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                memo: input.memo,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create vendor credit", e))?;
        Ok(credit.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<VendorCreditOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let credit = commerce
            .vendor_credits()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get vendor credit", e))?;
        Ok(credit.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<VendorCreditFilterInput>,
    ) -> Result<Vec<VendorCreditOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::VendorCreditFilter::default()),
            |f| -> Result<stateset_core::VendorCreditFilter> {
                Ok(stateset_core::VendorCreditFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::VendorCreditStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid vendor credit status")
                            })
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let credits = commerce
            .vendor_credits()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list vendor credits", e))?;
        Ok(credits.into_iter().map(Into::into).collect())
    }

    /// Apply a vendor credit against a bill or payment obligation.
    #[napi]
    pub async fn apply(
        &self,
        id: String,
        input: ApplyVendorCreditInput,
    ) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let target_type = input
            .target_type
            .parse::<stateset_core::VendorCreditTargetType>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid vendor credit target type"))?;
        let credit = commerce
            .vendor_credits()
            .apply(
                uuid.into(),
                stateset_core::ApplyVendorCredit {
                    target_type,
                    target_id: parse_uuid_str(&input.target_id, "target_id")?,
                    amount: parse_decimal_str(&input.amount, "amount")?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply vendor credit", e))?;
        Ok(credit.into())
    }

    /// List applications for a vendor credit.
    #[napi]
    pub async fn list_applications(
        &self,
        id: String,
    ) -> Result<Vec<VendorCreditApplicationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let applications = commerce
            .vendor_credits()
            .list_applications(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list vendor credit applications", e))?;
        Ok(applications.into_iter().map(Into::into).collect())
    }

    /// Reverse a previously-recorded application.
    #[napi]
    pub async fn reverse_application(
        &self,
        id: String,
        application_id: String,
    ) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let app_uuid = parse_uuid_str(&application_id, "application")?;
        let credit =
            commerce.vendor_credits().reverse_application(uuid.into(), app_uuid.into()).map_err(
                |e| wrap(ErrCode::Internal, "Failed to reverse vendor credit application", e),
            )?;
        Ok(credit.into())
    }

    /// Cancel a vendor credit.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<VendorCreditOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor credit")?;
        let credit = commerce
            .vendor_credits()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel vendor credit", e))?;
        Ok(credit.into())
    }
}
