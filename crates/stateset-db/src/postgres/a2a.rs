//! PostgreSQL implementation of A2A commerce repository

use super::{block_on, map_db_error};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    A2ACommerceRepository, A2APurchase, A2APurchaseFilter, CartAddress, CommerceError, CreateA2APurchase,
    CreateA2AQuote, QuoteStatus, Result, SkillQuote, SkillQuoteFilter, PurchaseStatus,
    QuotedItem, validate_batch_size, X402Asset, X402Network,
};
use std::str::FromStr;
use uuid::Uuid;

/// PostgreSQL A2A commerce repository
#[derive(Clone)]
pub struct PgA2ARepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct QuoteRow {
    id: Uuid,
    quote_number: String,
    status: String,
    buyer_agent_id: Uuid,
    seller_agent_id: Uuid,
    items: Value,
    subtotal: Decimal,
    tax_amount: Decimal,
    shipping_amount: Decimal,
    discount_amount: Decimal,
    total: Decimal,
    currency: String,
    payment_network: Option<String>,
    payment_asset: Option<String>,
    shipping_address: Option<Value>,
    valid_until: chrono::DateTime<Utc>,
    purchase_id: Option<Uuid>,
    payment_intent_id: Option<Uuid>,
    notes: Option<String>,
    metadata: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(FromRow)]
struct PurchaseRow {
    id: Uuid,
    purchase_number: String,
    status: String,
    buyer_agent_id: Uuid,
    seller_agent_id: Uuid,
    quote_id: Option<Uuid>,
    cart_id: Option<Uuid>,
    order_id: Option<Uuid>,
    payment_intent_id: Option<Uuid>,
    items: Value,
    total: Decimal,
    currency: String,
    fulfillment_type: Option<String>,
    tracking_info: Option<Value>,
    delivered_at: Option<chrono::DateTime<Utc>>,
    delivery_confirmed_at: Option<chrono::DateTime<Utc>>,
    delivery_confirmation_signature: Option<String>,
    buyer_rating: Option<i32>,
    buyer_feedback: Option<String>,
    seller_rating: Option<i32>,
    seller_feedback: Option<String>,
    notes: Option<String>,
    metadata: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(FromRow)]
struct QuoteValidationRow {
    buyer_agent_id: Uuid,
    seller_agent_id: Uuid,
    status: String,
    total: Decimal,
    currency: String,
    valid_until: chrono::DateTime<Utc>,
}

impl PgA2ARepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn generate_quote_number() -> String {
        let now = Utc::now();
        let random = Uuid::new_v4();
        format!(
            "QTE-{}-{:08X}",
            now.timestamp_millis(),
            (random.as_u128() & 0xFFFFFFFF) as u32
        )
    }

    fn generate_purchase_number() -> String {
        let now = Utc::now();
        let random = Uuid::new_v4();
        format!(
            "PUR-{}-{:08X}",
            now.timestamp_millis(),
            (random.as_u128() & 0xFFFFFFFF) as u32
        )
    }

    fn parse_quote_status(value: &str, entity: &str, field: &str) -> Result<QuoteStatus> {
        match value.to_ascii_lowercase().as_str() {
            "pending" => Ok(QuoteStatus::Pending),
            "quoted" => Ok(QuoteStatus::Quoted),
            "accepted" => Ok(QuoteStatus::Accepted),
            "rejected" => Ok(QuoteStatus::Rejected),
            "expired" => Ok(QuoteStatus::Expired),
            "purchased" => Ok(QuoteStatus::Purchased),
            _ => Err(CommerceError::DatabaseError(format!(
                "Invalid {}.{} for a2a quote status: '{}'",
                entity, field, value
            ))),
        }
    }

    fn parse_purchase_status(value: &str, entity: &str, field: &str) -> Result<PurchaseStatus> {
        match value.to_ascii_lowercase().as_str() {
            "initiated" => Ok(PurchaseStatus::Initiated),
            "payment_pending" => Ok(PurchaseStatus::PaymentPending),
            "paid" => Ok(PurchaseStatus::Paid),
            "fulfilling" => Ok(PurchaseStatus::Fulfilling),
            "shipped" => Ok(PurchaseStatus::Shipped),
            "delivered" => Ok(PurchaseStatus::Delivered),
            "completed" => Ok(PurchaseStatus::Completed),
            "cancelled" => Ok(PurchaseStatus::Cancelled),
            "disputed" => Ok(PurchaseStatus::Disputed),
            _ => Err(CommerceError::DatabaseError(format!(
                "Invalid {}.{} for a2a purchase status: '{}'",
                entity, field, value
            ))),
        }
    }

    fn is_valid_quote_status_transition(current: QuoteStatus, next: QuoteStatus) -> bool {
        if current == next {
            return true;
        }

        match current {
            QuoteStatus::Pending => {
                matches!(next, QuoteStatus::Quoted | QuoteStatus::Rejected | QuoteStatus::Expired)
            }
            QuoteStatus::Quoted => {
                matches!(next, QuoteStatus::Accepted | QuoteStatus::Rejected | QuoteStatus::Expired)
            }
            QuoteStatus::Accepted => matches!(next, QuoteStatus::Purchased),
            QuoteStatus::Rejected | QuoteStatus::Expired | QuoteStatus::Purchased => false,
        }
    }

    fn is_valid_purchase_status_transition(current: PurchaseStatus, next: PurchaseStatus) -> bool {
        if current == next {
            return true;
        }

        match current {
            PurchaseStatus::Initiated => {
                matches!(next, PurchaseStatus::PaymentPending | PurchaseStatus::Cancelled | PurchaseStatus::Disputed)
            }
            PurchaseStatus::PaymentPending => {
                matches!(
                    next,
                    PurchaseStatus::Paid
                        | PurchaseStatus::Shipped
                        | PurchaseStatus::Cancelled
                        | PurchaseStatus::Disputed
                )
            }
            PurchaseStatus::Paid => {
                matches!(
                    next,
                    PurchaseStatus::Fulfilling
                        | PurchaseStatus::Shipped
                        | PurchaseStatus::Completed
                        | PurchaseStatus::Cancelled
                        | PurchaseStatus::Disputed
                )
            }
            PurchaseStatus::Fulfilling => {
                matches!(next, PurchaseStatus::Shipped | PurchaseStatus::Cancelled | PurchaseStatus::Disputed)
            }
            PurchaseStatus::Shipped => {
                matches!(
                    next,
                    PurchaseStatus::Delivered
                        | PurchaseStatus::Completed
                        | PurchaseStatus::Cancelled
                        | PurchaseStatus::Disputed
                )
            }
            PurchaseStatus::Delivered => {
                matches!(next, PurchaseStatus::Completed | PurchaseStatus::Cancelled | PurchaseStatus::Disputed)
            }
            PurchaseStatus::Completed | PurchaseStatus::Cancelled | PurchaseStatus::Disputed => false,
        }
    }

    fn parse_x402_network(
        value: &str,
        entity: &str,
        field: &str,
    ) -> Result<X402Network> {
        X402Network::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} for x402_network: '{}' - {}",
                entity, field, value, e
            ))
        })
    }

    fn parse_x402_asset(
        value: &str,
        entity: &str,
        field: &str,
    ) -> Result<X402Asset> {
        X402Asset::from_str(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} for x402_asset: '{}' - {}",
                entity, field, value, e
            ))
        })
    }

    fn parse_json<T: DeserializeOwned>(
        value: Value,
        entity: &str,
        field: &str,
    ) -> Result<T> {
        serde_json::from_value::<T>(value).map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid {}.{} JSON: {}",
                entity, field, e
            ))
        })
    }

    fn parse_cart_address(
        value: Option<Value>,
        entity: &str,
        field: &str,
    ) -> Result<Option<CartAddress>> {
        match value {
            Some(value) => match value {
                Value::String(value) => {
                    serde_json::from_str::<CartAddress>(&value).map_err(|e| {
                        CommerceError::DatabaseError(format!(
                            "Invalid {}.{} JSON string: {}",
                            entity, field, e
                        ))
                    })
                }
                _ => {
                    let value = Self::parse_json::<CartAddress>(value, entity, field)?;
                    Ok(Some(value))
                }
            }
            .map(Some),
            None => Ok(None),
        }
    }

    fn parse_tracking_info(
        value: Option<Value>,
        entity: &str,
        field: &str,
    ) -> Result<Option<String>> {
        match value {
            Some(value) => match value {
                Value::String(value) => Ok(Some(value)),
                _ => Ok(Some(Self::parse_json::<String>(
                    value,
                    entity,
                    field,
                )?)),
            },
            None => Ok(None),
        }
    }

    fn parse_optional_u8(value: Option<i32>, entity: &str, field: &str) -> Result<Option<u8>> {
        match value {
            Some(value) => {
                if !(0..=5).contains(&value) {
                    return Err(CommerceError::DatabaseError(format!(
                        "Invalid {}.{} '{}': expected 0-5",
                        entity, field, value
                    )));
                }
                Ok(Some(value as u8))
            }
            None => Ok(None),
        }
    }

    fn normalize_currency(raw: Option<String>) -> String {
        raw.unwrap_or_else(|| "USD".to_string())
            .trim()
            .to_ascii_uppercase()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }

    fn row_to_quote(row: QuoteRow) -> Result<SkillQuote> {
        Ok(SkillQuote {
            id: row.id,
            quote_number: row.quote_number,
            status: Self::parse_quote_status(&row.status, "a2a_quote", "status")?,
            buyer_agent_id: row.buyer_agent_id,
            seller_agent_id: row.seller_agent_id,
            items: Self::parse_json::<Vec<QuotedItem>>(row.items, "a2a_quote", "items")?,
            subtotal: row.subtotal,
            tax_amount: row.tax_amount,
            shipping_amount: row.shipping_amount,
            discount_amount: row.discount_amount,
            total: row.total,
            currency: row.currency,
            payment_network: match row.payment_network {
                Some(network) => Some(Self::parse_x402_network(
                    &network,
                    "a2a_quote",
                    "payment_network",
                )?),
                None => None,
            },
            payment_asset: match row.payment_asset {
                Some(asset) => Some(Self::parse_x402_asset(&asset, "a2a_quote", "payment_asset")?),
                None => None,
            },
            shipping_address: Self::parse_cart_address(
                row.shipping_address,
                "a2a_quote",
                "shipping_address",
            )?,
            valid_until: row.valid_until,
            purchase_id: row.purchase_id,
            payment_intent_id: row.payment_intent_id,
            notes: row.notes,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_purchase(row: PurchaseRow) -> Result<A2APurchase> {
        Ok(A2APurchase {
            id: row.id,
            purchase_number: row.purchase_number,
            status: Self::parse_purchase_status(&row.status, "a2a_purchase", "status")?,
            buyer_agent_id: row.buyer_agent_id,
            seller_agent_id: row.seller_agent_id,
            quote_id: row.quote_id,
            cart_id: row.cart_id,
            order_id: row.order_id,
            payment_intent_id: row.payment_intent_id,
            items: Self::parse_json::<Vec<QuotedItem>>(row.items, "a2a_purchase", "items")?,
            total: row.total,
            currency: row.currency,
            fulfillment_type: row.fulfillment_type,
            tracking_info: Self::parse_tracking_info(
                row.tracking_info,
                "a2a_purchase",
                "tracking_info",
            )?,
            delivered_at: row.delivered_at,
            delivery_confirmed_at: row.delivery_confirmed_at,
            delivery_confirmation_signature: row.delivery_confirmation_signature,
            buyer_rating: Self::parse_optional_u8(row.buyer_rating, "a2a_purchase", "buyer_rating")?,
            buyer_feedback: row.buyer_feedback,
            seller_rating: Self::parse_optional_u8(row.seller_rating, "a2a_purchase", "seller_rating")?,
            seller_feedback: row.seller_feedback,
            notes: row.notes,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn validate_quote_input(&self, input: &CreateA2AQuote) -> Result<()> {
        if input.buyer_agent_id.is_nil() {
            return Err(CommerceError::InvalidInput {
                field: "buyer_agent_id".to_string(),
                message: "buyer_agent_id cannot be nil UUID".to_string(),
            });
        }

        if input.seller_agent_id.is_nil() {
            return Err(CommerceError::InvalidInput {
                field: "seller_agent_id".to_string(),
                message: "seller_agent_id cannot be nil UUID".to_string(),
            });
        }

        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a2a quote must include at least one item".to_string(),
            ));
        }

        if input.total < Decimal::ZERO {
            return Err(CommerceError::InvalidInput {
                field: "total".to_string(),
                message: "total must be non-negative".to_string(),
            });
        }

        if input.subtotal < Decimal::ZERO {
            return Err(CommerceError::InvalidInput {
                field: "subtotal".to_string(),
                message: "subtotal must be non-negative".to_string(),
            });
        }

        if input.valid_until <= Utc::now() {
            return Err(CommerceError::ValidationError(
                "valid_until must be in the future".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_purchase_input(&self, input: &CreateA2APurchase) -> Result<()> {
        if input.buyer_agent_id.is_nil() {
            return Err(CommerceError::InvalidInput {
                field: "buyer_agent_id".to_string(),
                message: "buyer_agent_id cannot be nil UUID".to_string(),
            });
        }

        if input.seller_agent_id.is_nil() {
            return Err(CommerceError::InvalidInput {
                field: "seller_agent_id".to_string(),
                message: "seller_agent_id cannot be nil UUID".to_string(),
            });
        }

        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a2a purchase must include at least one item".to_string(),
            ));
        }

        if input.total < Decimal::ZERO {
            return Err(CommerceError::InvalidInput {
                field: "total".to_string(),
                message: "total must be non-negative".to_string(),
            });
        }

        Ok(())
    }

    async fn ensure_quote_for_purchase(
        &self,
        quote_id: Uuid,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
    ) -> Result<Option<QuoteValidationRow>> {
        let row = sqlx::query_as::<_, QuoteValidationRow>(
            r#"
            SELECT
                buyer_agent_id,
                seller_agent_id,
                status,
                total,
                currency,
                valid_until
            FROM a2a_quotes
            WHERE id = $1
            "#,
        )
        .bind(quote_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        match row {
            Some(row) => {
                if row.buyer_agent_id != buyer_agent_id || row.seller_agent_id != seller_agent_id {
                    return Err(CommerceError::ValidationError(
                        "quote participants do not match purchase participants".to_string(),
                    ));
                }
                Ok(Some(row))
            }
            None => Ok(None),
        }
    }

    pub async fn create_quote_async(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        self.validate_quote_input(&input)?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        let quote_number = Self::generate_quote_number();
        let status = QuoteStatus::Pending;
        let currency = Self::normalize_currency(input.currency);
        let items_json = serde_json::to_value(&input.items).map_err(|e| CommerceError::Internal(e.to_string()))?;
        let shipping_json = input
            .shipping_address
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let tax_amount = input.tax_amount.unwrap_or(Decimal::ZERO);
        let shipping_amount = input.shipping_amount.unwrap_or(Decimal::ZERO);
        let discount_amount = input.discount_amount.unwrap_or(Decimal::ZERO);

        sqlx::query(
            r#"
            INSERT INTO a2a_quotes (
                id, quote_number, status, buyer_agent_id, seller_agent_id, items,
                subtotal, tax_amount, shipping_amount, discount_amount, total, currency,
                payment_network, payment_asset, shipping_address, valid_until,
                notes, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16,
                $17, $18, $19, $20
            )
            "#,
        )
        .bind(id)
        .bind(quote_number)
        .bind(status.to_string())
        .bind(input.buyer_agent_id)
        .bind(input.seller_agent_id)
        .bind(items_json)
        .bind(input.subtotal)
        .bind(tax_amount)
        .bind(shipping_amount)
        .bind(discount_amount)
        .bind(input.total)
        .bind(currency)
        .bind(input.payment_network.map(|v| v.to_string()))
        .bind(input.payment_asset.map(|v| v.to_string()))
        .bind(shipping_json)
        .bind(input.valid_until)
        .bind(input.notes)
        .bind(input.metadata)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_quote_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_quote_async(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        let row = sqlx::query_as::<_, QuoteRow>("SELECT * FROM a2a_quotes WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_quote).transpose()
    }

    pub async fn get_quote_by_number_async(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        let row = sqlx::query_as::<_, QuoteRow>("SELECT * FROM a2a_quotes WHERE quote_number = $1")
            .bind(quote_number)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_quote).transpose()
    }

    pub async fn update_quote_status_async(
        &self,
        id: Uuid,
        status: QuoteStatus,
    ) -> Result<SkillQuote> {
        let existing = self.get_quote_async(id).await?.ok_or(CommerceError::NotFound)?;

        if !Self::is_valid_quote_status_transition(existing.status, status) {
            return Err(CommerceError::ValidationError(
                format!("invalid quote status transition: {:?} -> {:?}", existing.status, status),
            ));
        }

        if existing.status == status {
            return Ok(existing);
        }

        sqlx::query("UPDATE a2a_quotes SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_quote_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_quotes_async(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT * FROM a2a_quotes WHERE 1=1");

        if let Some(buyer) = filter.buyer_agent_id {
            qb.push(" AND buyer_agent_id = ");
            qb.push_bind(buyer);
        }
        if let Some(seller) = filter.seller_agent_id {
            qb.push(" AND seller_agent_id = ");
            qb.push_bind(seller);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status.to_string());
        }
        if let Some(from) = filter.from_date {
            qb.push(" AND created_at >= ");
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            qb.push(" AND created_at <= ");
            qb.push_bind(to);
        }

        let limit = filter.limit.unwrap_or(100).min(1000) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows = qb.build_query_as::<QuoteRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_quote).collect()
    }

    pub async fn count_quotes_async(&self, filter: SkillQuoteFilter) -> Result<u64> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM a2a_quotes WHERE 1=1");

        if let Some(buyer) = filter.buyer_agent_id {
            qb.push(" AND buyer_agent_id = ");
            qb.push_bind(buyer);
        }
        if let Some(seller) = filter.seller_agent_id {
            qb.push(" AND seller_agent_id = ");
            qb.push_bind(seller);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status.to_string());
        }
        if let Some(from) = filter.from_date {
            qb.push(" AND created_at >= ");
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            qb.push(" AND created_at <= ");
            qb.push_bind(to);
        }

        let row: (i64,) = qb.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(row.0 as u64)
    }

    pub async fn create_purchase_async(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        self.validate_purchase_input(&input)?;

        let now = Utc::now();
        let now_str = now;
        let id = Uuid::new_v4();
        let purchase_number = Self::generate_purchase_number();
        let quote_id = input.quote_id;

        if let Some(quote_id) = quote_id {
            let quote = self
                .ensure_quote_for_purchase(quote_id, input.buyer_agent_id, input.seller_agent_id)
                .await?;
            let quote = quote.ok_or(CommerceError::NotFound)?;
            let quote_status = Self::parse_quote_status(&quote.status, "a2a_quote", "status")?;
            if !matches!(quote_status, QuoteStatus::Quoted | QuoteStatus::Accepted) {
                return Err(CommerceError::ValidationError(
                    "quote is not available for purchase creation".to_string(),
                ));
            }
            if quote_status == QuoteStatus::Purchased {
                return Err(CommerceError::ValidationError(
                    "quote already has a linked purchase".to_string(),
                ));
            }
            if quote.valid_until <= Utc::now() {
                return Err(CommerceError::ValidationError("quote has expired".to_string()));
            }
            if quote.currency != Self::normalize_currency(input.currency.clone()) {
                return Err(CommerceError::ValidationError(
                    "purchase currency does not match quote currency".to_string(),
                ));
            }
            if quote.total != input.total {
                return Err(CommerceError::ValidationError(
                    "purchase total must match quote total".to_string(),
                ));
            }
        }

        let items_json = serde_json::to_value(&input.items).map_err(|e| CommerceError::Internal(e.to_string()))?;
        let currency = Self::normalize_currency(input.currency);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        sqlx::query(
            r#"
            INSERT INTO a2a_purchases (
                id, purchase_number, status, buyer_agent_id, seller_agent_id, quote_id,
                cart_id, order_id, payment_intent_id, items, total, currency,
                fulfillment_type, notes, metadata, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17
            )
            "#,
        )
        .bind(id)
        .bind(purchase_number)
        .bind(PurchaseStatus::Initiated.to_string())
        .bind(input.buyer_agent_id)
        .bind(input.seller_agent_id)
        .bind(quote_id)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(input.payment_intent_id)
        .bind(items_json)
        .bind(input.total)
        .bind(currency)
        .bind(input.fulfillment_type)
        .bind(input.notes)
        .bind(input.metadata)
        .bind(now_str)
        .bind(now_str)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if let Some(quote_id) = quote_id {
            sqlx::query(
                "UPDATE a2a_quotes SET purchase_id = $1, status = $2, updated_at = $3 WHERE id = $4",
            )
            .bind(id)
            .bind(QuoteStatus::Purchased.to_string())
            .bind(now)
            .bind(quote_id)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;

        self.get_purchase_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_purchase_async(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        let row = sqlx::query_as::<_, PurchaseRow>("SELECT * FROM a2a_purchases WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        row.map(Self::row_to_purchase).transpose()
    }

    pub async fn get_purchase_by_number_async(
        &self,
        purchase_number: &str,
    ) -> Result<Option<A2APurchase>> {
        let row = sqlx::query_as::<_, PurchaseRow>(
            "SELECT * FROM a2a_purchases WHERE purchase_number = $1",
        )
        .bind(purchase_number)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_purchase).transpose()
    }

    pub async fn update_purchase_status_async(
        &self,
        id: Uuid,
        status: PurchaseStatus,
    ) -> Result<A2APurchase> {
        let existing = self.get_purchase_async(id).await?.ok_or(CommerceError::NotFound)?;

        if !Self::is_valid_purchase_status_transition(existing.status, status) {
            return Err(CommerceError::ValidationError(
                format!(
                    "invalid purchase status transition: {:?} -> {:?}",
                    existing.status, status
                ),
            ));
        }

        if existing.status == status {
            return Ok(existing);
        }

        sqlx::query("UPDATE a2a_purchases SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status.to_string())
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_purchase_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn link_purchase_to_order_async(
        &self,
        purchase_id: Uuid,
        order_id: Uuid,
    ) -> Result<A2APurchase> {
        sqlx::query("UPDATE a2a_purchases SET order_id = $1, updated_at = $2 WHERE id = $3")
            .bind(order_id)
            .bind(Utc::now())
            .bind(purchase_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_purchase_async(purchase_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn confirm_delivery_async(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase> {
        if signature.trim().is_empty() {
            return Err(CommerceError::InvalidInput {
                field: "signature".to_string(),
                message: "signature cannot be empty".to_string(),
            });
        }

        let existing = self.get_purchase_async(purchase_id).await?.ok_or(CommerceError::NotFound)?;
        if matches!(
            existing.status,
            PurchaseStatus::Cancelled | PurchaseStatus::Disputed
        ) {
            return Err(CommerceError::ValidationError(
                "cannot confirm delivery for cancelled purchase".to_string(),
            ));
        }
        if !matches!(
            existing.status,
            PurchaseStatus::Shipped | PurchaseStatus::Delivered | PurchaseStatus::Completed
        ) {
            return Err(CommerceError::ValidationError(
                "purchase must be shipped before confirming delivery".to_string(),
            ));
        }

        let now = Utc::now();
        let delivered_at = existing.delivered_at.unwrap_or(now);
        let rating = rating.map(|value| value as i16);

        sqlx::query(
            "UPDATE a2a_purchases
                 SET status = $1, delivered_at = $2, delivery_confirmed_at = $3, delivery_confirmation_signature = $4,
                     buyer_rating = COALESCE($5, buyer_rating), buyer_feedback = COALESCE($6, buyer_feedback), updated_at = $7
                 WHERE id = $8",
        )
        .bind(PurchaseStatus::Completed.to_string())
        .bind(delivered_at)
        .bind(now)
        .bind(signature)
        .bind(rating)
        .bind(feedback)
        .bind(now)
        .bind(purchase_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_purchase_async(purchase_id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_purchases_async(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT * FROM a2a_purchases WHERE 1=1");

        if let Some(buyer) = filter.buyer_agent_id {
            qb.push(" AND buyer_agent_id = ");
            qb.push_bind(buyer);
        }
        if let Some(seller) = filter.seller_agent_id {
            qb.push(" AND seller_agent_id = ");
            qb.push_bind(seller);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status.to_string());
        }
        if let Some(order_id) = filter.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }
        if let Some(from) = filter.from_date {
            qb.push(" AND created_at >= ");
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            qb.push(" AND created_at <= ");
            qb.push_bind(to);
        }

        let limit = filter.limit.unwrap_or(100).min(1000) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        qb.push(" ORDER BY created_at DESC LIMIT ");
        qb.push_bind(limit);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        let rows = qb
            .build_query_as::<PurchaseRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_purchase).collect()
    }

    pub async fn count_purchases_async(&self, filter: A2APurchaseFilter) -> Result<u64> {
        let mut qb = QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM a2a_purchases WHERE 1=1");

        if let Some(buyer) = filter.buyer_agent_id {
            qb.push(" AND buyer_agent_id = ");
            qb.push_bind(buyer);
        }
        if let Some(seller) = filter.seller_agent_id {
            qb.push(" AND seller_agent_id = ");
            qb.push_bind(seller);
        }
        if let Some(status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status.to_string());
        }
        if let Some(order_id) = filter.order_id {
            qb.push(" AND order_id = ");
            qb.push_bind(order_id);
        }
        if let Some(from) = filter.from_date {
            qb.push(" AND created_at >= ");
            qb.push_bind(from);
        }
        if let Some(to) = filter.to_date {
            qb.push(" AND created_at <= ");
            qb.push_bind(to);
        }

        let row: (i64,) = qb.build_query_as().fetch_one(&self.pool).await.map_err(map_db_error)?;
        Ok(row.0 as u64)
    }
}

impl A2ACommerceRepository for PgA2ARepository {
    fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        block_on(self.create_quote_async(input))
    }

    fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        block_on(self.get_quote_async(id))
    }

    fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        block_on(self.get_quote_by_number_async(quote_number))
    }

    fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote> {
        block_on(self.update_quote_status_async(id, status))
    }

    fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        block_on(self.list_quotes_async(filter))
    }

    fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64> {
        block_on(self.count_quotes_async(filter))
    }

    fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        block_on(self.create_purchase_async(input))
    }

    fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        block_on(self.get_purchase_async(id))
    }

    fn get_purchase_by_number(&self, purchase_number: &str) -> Result<Option<A2APurchase>> {
        block_on(self.get_purchase_by_number_async(purchase_number))
    }

    fn update_purchase_status(&self, id: Uuid, status: PurchaseStatus) -> Result<A2APurchase> {
        block_on(self.update_purchase_status_async(id, status))
    }

    fn link_purchase_to_order(&self, purchase_id: Uuid, order_id: Uuid) -> Result<A2APurchase> {
        block_on(self.link_purchase_to_order_async(purchase_id, order_id))
    }

    fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase> {
        block_on(self.confirm_delivery_async(purchase_id, signature, rating, feedback))
    }

    fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        block_on(self.list_purchases_async(filter))
    }

    fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64> {
        block_on(self.count_purchases_async(filter))
    }
}
