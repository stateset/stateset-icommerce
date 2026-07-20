//! PostgreSQL vendor return (return-to-supplier) repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateVendorReturn, CurrencyCode, Result, VendorReturn, VendorReturnFilter,
    VendorReturnId, VendorReturnItem, VendorReturnItemId, VendorReturnReason,
    VendorReturnRepository, VendorReturnStatus,
};
use uuid::Uuid;

/// PostgreSQL implementation of `VendorReturnRepository`
#[derive(Debug, Clone)]
pub struct PgVendorReturnRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct VendorReturnRow {
    id: Uuid,
    number: String,
    supplier_id: Uuid,
    purchase_order_id: Option<Uuid>,
    status: String,
    currency: String,
    credit_generated: bool,
    notes: Option<String>,
    processed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct VendorReturnItemRow {
    id: Uuid,
    vendor_return_id: Uuid,
    product_id: Uuid,
    sku: String,
    quantity: Decimal,
    unit_cost: Decimal,
    reason: String,
}

impl PgVendorReturnRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_field<T: std::str::FromStr>(raw: &str, field: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid vendor_return.{field} '{raw}': {e}"))
        })
    }

    fn row_to_head(row: VendorReturnRow) -> Result<VendorReturn> {
        Ok(VendorReturn {
            id: row.id.into(),
            number: row.number,
            supplier_id: row.supplier_id,
            purchase_order_id: row.purchase_order_id,
            status: Self::parse_field::<VendorReturnStatus>(&row.status, "status")?,
            currency: Self::parse_field::<CurrencyCode>(&row.currency, "currency")?,
            items: Vec::new(),
            credit_generated: row.credit_generated,
            notes: row.notes,
            processed_at: row.processed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_item(row: VendorReturnItemRow) -> Result<VendorReturnItem> {
        Ok(VendorReturnItem {
            id: row.id.into(),
            vendor_return_id: row.vendor_return_id.into(),
            product_id: row.product_id.into(),
            sku: row.sku,
            quantity: row.quantity,
            unit_cost: row.unit_cost,
            reason: Self::parse_field::<VendorReturnReason>(&row.reason, "items.reason")?,
        })
    }

    async fn load_items_async(&self, id: Uuid) -> Result<Vec<VendorReturnItem>> {
        let rows = sqlx::query_as::<_, VendorReturnItemRow>(
            "SELECT * FROM vendor_return_items WHERE vendor_return_id = $1 ORDER BY sku",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_item).collect()
    }

    async fn load_full_async(&self, id: Uuid) -> Result<Option<VendorReturn>> {
        let row =
            sqlx::query_as::<_, VendorReturnRow>("SELECT * FROM vendor_returns WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        match row {
            Some(row) => {
                let mut head = Self::row_to_head(row)?;
                head.items = self.load_items_async(id).await?;
                Ok(Some(head))
            }
            None => Ok(None),
        }
    }

    /// Lock the row and return its current status, or `NotFound`.
    async fn current_status_locked(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<VendorReturnStatus> {
        let status: Option<(String,)> =
            sqlx::query_as("SELECT status FROM vendor_returns WHERE id = $1 FOR UPDATE")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let (status,) = status.ok_or(CommerceError::NotFound)?;
        Self::parse_field::<VendorReturnStatus>(&status, "status")
    }

    /// Create a vendor return (async)
    pub async fn create_async(&self, input: CreateVendorReturn) -> Result<VendorReturn> {
        if input.items.is_empty() {
            return Err(CommerceError::ValidationError(
                "a vendor return requires at least one item".into(),
            ));
        }
        let id = VendorReturnId::new();
        let id_uuid = Uuid::from(id);
        let now = Utc::now();
        let number = format!("VR-{}", &id_uuid.to_string()[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        sqlx::query(
            "INSERT INTO vendor_returns (id, number, supplier_id, purchase_order_id, status, currency, credit_generated, notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'draft', $5, FALSE, $6, $7, $7)",
        )
        .bind(id_uuid)
        .bind(&number)
        .bind(input.supplier_id)
        .bind(input.purchase_order_id)
        .bind(currency.to_string())
        .bind(&input.notes)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for item in &input.items {
            sqlx::query(
                "INSERT INTO vendor_return_items (id, vendor_return_id, product_id, sku, quantity, unit_cost, reason)
                 VALUES ($1, $2, $3, '', $4, $5, $6)",
            )
            .bind(Uuid::from(VendorReturnItemId::new()))
            .bind(id_uuid)
            .bind(Uuid::from(item.product_id))
            .bind(item.quantity)
            .bind(item.unit_cost)
            .bind(item.reason.to_string())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;

        self.load_full_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a vendor return with items (async)
    pub async fn get_async(&self, id: VendorReturnId) -> Result<Option<VendorReturn>> {
        self.load_full_async(id.into()).await
    }

    /// List vendor returns (async)
    pub async fn list_async(&self, filter: VendorReturnFilter) -> Result<Vec<VendorReturn>> {
        let limit = i64::from(filter.limit.unwrap_or(100));
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM vendor_returns WHERE 1=1");
        let mut param_idx = 1;
        if filter.supplier_id.is_some() {
            query.push_str(&format!(" AND supplier_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, VendorReturnRow>(&query);
        if let Some(supplier_id) = filter.supplier_id {
            q = q.bind(supplier_id);
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row.id;
            let mut head = Self::row_to_head(row)?;
            head.items = self.load_items_async(id).await?;
            out.push(head);
        }
        Ok(out)
    }

    /// Submit a draft vendor return (async)
    pub async fn submit_async(&self, id: VendorReturnId) -> Result<VendorReturn> {
        let id_uuid = Uuid::from(id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        if Self::current_status_locked(&mut tx, id_uuid).await? != VendorReturnStatus::Draft {
            return Err(CommerceError::Conflict(
                "only draft vendor returns can be submitted".into(),
            ));
        }
        sqlx::query("UPDATE vendor_returns SET status = 'pending', updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id_uuid)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.load_full_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// Process a vendor return, optionally flagging credit generation (async)
    pub async fn process_async(
        &self,
        id: VendorReturnId,
        generate_credit: bool,
    ) -> Result<VendorReturn> {
        let id_uuid = Uuid::from(id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let status = Self::current_status_locked(&mut tx, id_uuid).await?;
        if status.is_terminal() {
            return Err(CommerceError::Conflict(
                "vendor return is already in a terminal state".into(),
            ));
        }
        let now = Utc::now();
        sqlx::query(
            "UPDATE vendor_returns SET status = 'processed', credit_generated = $1, processed_at = $2, updated_at = $2 WHERE id = $3",
        )
        .bind(generate_credit)
        .bind(now)
        .bind(id_uuid)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.load_full_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }

    /// Cancel a vendor return (async)
    pub async fn cancel_async(&self, id: VendorReturnId) -> Result<VendorReturn> {
        let id_uuid = Uuid::from(id);
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        if Self::current_status_locked(&mut tx, id_uuid).await? == VendorReturnStatus::Processed {
            return Err(CommerceError::Conflict(
                "processed vendor returns cannot be cancelled".into(),
            ));
        }
        sqlx::query(
            "UPDATE vendor_returns SET status = 'cancelled', updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id_uuid)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.load_full_async(id_uuid).await?.ok_or(CommerceError::NotFound)
    }
}

impl VendorReturnRepository for PgVendorReturnRepository {
    fn create(&self, input: CreateVendorReturn) -> Result<VendorReturn> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: VendorReturnId) -> Result<Option<VendorReturn>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: VendorReturnFilter) -> Result<Vec<VendorReturn>> {
        super::block_on(self.list_async(filter))
    }

    fn submit(&self, id: VendorReturnId) -> Result<VendorReturn> {
        super::block_on(self.submit_async(id))
    }

    fn process(&self, id: VendorReturnId, generate_credit: bool) -> Result<VendorReturn> {
        super::block_on(self.process_async(id, generate_credit))
    }

    fn cancel(&self, id: VendorReturnId) -> Result<VendorReturn> {
        super::block_on(self.cancel_async(id))
    }
}
