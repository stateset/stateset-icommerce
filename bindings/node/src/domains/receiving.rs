//! Receiving API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Receiving API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReceiptInput {
    #[napi(ts_type = "ReceiptTypeInput")]
    pub receipt_type: String,
    pub warehouse_id: i32,
    pub purchase_order_id: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReceiptOutput {
    pub id: String,
    pub receipt_number: String,
    #[napi(ts_type = "ReceiptType")]
    pub receipt_type: String,
    pub warehouse_id: i32,
    #[napi(ts_type = "ReceiptStatus")]
    pub status: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::Receipt> for ReceiptOutput {
    fn from(r: stateset_core::Receipt) -> Self {
        Self {
            id: r.id.to_string(),
            receipt_number: r.receipt_number,
            receipt_type: format!("{:?}", r.receipt_type),
            warehouse_id: r.warehouse_id,
            status: format!("{:?}", r.status),
            carrier: r.carrier,
            tracking_number: r.tracking_number,
            created_at: r.created_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_receipt_type(s: &str) -> Result<stateset_core::ReceiptType> {
    Ok(match s.to_lowercase().as_str() {
        "purchase_order" | "purchaseorder" | "po" => stateset_core::ReceiptType::PurchaseOrder,
        "return" | "customer_return" => stateset_core::ReceiptType::Return,
        "transfer" => stateset_core::ReceiptType::Transfer,
        "adjustment" => stateset_core::ReceiptType::Adjustment,
        _ => {
            return Err(unknown_variant(
                "receipt type",
                s,
                &["purchase_order", "po", "return", "customer_return", "transfer", "adjustment"],
            ));
        }
    })
}

/// Optional filters for `Receiving.listReceipts`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ReceiptFilterInput {
    pub warehouse_id: Option<i32>,
    /// The rendered form (`PurchaseOrder`) or the engine's snake_case (`purchase_order`).
    #[napi(ts_type = "ReceiptTypeFilter")]
    pub receipt_type: Option<String>,
    /// The rendered form (`PuttingAway`) or the engine's snake_case (`putting_away`).
    #[napi(ts_type = "ReceiptStatusInput")]
    pub status: Option<String>,
    pub supplier_id: Option<String>,
    pub reference_id: Option<String>,
    /// RFC 3339 timestamp.
    pub from_date: Option<String>,
    /// RFC 3339 timestamp.
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<ReceiptFilterInput> for stateset_core::ReceiptFilter {
    type Error = Error;

    fn try_from(f: ReceiptFilterInput) -> Result<Self> {
        Ok(Self {
            warehouse_id: f.warehouse_id,
            receipt_type: parse_optional_enum(f.receipt_type, "receipt type")?,
            status: parse_optional_enum(f.status, "receipt status")?,
            supplier_id: parse_optional_id(f.supplier_id, "supplier")?,
            reference_id: parse_optional_id(f.reference_id, "reference")?,
            from_date: parse_optional_datetime(f.from_date, "from date")?,
            to_date: parse_optional_datetime(f.to_date, "to date")?,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

#[napi]
pub struct Receiving {
    pub(crate) commerce: Handle,
}

#[napi]
impl Receiving {
    /// Create a new receipt
    #[napi]
    pub async fn create_receipt(&self, input: CreateReceiptInput) -> Result<ReceiptOutput> {
        let commerce = self.commerce.get()?;
        let receipt = commerce
            .receiving()
            .create_receipt(stateset_core::CreateReceipt {
                receipt_number: None,
                receipt_type: parse_receipt_type(&input.receipt_type)?,
                reference_type: input
                    .purchase_order_id
                    .as_ref()
                    .map(|_| "purchase_order".to_string()),
                reference_id: parse_optional_id(input.purchase_order_id, "purchase order")?,
                supplier_id: None,
                warehouse_id: input.warehouse_id,
                carrier: input.carrier,
                tracking_number: input.tracking_number,
                expected_date: None,
                notes: None,
                created_by: None,
                items: vec![],
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create receipt", e))?;
        Ok(receipt.into())
    }

    /// Get a receipt by ID
    #[napi]
    pub async fn get_receipt(&self, id: String) -> Result<Option<ReceiptOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .get_receipt(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get receipt", e))?;
        Ok(receipt.map(|r| r.into()))
    }

    /// Get a receipt by receipt number
    #[napi]
    pub async fn get_receipt_by_number(&self, number: String) -> Result<Option<ReceiptOutput>> {
        let commerce = self.commerce.get()?;
        let receipt = commerce
            .receiving()
            .get_receipt_by_number(&number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get receipt", e))?;
        Ok(receipt.map(|r| r.into()))
    }

    /// List receipts, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_receipts(
        &self,
        filter: Option<ReceiptFilterInput>,
    ) -> Result<Vec<ReceiptOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::ReceiptFilter = filter.unwrap_or_default().try_into()?;
        let receipts = commerce
            .receiving()
            .list_receipts(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list receipts", e))?;
        Ok(receipts.into_iter().map(|r| r.into()).collect())
    }

    /// Start receiving
    #[napi]
    pub async fn start_receiving(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .start_receiving(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to start receiving", e))?;
        Ok(receipt.into())
    }

    /// Complete receiving
    #[napi]
    pub async fn complete_receiving(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .complete_receiving(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete receiving", e))?;
        Ok(receipt.into())
    }

    /// Cancel a receipt
    #[napi]
    pub async fn cancel_receipt(&self, id: String) -> Result<ReceiptOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let receipt = commerce
            .receiving()
            .cancel_receipt(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel receipt", e))?;
        Ok(receipt.into())
    }

    /// Create a receipt from a purchase order
    #[napi]
    pub async fn create_receipt_from_po(
        &self,
        po_id: String,
        warehouse_id: i32,
    ) -> Result<ReceiptOutput> {
        let commerce = self.commerce.get()?;
        let uuid = po_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid PO UUID"))?;
        let receipt = commerce
            .receiving()
            .create_receipt_from_po(uuid, warehouse_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create receipt from PO", e))?;
        Ok(receipt.into())
    }

    /// Count receipts
    #[napi]
    pub async fn count_receipts(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .receiving()
            .count_receipts(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count receipts", e))?;
        Ok(count as u32)
    }
}
