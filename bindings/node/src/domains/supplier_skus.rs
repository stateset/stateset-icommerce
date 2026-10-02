//! Supplier SKUs  (per-supplier SKU / unit-cost overrides).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Supplier SKUs  (per-supplier SKU / unit-cost overrides)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateSupplierSkuInput {
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateSupplierSkuInput {
    pub sku: Option<String>,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
    pub is_preferred: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierSkuFilterInput {
    pub supplier_id: Option<String>,
    pub product_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BulkSupplierSkuItemInput {
    pub product_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct SupplierSkuOutput {
    pub id: String,
    pub product_id: String,
    pub supplier_id: String,
    pub sku: String,
    /// Exact decimal string
    pub unit_cost: Option<String>,
    pub currency: String,
    /// Exact decimal string
    pub min_order_qty: Option<String>,
    pub lead_time_days: Option<i32>,
    pub is_preferred: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::SupplierSku> for SupplierSkuOutput {
    fn from(s: stateset_core::SupplierSku) -> Self {
        Self {
            id: s.id.to_string(),
            product_id: s.product_id.to_string(),
            supplier_id: s.supplier_id.to_string(),
            sku: s.sku,
            unit_cost: s.unit_cost.map(|c| c.to_string()),
            currency: s.currency.to_string(),
            min_order_qty: s.min_order_qty.map(|q| q.to_string()),
            lead_time_days: s.lead_time_days,
            is_preferred: s.is_preferred,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct SupplierSkus {
    pub(crate) commerce: Handle,
}

#[napi]
impl SupplierSkus {
    /// Whether the supplier-SKUs backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.supplier_skus().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateSupplierSkuInput) -> Result<SupplierSkuOutput> {
        let commerce = self.commerce.get()?;
        let record = commerce
            .supplier_skus()
            .create(stateset_core::CreateSupplierSku {
                product_id: parse_uuid_str(&input.product_id, "product")?.into(),
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier")?,
                sku: input.sku,
                unit_cost: parse_optional_decimal_str(input.unit_cost, "unit_cost")?,
                currency: parse_currency_opt(input.currency)?,
                min_order_qty: parse_optional_decimal_str(input.min_order_qty, "min_order_qty")?,
                lead_time_days: input.lead_time_days,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create supplier SKU", e))?;
        Ok(record.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<SupplierSkuOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        let record = commerce
            .supplier_skus()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get supplier SKU", e))?;
        Ok(record.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdateSupplierSkuInput,
    ) -> Result<SupplierSkuOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        let record = commerce
            .supplier_skus()
            .update(
                uuid.into(),
                stateset_core::UpdateSupplierSku {
                    sku: input.sku,
                    unit_cost: parse_optional_decimal_str(input.unit_cost, "unit_cost")?,
                    currency: parse_currency_opt(input.currency)?,
                    min_order_qty: parse_optional_decimal_str(
                        input.min_order_qty,
                        "min_order_qty",
                    )?,
                    lead_time_days: input.lead_time_days,
                    is_preferred: input.is_preferred,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update supplier SKU", e))?;
        Ok(record.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<SupplierSkuFilterInput>,
    ) -> Result<Vec<SupplierSkuOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::SupplierSkuFilter::default()),
            |f| -> Result<stateset_core::SupplierSkuFilter> {
                Ok(stateset_core::SupplierSkuFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    product_id: parse_optional_uuid(f.product_id, "product_id")?.map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let records = commerce
            .supplier_skus()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list supplier SKUs", e))?;
        Ok(records.into_iter().map(Into::into).collect())
    }

    /// Delete a supplier SKU.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "supplier SKU")?;
        commerce
            .supplier_skus()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete supplier SKU", e))?;
        Ok(())
    }

    /// Bulk upsert supplier SKUs for a supplier, keyed by internal product.
    /// Returns the number of records upserted.
    #[napi]
    pub async fn bulk_upsert(
        &self,
        supplier_id: String,
        items: Vec<BulkSupplierSkuItemInput>,
    ) -> Result<i64> {
        let commerce = self.commerce.get()?;
        let supplier_uuid = parse_uuid_str(&supplier_id, "supplier")?;
        let items = items
            .into_iter()
            .map(|i| -> Result<stateset_core::BulkSupplierSkuItem> {
                Ok(stateset_core::BulkSupplierSkuItem {
                    product_id: parse_uuid_str(&i.product_id, "product")?.into(),
                    sku: i.sku,
                    unit_cost: parse_optional_decimal_str(i.unit_cost, "unit_cost")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let count = commerce
            .supplier_skus()
            .bulk_upsert(supplier_uuid, items)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to bulk upsert supplier SKUs", e))?;
        i64::try_from(count)
            .map_err(|_| coded(ErrCode::Validation, "Bulk upsert count exceeds i64 range"))
    }
}
