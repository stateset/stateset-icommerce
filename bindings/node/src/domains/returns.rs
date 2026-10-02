//! Returns API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Returns API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReturnItemInput {
    pub order_item_id: String,
    pub quantity: i32,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateReturnInput {
    pub order_id: String,
    /// An unrecognised reason is refused with `VALIDATION`; send `other` explicitly.
    #[napi(ts_type = "ReturnReason")]
    pub reason: String,
    pub reason_details: Option<String>,
    pub idempotency_key: Option<String>,
    pub items: Vec<CreateReturnItemInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReturnOutput {
    pub id: String,
    pub order_id: String,
    #[napi(ts_type = "ReturnStatus")]
    pub status: String,
    #[napi(ts_type = "ReturnReason")]
    pub reason: String,
    pub version: i32,
    pub created_at: String,
    pub idempotency_key: Option<String>,
}

impl From<stateset_core::Return> for ReturnOutput {
    fn from(r: stateset_core::Return) -> Self {
        Self {
            id: r.id.to_string(),
            order_id: r.order_id.to_string(),
            status: format!("{}", r.status),
            reason: format!("{}", r.reason),
            version: r.version,
            created_at: r.created_at.to_rfc3339(),
            idempotency_key: r.idempotency_key,
        }
    }
}

#[napi]
pub struct Returns {
    pub(crate) commerce: Handle,
}

#[napi]
impl Returns {
    #[napi]
    pub async fn create(&self, input: CreateReturnInput) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;

        let order_id =
            input.order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;

        let reason = match input.reason.to_lowercase().as_str() {
            "defective" => stateset_core::ReturnReason::Defective,
            "not_as_described" => stateset_core::ReturnReason::NotAsDescribed,
            "wrong_item" => stateset_core::ReturnReason::WrongItem,
            "no_longer_needed" => stateset_core::ReturnReason::NoLongerNeeded,
            "changed_mind" => stateset_core::ReturnReason::ChangedMind,
            "better_price_found" => stateset_core::ReturnReason::BetterPriceFound,
            "damaged" => stateset_core::ReturnReason::Damaged,
            "other" => stateset_core::ReturnReason::Other,
            _ => {
                return Err(unknown_variant(
                    "return reason",
                    &input.reason,
                    &[
                        "defective",
                        "not_as_described",
                        "wrong_item",
                        "no_longer_needed",
                        "changed_mind",
                        "better_price_found",
                        "damaged",
                        "other",
                    ],
                ));
            }
        };

        let items: Vec<stateset_core::CreateReturnItem> = input
            .items
            .into_iter()
            .map(|i| {
                let order_item_id = i.order_item_id.parse().unwrap_or_default();
                stateset_core::CreateReturnItem {
                    order_item_id,
                    quantity: i.quantity,
                    ..Default::default()
                }
            })
            .collect();

        let ret = commerce
            .returns()
            .create(stateset_core::CreateReturn {
                order_id,
                reason,
                reason_details: input.reason_details,
                idempotency_key: input.idempotency_key,
                items,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create return", e))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ReturnOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let ret = commerce
            .returns()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get return", e))?;

        Ok(ret.map(|r| r.into()))
    }

    #[napi]
    pub async fn approve(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let ret = commerce
            .returns()
            .approve(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to approve return", e))?;

        Ok(ret.into())
    }

    #[napi]
    pub async fn reject(&self, id: String, reason: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let ret = commerce
            .returns()
            .reject(uuid.into(), &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reject return", e))?;

        Ok(ret.into())
    }

    /// List returns, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every return).
    #[napi]
    pub async fn list(&self, filter: Option<ReturnFilterInput>) -> Result<Vec<ReturnOutput>> {
        let commerce = self.commerce.get()?;
        let filter = return_filter_from_input(filter)?;
        let returns = commerce
            .returns()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list returns", e))?;

        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .returns()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count returns", e))?;

        Ok(count as u32)
    }

    #[napi]
    pub async fn list_for_order(&self, order_id: String) -> Result<Vec<ReturnOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;
        let returns = commerce
            .returns()
            .list_for_order(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list returns", e))?;
        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    #[napi]
    pub async fn list_for_customer(&self, customer_id: String) -> Result<Vec<ReturnOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let returns = commerce
            .returns()
            .list_for_customer(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list returns", e))?;
        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    #[napi]
    pub async fn list_pending(&self) -> Result<Vec<ReturnOutput>> {
        let commerce = self.commerce.get()?;
        let returns = commerce
            .returns()
            .list_pending()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list pending returns", e))?;
        Ok(returns.into_iter().map(|r| r.into()).collect())
    }

    #[napi]
    pub async fn mark_received(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ret = commerce
            .returns()
            .mark_received(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to mark return received", e))?;
        Ok(ret.into())
    }

    #[napi]
    pub async fn complete(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ret = commerce
            .returns()
            .complete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete return", e))?;
        Ok(ret.into())
    }

    #[napi]
    pub async fn cancel(&self, id: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ret = commerce
            .returns()
            .cancel(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel return", e))?;
        Ok(ret.into())
    }

    #[napi]
    pub async fn add_tracking(&self, id: String, tracking_number: String) -> Result<ReturnOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let ret = commerce
            .returns()
            .add_tracking(uuid.into(), &tracking_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add tracking", e))?;
        Ok(ret.into())
    }
}
