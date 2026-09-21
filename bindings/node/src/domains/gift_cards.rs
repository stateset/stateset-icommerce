//! Gift Cards  (money represented as exact decimal STRINGS, not f64 — new code avoids the precision loss of the binding's older f64 money fields).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Gift Cards  (money represented as exact decimal STRINGS, not f64 — new code
// avoids the precision loss of the binding's older f64 money fields)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGiftCardInput {
    /// Redemption code (auto-generated if omitted)
    pub code: Option<String>,
    /// Initial balance as an exact decimal string, e.g. "50.00"
    pub initial_balance: String,
    /// Currency code, e.g. "USD"
    pub currency: String,
    pub recipient_email: Option<String>,
    pub sender_name: Option<String>,
    pub message: Option<String>,
    /// RFC 3339 expiry timestamp
    pub expires_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateGiftCardInput {
    #[napi(ts_type = "GiftCardStatus")]
    pub status: Option<String>,
    pub recipient_email: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardFilterInput {
    #[napi(ts_type = "GiftCardStatus")]
    pub status: Option<String>,
    pub code: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardOutput {
    pub id: String,
    pub code: String,
    /// Exact decimal string
    pub initial_balance: String,
    /// Exact decimal string
    pub current_balance: String,
    pub currency: String,
    #[napi(ts_type = "GiftCardStatus")]
    pub status: String,
    pub recipient_email: Option<String>,
    pub sender_name: Option<String>,
    pub message: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::GiftCard> for GiftCardOutput {
    fn from(g: stateset_core::GiftCard) -> Self {
        Self {
            id: g.id.to_string(),
            code: g.code,
            initial_balance: g.initial_balance.to_string(),
            current_balance: g.current_balance.to_string(),
            currency: g.currency.to_string(),
            status: format!("{}", g.status),
            recipient_email: g.recipient_email,
            sender_name: g.sender_name,
            message: g.message,
            expires_at: g.expires_at.map(|d| d.to_rfc3339()),
            created_at: g.created_at.to_rfc3339(),
            updated_at: g.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GiftCardTransactionOutput {
    pub id: String,
    pub gift_card_id: String,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub balance_after: String,
    #[napi(ts_type = "GiftCardTransactionType")]
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::GiftCardTransaction> for GiftCardTransactionOutput {
    fn from(t: stateset_core::GiftCardTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            gift_card_id: t.gift_card_id.to_string(),
            amount: t.amount.to_string(),
            balance_after: t.balance_after.to_string(),
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct GiftCards {
    pub(crate) commerce: Handle,
}

#[napi]
impl GiftCards {
    /// Whether the gift-cards backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.gift_cards().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateGiftCardInput) -> Result<GiftCardOutput> {
        let commerce = self.commerce.get()?;
        let initial_balance = input
            .initial_balance
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid initial_balance decimal"))?;
        let currency = input
            .currency
            .parse::<CurrencyCode>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid currency code"))?;
        let expires_at = match input.expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        coded(ErrCode::Validation, "Invalid expires_at RFC 3339 timestamp")
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .create(stateset_core::CreateGiftCard {
                code: input.code,
                initial_balance,
                currency,
                recipient_email: input.recipient_email,
                sender_name: input.sender_name,
                message: input.message,
                expires_at,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create gift card", e))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<GiftCardOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get gift card", e))?;
        Ok(card.map(Into::into))
    }

    #[napi]
    pub async fn get_by_code(&self, code: String) -> Result<Option<GiftCardOutput>> {
        let commerce = self.commerce.get()?;
        let card = commerce
            .gift_cards()
            .get_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get gift card by code", e))?;
        Ok(card.map(Into::into))
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateGiftCardInput) -> Result<GiftCardOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let status = match input.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid gift card status"))?,
            ),
            None => None,
        };
        let card = commerce
            .gift_cards()
            .update(
                uuid.into(),
                stateset_core::UpdateGiftCard {
                    status,
                    recipient_email: input.recipient_email,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update gift card", e))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn list(&self, filter: Option<GiftCardFilterInput>) -> Result<Vec<GiftCardOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or(GiftCardFilterInput {
            status: None,
            code: None,
            limit: None,
            offset: None,
        });
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::GiftCardStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid gift card status"))?,
            ),
            None => None,
        };
        let cards = commerce
            .gift_cards()
            .list(stateset_core::GiftCardFilter {
                status,
                code: filter.code,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list gift cards", e))?;
        Ok(cards.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn charge(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransactionOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .charge(uuid.into(), amount, reference_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to charge gift card", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn refund(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<GiftCardTransactionOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid amount decimal"))?;
        let txn = commerce
            .gift_cards()
            .refund(uuid.into(), amount, reference_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to refund gift card", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn disable(&self, id: String) -> Result<GiftCardOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let card = commerce
            .gift_cards()
            .disable(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to disable gift card", e))?;
        Ok(card.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        gift_card_id: String,
    ) -> Result<Vec<GiftCardTransactionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            gift_card_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let txns = commerce
            .gift_cards()
            .get_transactions(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get transactions", e))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}
