//! Vendor Returns.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Vendor Returns
// ============================================================================

pub(crate) fn parse_vendor_return_status(s: &str) -> Result<stateset_core::VendorReturnStatus> {
    s.parse::<stateset_core::VendorReturnStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid vendor return status: {s}")))
}

pub(crate) fn parse_vendor_return_reason(s: &str) -> Result<stateset_core::VendorReturnReason> {
    s.parse::<stateset_core::VendorReturnReason>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid vendor return reason: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorReturnItemInput {
    pub product_id: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub unit_cost: String,
    /// Snake-case reason: `defective`, `overage`, `wrong_item`, `other`
    #[napi(ts_type = "VendorReturnReason")]
    pub reason: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateVendorReturnInput {
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    pub currency: Option<String>,
    pub items: Vec<CreateVendorReturnItemInput>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnFilterInput {
    pub supplier_id: Option<String>,
    /// Snake-case status
    #[napi(ts_type = "VendorReturnStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnItemOutput {
    pub id: String,
    pub vendor_return_id: String,
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub quantity: String,
    /// Exact decimal string
    pub unit_cost: String,
    /// Exact decimal string
    pub line_total: String,
    /// Snake-case reason
    #[napi(ts_type = "VendorReturnReason")]
    pub reason: String,
}

impl From<stateset_core::VendorReturnItem> for VendorReturnItemOutput {
    fn from(i: stateset_core::VendorReturnItem) -> Self {
        let line_total = i.line_total().to_string();
        Self {
            id: i.id.to_string(),
            vendor_return_id: i.vendor_return_id.to_string(),
            product_id: i.product_id.to_string(),
            sku: i.sku,
            quantity: i.quantity.to_string(),
            unit_cost: i.unit_cost.to_string(),
            line_total,
            reason: i.reason.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VendorReturnOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    pub purchase_order_id: Option<String>,
    /// Snake-case status
    #[napi(ts_type = "VendorReturnStatus")]
    pub status: String,
    pub currency: String,
    pub items: Vec<VendorReturnItemOutput>,
    /// Exact decimal string
    pub total_credit: String,
    pub credit_generated: bool,
    pub notes: Option<String>,
    pub processed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::VendorReturn> for VendorReturnOutput {
    fn from(r: stateset_core::VendorReturn) -> Self {
        let total_credit = r.total_credit().to_string();
        Self {
            id: r.id.to_string(),
            number: r.number,
            supplier_id: r.supplier_id.to_string(),
            purchase_order_id: r.purchase_order_id.map(|id| id.to_string()),
            status: r.status.to_string(),
            currency: r.currency.to_string(),
            items: r.items.into_iter().map(Into::into).collect(),
            total_credit,
            credit_generated: r.credit_generated,
            notes: r.notes,
            processed_at: r.processed_at.map(|d| d.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct VendorReturns {
    pub(crate) commerce: Handle,
}

#[napi]
impl VendorReturns {
    /// Whether the vendor-returns backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.vendor_returns().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateVendorReturnInput) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.get()?;
        let items = input
            .items
            .into_iter()
            .map(|i| -> Result<stateset_core::CreateVendorReturnItem> {
                Ok(stateset_core::CreateVendorReturnItem {
                    product_id: parse_uuid_str(&i.product_id, "product_id")?.into(),
                    quantity: parse_decimal_str(&i.quantity, "quantity")?,
                    unit_cost: parse_decimal_str(&i.unit_cost, "unit_cost")?,
                    reason: i
                        .reason
                        .as_deref()
                        .map(parse_vendor_return_reason)
                        .transpose()?
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let vendor_return = commerce
            .vendor_returns()
            .create(stateset_core::CreateVendorReturn {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                purchase_order_id: parse_optional_uuid(
                    input.purchase_order_id,
                    "purchase_order_id",
                )?,
                currency: parse_currency_opt(input.currency)?,
                items,
                notes: input.notes,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create vendor return", e))?;
        Ok(vendor_return.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<VendorReturnOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get vendor return", e))?;
        Ok(vendor_return.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<VendorReturnFilterInput>,
    ) -> Result<Vec<VendorReturnOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::VendorReturnFilter::default()),
            |f| -> Result<stateset_core::VendorReturnFilter> {
                Ok(stateset_core::VendorReturnFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f.status.as_deref().map(parse_vendor_return_status).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let returns = commerce
            .vendor_returns()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list vendor returns", e))?;
        Ok(returns.into_iter().map(Into::into).collect())
    }

    /// Submit a draft vendor return to the supplier.
    #[napi]
    pub async fn submit(&self, id: String) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .submit(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to submit vendor return", e))?;
        Ok(vendor_return.into())
    }

    /// Process a vendor return, optionally generating a vendor credit.
    #[napi]
    pub async fn process(&self, id: String, generate_credit: bool) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .process(uuid.into(), generate_credit)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to process vendor return", e))?;
        Ok(vendor_return.into())
    }

    /// Cancel a vendor return.
    #[napi]
    pub async fn cancel(&self, id: String) -> Result<VendorReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "vendor_return")?;
        let vendor_return = commerce
            .vendor_returns()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel vendor return", e))?;
        Ok(vendor_return.into())
    }
}
