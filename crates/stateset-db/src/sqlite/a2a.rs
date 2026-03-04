//! SQLite Agent-to-Agent commerce repository implementation

use super::{
    map_db_error, params_refs, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row,
    parse_json_opt_row, parse_json_row, parse_uuid_opt_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, ToSql};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde_json::Value;
use stateset_core::{
    A2ACommerceRepository, A2APurchase, A2APurchaseFilter, CommerceError, CreateA2APurchase,
    CreateA2AQuote, CurrencyCode, PurchaseStatus, QuoteStatus, Result, SkillQuote,
    SkillQuoteFilter, X402Asset, X402Network,
};
use uuid::Uuid;

/// SQLite implementation of `A2ACommerceRepository`
struct QuoteValidationRow {
    buyer_agent_id: Uuid,
    seller_agent_id: Uuid,
    status: QuoteStatus,
    total: Decimal,
    currency: String,
    valid_until: chrono::DateTime<Utc>,
}

#[derive(Debug)]
pub struct SqliteA2ARepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteA2ARepository {
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn generate_quote_number() -> String {
        let now = Utc::now();
        let random = Uuid::new_v4();
        format!("QTE-{}-{:08X}", now.timestamp_millis(), (random.as_u128() & 0xFFFFFFFF) as u32,)
    }

    fn generate_purchase_number() -> String {
        let now = Utc::now();
        let random = Uuid::new_v4();
        format!("PUR-{}-{:08X}", now.timestamp_millis(), (random.as_u128() & 0xFFFFFFFF) as u32,)
    }

    fn parse_quote_status(value: &str, entity: &str, field: &str) -> rusqlite::Result<QuoteStatus> {
        match value {
            "pending" => Ok(QuoteStatus::Pending),
            "quoted" => Ok(QuoteStatus::Quoted),
            "accepted" => Ok(QuoteStatus::Accepted),
            "rejected" => Ok(QuoteStatus::Rejected),
            "expired" => Ok(QuoteStatus::Expired),
            "purchased" => Ok(QuoteStatus::Purchased),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid {}.{} for quote_status: '{}'", entity, field, value),
                )),
            )),
        }
    }

    fn parse_purchase_status(
        value: &str,
        entity: &str,
        field: &str,
    ) -> rusqlite::Result<PurchaseStatus> {
        match value {
            "initiated" => Ok(PurchaseStatus::Initiated),
            "payment_pending" => Ok(PurchaseStatus::PaymentPending),
            "paid" => Ok(PurchaseStatus::Paid),
            "fulfilling" => Ok(PurchaseStatus::Fulfilling),
            "shipped" => Ok(PurchaseStatus::Shipped),
            "delivered" => Ok(PurchaseStatus::Delivered),
            "completed" => Ok(PurchaseStatus::Completed),
            "cancelled" => Ok(PurchaseStatus::Cancelled),
            "disputed" => Ok(PurchaseStatus::Disputed),
            _ => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid {}.{} for purchase_status: '{}'", entity, field, value),
                )),
            )),
        }
    }

    fn parse_x402_network(value: &str, entity: &str, field: &str) -> rusqlite::Result<X402Network> {
        value.parse::<X402Network>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid {}.{} for x402_network: '{}' - {}", entity, field, value, e),
                )),
            )
        })
    }

    fn parse_x402_asset(value: &str, entity: &str, field: &str) -> rusqlite::Result<X402Asset> {
        value.parse::<X402Asset>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid {}.{} for x402_asset: '{}' - {}", entity, field, value, e),
                )),
            )
        })
    }

    fn parse_optional_u8(
        value: Option<i64>,
        entity: &str,
        field: &str,
    ) -> rusqlite::Result<Option<u8>> {
        match value {
            Some(rating) => {
                if !(0..=5).contains(&rating) {
                    return Err(rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Integer,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Invalid {}.{} '{}': expected 0-5", entity, field, rating),
                        )),
                    ));
                }
                Ok(Some(rating as u8))
            }
            None => Ok(None),
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
            _ => false,
        }
    }

    fn is_valid_purchase_status_transition(current: PurchaseStatus, next: PurchaseStatus) -> bool {
        if current == next {
            return true;
        }

        match current {
            PurchaseStatus::Initiated => {
                matches!(
                    next,
                    PurchaseStatus::PaymentPending
                        | PurchaseStatus::Cancelled
                        | PurchaseStatus::Disputed
                )
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
                matches!(
                    next,
                    PurchaseStatus::Shipped | PurchaseStatus::Cancelled | PurchaseStatus::Disputed
                )
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
                matches!(
                    next,
                    PurchaseStatus::Completed
                        | PurchaseStatus::Cancelled
                        | PurchaseStatus::Disputed
                )
            }
            PurchaseStatus::Completed | PurchaseStatus::Cancelled | PurchaseStatus::Disputed => {
                false
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn parse_json<T: DeserializeOwned>(
        value: Value,
        entity: &str,
        field: &str,
    ) -> rusqlite::Result<T> {
        serde_json::from_value(value).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid {}.{} JSON: {}", entity, field, e),
                )),
            )
        })
    }

    fn parse_tracking_info(
        value: Option<String>,
        _entity: &str,
        _field: &str,
    ) -> rusqlite::Result<Option<String>> {
        match value {
            Some(value) => Ok(Some(value)),
            None => Ok(None),
        }
    }

    fn row_to_quote(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillQuote> {
        Ok(SkillQuote {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "a2a_quote", "id")?,
            quote_number: row.get("quote_number")?,
            status: Self::parse_quote_status(
                &row.get::<_, String>("status")?,
                "a2a_quote",
                "status",
            )?,
            buyer_agent_id: parse_uuid_row(
                &row.get::<_, String>("buyer_agent_id")?,
                "a2a_quote",
                "buyer_agent_id",
            )?,
            seller_agent_id: parse_uuid_row(
                &row.get::<_, String>("seller_agent_id")?,
                "a2a_quote",
                "seller_agent_id",
            )?,
            items: parse_json_row(&row.get::<_, String>("items")?, "a2a_quote", "items")?,
            subtotal: parse_decimal_row(
                &row.get::<_, String>("subtotal")?,
                "a2a_quote",
                "subtotal",
            )?,
            tax_amount: parse_decimal_row(
                &row.get::<_, String>("tax_amount")?,
                "a2a_quote",
                "tax_amount",
            )?,
            shipping_amount: parse_decimal_row(
                &row.get::<_, String>("shipping_amount")?,
                "a2a_quote",
                "shipping_amount",
            )?,
            discount_amount: parse_decimal_row(
                &row.get::<_, String>("discount_amount")?,
                "a2a_quote",
                "discount_amount",
            )?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "a2a_quote", "total")?,
            currency: row.get("currency")?,
            payment_network: match row.get::<_, Option<String>>("payment_network")? {
                Some(value) => {
                    Some(Self::parse_x402_network(&value, "a2a_quote", "payment_network")?)
                }
                None => None,
            },
            payment_asset: match row.get::<_, Option<String>>("payment_asset")? {
                Some(value) => Some(Self::parse_x402_asset(&value, "a2a_quote", "payment_asset")?),
                None => None,
            },
            shipping_address: parse_json_opt_row(
                row.get::<_, Option<String>>("shipping_address")?,
                "a2a_quote",
                "shipping_address",
            )?,
            valid_until: parse_datetime_row(
                &row.get::<_, String>("valid_until")?,
                "a2a_quote",
                "valid_until",
            )?,
            purchase_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_id")?,
                "a2a_quote",
                "purchase_id",
            )?,
            payment_intent_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("payment_intent_id")?,
                "a2a_quote",
                "payment_intent_id",
            )?,
            notes: row.get("notes")?,
            metadata: row.get("metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "a2a_quote",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "a2a_quote",
                "updated_at",
            )?,
        })
    }

    fn row_to_purchase(row: &rusqlite::Row<'_>) -> rusqlite::Result<A2APurchase> {
        Ok(A2APurchase {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "a2a_purchase", "id")?,
            purchase_number: row.get("purchase_number")?,
            status: Self::parse_purchase_status(
                &row.get::<_, String>("status")?,
                "a2a_purchase",
                "status",
            )?,
            buyer_agent_id: parse_uuid_row(
                &row.get::<_, String>("buyer_agent_id")?,
                "a2a_purchase",
                "buyer_agent_id",
            )?,
            seller_agent_id: parse_uuid_row(
                &row.get::<_, String>("seller_agent_id")?,
                "a2a_purchase",
                "seller_agent_id",
            )?,
            quote_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("quote_id")?,
                "a2a_purchase",
                "quote_id",
            )?,
            cart_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("cart_id")?,
                "a2a_purchase",
                "cart_id",
            )?,
            order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("order_id")?,
                "a2a_purchase",
                "order_id",
            )?,
            payment_intent_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("payment_intent_id")?,
                "a2a_purchase",
                "payment_intent_id",
            )?,
            items: parse_json_row(&row.get::<_, String>("items")?, "a2a_purchase", "items")?,
            total: parse_decimal_row(&row.get::<_, String>("total")?, "a2a_purchase", "total")?,
            currency: row.get("currency")?,
            fulfillment_type: row.get("fulfillment_type")?,
            tracking_info: Self::parse_tracking_info(
                row.get::<_, Option<String>>("tracking_info")?,
                "a2a_purchase",
                "tracking_info",
            )?,
            delivered_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("delivered_at")?,
                "a2a_purchase",
                "delivered_at",
            )?,
            delivery_confirmed_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("delivery_confirmed_at")?,
                "a2a_purchase",
                "delivery_confirmed_at",
            )?,
            delivery_confirmation_signature: row.get("delivery_confirmation_signature")?,
            buyer_rating: Self::parse_optional_u8(
                row.get::<_, Option<i64>>("buyer_rating")?,
                "a2a_purchase",
                "buyer_rating",
            )?,
            buyer_feedback: row.get("buyer_feedback")?,
            seller_rating: Self::parse_optional_u8(
                row.get::<_, Option<i64>>("seller_rating")?,
                "a2a_purchase",
                "seller_rating",
            )?,
            seller_feedback: row.get("seller_feedback")?,
            notes: row.get("notes")?,
            metadata: row.get("metadata")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "a2a_purchase",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "a2a_purchase",
                "updated_at",
            )?,
        })
    }

    fn parse_quote_filters(filter: &SkillQuoteFilter) -> (Vec<String>, Vec<Box<dyn ToSql>>) {
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn ToSql>> = vec![];

        if let Some(buyer) = filter.buyer_agent_id {
            conditions.push("buyer_agent_id = ?".to_string());
            params.push(Box::new(buyer.to_string()));
        }
        if let Some(seller) = filter.seller_agent_id {
            conditions.push("seller_agent_id = ?".to_string());
            params.push(Box::new(seller.to_string()));
        }
        if let Some(status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref from) = filter.from_date {
            conditions.push("created_at >= ?".to_string());
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(ref to) = filter.to_date {
            conditions.push("created_at <= ?".to_string());
            params.push(Box::new(to.to_rfc3339()));
        }

        (conditions, params)
    }

    fn parse_purchase_filters(filter: &A2APurchaseFilter) -> (Vec<String>, Vec<Box<dyn ToSql>>) {
        let mut conditions = vec!["1=1".to_string()];
        let mut params: Vec<Box<dyn ToSql>> = vec![];

        if let Some(buyer) = filter.buyer_agent_id {
            conditions.push("buyer_agent_id = ?".to_string());
            params.push(Box::new(buyer.to_string()));
        }
        if let Some(seller) = filter.seller_agent_id {
            conditions.push("seller_agent_id = ?".to_string());
            params.push(Box::new(seller.to_string()));
        }
        if let Some(status) = filter.status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(status.to_string()));
        }
        if let Some(order) = filter.order_id {
            conditions.push("order_id = ?".to_string());
            params.push(Box::new(order.to_string()));
        }
        if let Some(ref from) = filter.from_date {
            conditions.push("created_at >= ?".to_string());
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(ref to) = filter.to_date {
            conditions.push("created_at <= ?".to_string());
            params.push(Box::new(to.to_rfc3339()));
        }

        (conditions, params)
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

    fn normalize_currency(raw: Option<CurrencyCode>) -> CurrencyCode {
        raw.unwrap_or_default()
    }

    fn ensure_quote_for_purchase(
        &self,
        conn: &rusqlite::Connection,
        quote_id: Uuid,
        buyer_agent_id: Uuid,
        seller_agent_id: Uuid,
    ) -> Result<Option<QuoteValidationRow>> {
        let quote = conn
            .query_row(
                "SELECT buyer_agent_id, seller_agent_id, status, total, currency, valid_until FROM a2a_quotes WHERE id = ?",
                [&quote_id.to_string()],
                |row| {
                    Ok(QuoteValidationRow {
                        buyer_agent_id: parse_uuid_row(
                            &row.get::<_, String>(0)?,
                            "a2a_quote",
                            "buyer_agent_id",
                        )?,
                        seller_agent_id: parse_uuid_row(
                            &row.get::<_, String>(1)?,
                            "a2a_quote",
                            "seller_agent_id",
                        )?,
                        status: Self::parse_quote_status(
                            &row.get::<_, String>(2)?,
                            "a2a_quote",
                            "status",
                        )?,
                        total: parse_decimal_row(
                            &row.get::<_, String>(3)?,
                            "a2a_quote",
                            "total",
                        )?,
                        currency: row.get::<_, String>(4)?,
                        valid_until: parse_datetime_row(
                            &row.get::<_, String>(5)?,
                            "a2a_quote",
                            "valid_until",
                        )?,
                    })
                },
            )
            .optional()
            .map_err(map_db_error)?;

        match quote {
            Some(quote) => {
                if quote.buyer_agent_id != buyer_agent_id
                    || quote.seller_agent_id != seller_agent_id
                {
                    return Err(CommerceError::ValidationError(
                        "quote participants do not match purchase participants".to_string(),
                    ));
                }
                Ok(Some(quote))
            }
            None => Ok(None),
        }
    }
}

impl A2ACommerceRepository for SqliteA2ARepository {
    fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        self.validate_quote_input(&input)?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        let quote_number = Self::generate_quote_number();
        let status = QuoteStatus::Pending;
        let currency = Self::normalize_currency(input.currency);

        let items_json = serde_json::to_string(&input.items)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let shipping_json = input
            .shipping_address
            .as_ref()
            .map(|address| {
                serde_json::to_string(address).map_err(|e| CommerceError::Internal(e.to_string()))
            })
            .transpose()?;
        let subtotal = input.subtotal;
        let tax_amount = input.tax_amount.unwrap_or(Decimal::ZERO);
        let shipping_amount = input.shipping_amount.unwrap_or(Decimal::ZERO);
        let discount_amount = input.discount_amount.unwrap_or(Decimal::ZERO);
        let now_str = now.to_rfc3339();

        let conn = self.conn()?;
        conn.execute(
            r#"
            INSERT INTO a2a_quotes (
                id, quote_number, status, buyer_agent_id, seller_agent_id, items,
                subtotal, tax_amount, shipping_amount, discount_amount, total, currency,
                payment_network, payment_asset, shipping_address, valid_until,
                notes, metadata, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20
            )
            "#,
            rusqlite::params![
                id.to_string(),
                quote_number,
                status.to_string(),
                input.buyer_agent_id.to_string(),
                input.seller_agent_id.to_string(),
                items_json,
                subtotal.to_string(),
                tax_amount.to_string(),
                shipping_amount.to_string(),
                discount_amount.to_string(),
                input.total.to_string(),
                currency,
                input.payment_network.map(|v| v.to_string()),
                input.payment_asset.map(|v| v.to_string()),
                shipping_json,
                input.valid_until.to_rfc3339(),
                input.notes,
                input.metadata,
                now_str,
                now_str,
            ],
        )
        .map_err(map_db_error)?;

        self.get_quote(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM a2a_quotes WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_quote).optional().map_err(map_db_error)
    }

    fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM a2a_quotes WHERE quote_number = ?")
            .map_err(map_db_error)?;

        stmt.query_row([quote_number], Self::row_to_quote).optional().map_err(map_db_error)
    }

    fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote> {
        let existing = self.get_quote(id)?.ok_or(CommerceError::NotFound)?;

        if !Self::is_valid_quote_status_transition(existing.status, status) {
            return Err(CommerceError::ValidationError(format!(
                "invalid quote status transition: {:?} -> {:?}",
                existing.status, status
            )));
        }

        if existing.status == status {
            return Ok(existing);
        }

        let affected = self
            .conn()?
            .execute(
                "UPDATE a2a_quotes SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), Utc::now().to_rfc3339(), id.to_string(),],
            )
            .map_err(map_db_error)?;

        if affected == 0 {
            return Err(CommerceError::NotFound);
        }

        self.get_quote(id)?.ok_or(CommerceError::NotFound)
    }

    fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        let conn = self.conn()?;
        let (conditions, mut params) = Self::parse_quote_filters(&filter);
        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let mut sql = format!(
            "SELECT * FROM a2a_quotes WHERE {} ORDER BY created_at DESC",
            conditions.join(" AND ")
        );
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_refs(&params)), Self::row_to_quote)
            .map_err(map_db_error)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(map_db_error)?);
        }
        Ok(result)
    }

    fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64> {
        let conn = self.conn()?;
        let (conditions, params) = Self::parse_quote_filters(&filter);
        let sql = format!("SELECT COUNT(*) FROM a2a_quotes WHERE {}", conditions.join(" AND "));
        let param_refs = params_refs(&params);

        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }

    fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        self.validate_purchase_input(&input)?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let id = Uuid::new_v4();
        let purchase_number = Self::generate_purchase_number();

        if let Some(quote_id) = input.quote_id {
            let conn = self.conn()?;
            let quote = self.ensure_quote_for_purchase(
                &conn,
                quote_id,
                input.buyer_agent_id,
                input.seller_agent_id,
            )?;
            let quote = quote.ok_or(CommerceError::NotFound)?;
            if !matches!(quote.status, QuoteStatus::Quoted | QuoteStatus::Accepted) {
                return Err(CommerceError::ValidationError(
                    "quote is not available for purchase creation".to_string(),
                ));
            }
            if quote.status == QuoteStatus::Purchased {
                return Err(CommerceError::ValidationError(
                    "quote already has a linked purchase".to_string(),
                ));
            }
            if quote.valid_until <= Utc::now() {
                return Err(CommerceError::ValidationError("quote has expired".to_string()));
            }
            if quote.currency != Self::normalize_currency(input.currency).as_str() {
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

        let items_json = serde_json::to_string(&input.items)
            .map_err(|e| CommerceError::Internal(e.to_string()))?;
        let currency = Self::normalize_currency(input.currency);

        let quote_id = input.quote_id;
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                r#"
            INSERT INTO a2a_purchases (
                id, purchase_number, status, buyer_agent_id, seller_agent_id, quote_id,
                cart_id, order_id, payment_intent_id, items, total, currency,
                fulfillment_type, notes, metadata, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17
            )
            "#,
                rusqlite::params![
                    id.to_string(),
                    purchase_number,
                    PurchaseStatus::Initiated.to_string(),
                    input.buyer_agent_id.to_string(),
                    input.seller_agent_id.to_string(),
                    quote_id.map(|v| v.to_string()),
                    None::<String>,
                    None::<String>,
                    input.payment_intent_id.map(|v| v.to_string()),
                    items_json,
                    input.total.to_string(),
                    currency,
                    input.fulfillment_type,
                    input.notes,
                    input.metadata,
                    now_str,
                    now_str,
                ],
            )?;

            if let Some(quote_id) = quote_id {
                tx.execute(
                    "UPDATE a2a_quotes SET purchase_id = ?, status = ?, updated_at = ? WHERE id = ?",
                    rusqlite::params![
                        id.to_string(),
                        QuoteStatus::Purchased.to_string(),
                        now_str,
                        quote_id.to_string(),
                    ],
                )?;
            }

            Ok(())
        })?;

        self.get_purchase(id).ok().flatten().ok_or(CommerceError::NotFound)
    }

    fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM a2a_purchases WHERE id = ?").map_err(map_db_error)?;
        stmt.query_row([id.to_string()], Self::row_to_purchase).optional().map_err(map_db_error)
    }

    fn get_purchase_by_number(&self, purchase_number: &str) -> Result<Option<A2APurchase>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM a2a_purchases WHERE purchase_number = ?")
            .map_err(map_db_error)?;
        stmt.query_row([purchase_number], Self::row_to_purchase).optional().map_err(map_db_error)
    }

    fn update_purchase_status(&self, id: Uuid, status: PurchaseStatus) -> Result<A2APurchase> {
        let existing = self.get_purchase(id)?.ok_or(CommerceError::NotFound)?;

        if !Self::is_valid_purchase_status_transition(existing.status, status) {
            return Err(CommerceError::ValidationError(format!(
                "invalid purchase status transition: {:?} -> {:?}",
                existing.status, status
            )));
        }

        if existing.status == status {
            return Ok(existing);
        }

        let affected = self
            .conn()?
            .execute(
                "UPDATE a2a_purchases SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), Utc::now().to_rfc3339(), id.to_string(),],
            )
            .map_err(map_db_error)?;

        if affected == 0 {
            return Err(CommerceError::NotFound);
        }

        self.get_purchase(id)?.ok_or(CommerceError::NotFound)
    }

    fn link_purchase_to_order(&self, purchase_id: Uuid, order_id: Uuid) -> Result<A2APurchase> {
        let affected = self
            .conn()?
            .execute(
                "UPDATE a2a_purchases SET order_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    order_id.to_string(),
                    Utc::now().to_rfc3339(),
                    purchase_id.to_string()
                ],
            )
            .map_err(map_db_error)?;

        if affected == 0 {
            return Err(CommerceError::NotFound);
        }

        self.get_purchase(purchase_id)?.ok_or(CommerceError::NotFound)
    }

    fn confirm_delivery(
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

        let existing = self.get_purchase(purchase_id)?.ok_or(CommerceError::NotFound)?;
        if matches!(existing.status, PurchaseStatus::Cancelled | PurchaseStatus::Disputed) {
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
        let now_str = now.to_rfc3339();
        let delivered_at = existing.delivered_at.unwrap_or(now);

        let conn = self.conn()?;
        let update_result = conn
            .execute(
                "UPDATE a2a_purchases
                 SET status = ?, delivered_at = ?, delivery_confirmed_at = ?, delivery_confirmation_signature = ?,
                     buyer_rating = COALESCE(?, buyer_rating), buyer_feedback = COALESCE(?, buyer_feedback), updated_at = ?
                 WHERE id = ?",
                rusqlite::params![
                    PurchaseStatus::Completed.to_string(),
                    delivered_at.to_rfc3339(),
                    now_str,
                    signature,
                    rating.map(i64::from),
                    feedback,
                    now_str,
                    purchase_id.to_string(),
                ],
            )
            .map_err(map_db_error)?;

        if update_result == 0 {
            return Err(CommerceError::NotFound);
        }

        self.get_purchase(purchase_id)?.ok_or(CommerceError::NotFound)
    }

    fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        let conn = self.conn()?;
        let (conditions, mut params) = Self::parse_purchase_filters(&filter);
        let limit = filter.limit.unwrap_or(100).min(1000);
        let offset = filter.offset.unwrap_or(0);

        let mut sql = format!(
            "SELECT * FROM a2a_purchases WHERE {} ORDER BY created_at DESC",
            conditions.join(" AND ")
        );
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(limit as i64));
        params.push(Box::new(offset as i64));

        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_refs(&params)), Self::row_to_purchase)
            .map_err(map_db_error)?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(map_db_error)?);
        }
        Ok(result)
    }

    fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64> {
        let conn = self.conn()?;
        let (conditions, params) = Self::parse_purchase_filters(&filter);
        let sql = format!("SELECT COUNT(*) FROM a2a_purchases WHERE {}", conditions.join(" AND "));

        let count: i64 = conn
            .query_row(&sql, rusqlite::params_from_iter(params_refs(&params)), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }
}
