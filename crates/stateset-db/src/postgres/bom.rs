//! PostgreSQL BOM repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    BatchResult, BillOfMaterials, BomComponent, BomFilter, BomRepository, BomStatus, CommerceError,
    CreateBom, CreateBomComponent, Result, UpdateBom, validate_batch_size,
};
use uuid::Uuid;

/// PostgreSQL implementation of BomRepository
#[derive(Clone)]
pub struct PgBomRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct BomRow {
    id: Uuid,
    bom_number: String,
    product_id: Uuid,
    name: String,
    description: Option<String>,
    revision: String,
    status: String,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct BomComponentRow {
    id: Uuid,
    bom_id: Uuid,
    component_product_id: Option<Uuid>,
    component_sku: Option<String>,
    name: String,
    quantity: Decimal,
    unit_of_measure: String,
    position: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgBomRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_bom(row: BomRow, components: Vec<BomComponent>) -> Result<BillOfMaterials> {
        let BomRow {
            id,
            bom_number,
            product_id,
            name,
            description,
            revision,
            status,
            created_by,
            updated_by,
            created_at,
            updated_at,
        } = row;

        let status: BomStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid bom.status '{}': {}", status, e))
        })?;

        Ok(BillOfMaterials {
            id,
            bom_number,
            product_id,
            name,
            description,
            revision,
            status,
            components,
            created_by,
            updated_by,
            created_at,
            updated_at,
        })
    }

    fn row_to_component(row: BomComponentRow) -> BomComponent {
        BomComponent {
            id: row.id,
            bom_id: row.bom_id,
            component_product_id: row.component_product_id,
            component_sku: row.component_sku,
            name: row.name,
            quantity: row.quantity,
            unit_of_measure: row.unit_of_measure,
            position: row.position,
            notes: row.notes,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    async fn get_components_async(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        let rows = sqlx::query_as::<_, BomComponentRow>(
            "SELECT * FROM manufacturing_bom_components WHERE bom_id = $1 ORDER BY position, created_at",
        )
        .bind(bom_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(rows.into_iter().map(Self::row_to_component).collect())
    }

    /// Create a BOM (async)
    pub async fn create_async(&self, input: CreateBom) -> Result<BillOfMaterials> {
        let id = Uuid::new_v4();
        let bom_number = BillOfMaterials::generate_bom_number();
        let now = Utc::now();
        let revision = input.revision.unwrap_or_else(|| "A".to_string());

        sqlx::query(
            r#"
            INSERT INTO manufacturing_boms (id, bom_number, product_id, name, description, revision, status, created_by, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9)
            "#,
        )
        .bind(id)
        .bind(&bom_number)
        .bind(input.product_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&revision)
        .bind(input.created_by)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Create components if provided
        let mut components = Vec::new();
        if let Some(comp_inputs) = input.components {
            for comp_input in comp_inputs {
                let comp = self.add_component_async(id, comp_input).await?;
                components.push(comp);
            }
        }

        Ok(BillOfMaterials {
            id,
            bom_number,
            product_id: input.product_id,
            name: input.name,
            description: input.description,
            revision,
            status: BomStatus::Draft,
            components,
            created_by: input.created_by,
            updated_by: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Get BOM by ID (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        let result = sqlx::query_as::<_, BomRow>("SELECT * FROM manufacturing_boms WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

        match result {
            Some(row) => {
                let components = self.get_components_async(row.id).await?;
                Ok(Some(Self::row_to_bom(row, components)?))
            }
            None => Ok(None),
        }
    }

    /// Get BOM by number (async)
    pub async fn get_by_number_async(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        let result =
            sqlx::query_as::<_, BomRow>("SELECT * FROM manufacturing_boms WHERE bom_number = $1")
                .bind(bom_number)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;

        match result {
            Some(row) => {
                let components = self.get_components_async(row.id).await?;
                Ok(Some(Self::row_to_bom(row, components)?))
            }
            None => Ok(None),
        }
    }

    /// Update BOM (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        let existing = self.get_async(id).await?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_name = input.name.unwrap_or(existing.name);
        let new_description = input.description.or(existing.description);
        let new_revision = input.revision.unwrap_or(existing.revision);
        let new_status = input.status.unwrap_or(existing.status);

        sqlx::query(
            "UPDATE manufacturing_boms SET name = $1, description = $2, revision = $3, status = $4, updated_by = $5, updated_at = $6 WHERE id = $7",
        )
        .bind(&new_name)
        .bind(&new_description)
        .bind(&new_revision)
        .bind(new_status.to_string())
        .bind(input.updated_by)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List BOMs (async)
    pub async fn list_async(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        let limit = filter.limit.unwrap_or(100) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;

        // Build query based on filter
        let rows = if let Some(product_id) = filter.product_id {
            sqlx::query_as::<_, BomRow>(
                "SELECT * FROM manufacturing_boms WHERE product_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(product_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?
        } else if let Some(status) = filter.status {
            sqlx::query_as::<_, BomRow>(
                "SELECT * FROM manufacturing_boms WHERE status = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            )
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?
        } else {
            sqlx::query_as::<_, BomRow>(
                "SELECT * FROM manufacturing_boms ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?
        };

        let mut boms = Vec::new();
        for row in rows {
            let components = self.get_components_async(row.id).await?;
            boms.push(Self::row_to_bom(row, components)?);
        }

        Ok(boms)
    }

    /// Delete BOM (async) - marks as obsolete
    pub async fn delete_async(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE manufacturing_boms SET status = 'obsolete', updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Add component (async)
    pub async fn add_component_async(
        &self,
        bom_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let uom = component.unit_of_measure.unwrap_or_else(|| "each".to_string());

        sqlx::query(
            r#"
            INSERT INTO manufacturing_bom_components (id, bom_id, component_product_id, component_sku, name, quantity, unit_of_measure, position, notes, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(id)
        .bind(bom_id)
        .bind(component.component_product_id)
        .bind(&component.component_sku)
        .bind(&component.name)
        .bind(component.quantity)
        .bind(&uom)
        .bind(&component.position)
        .bind(&component.notes)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(BomComponent {
            id,
            bom_id,
            component_product_id: component.component_product_id,
            component_sku: component.component_sku,
            name: component.name,
            quantity: component.quantity,
            unit_of_measure: uom,
            position: component.position,
            notes: component.notes,
            created_at: now,
            updated_at: now,
        })
    }

    /// Update component (async)
    pub async fn update_component_async(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        let now = Utc::now();
        let uom = component.unit_of_measure.unwrap_or_else(|| "each".to_string());

        sqlx::query(
            "UPDATE manufacturing_bom_components SET component_product_id = $1, component_sku = $2, name = $3, quantity = $4, unit_of_measure = $5, position = $6, notes = $7, updated_at = $8 WHERE id = $9",
        )
        .bind(component.component_product_id)
        .bind(&component.component_sku)
        .bind(&component.name)
        .bind(component.quantity)
        .bind(&uom)
        .bind(&component.position)
        .bind(&component.notes)
        .bind(now)
        .bind(component_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        // Fetch and return the updated component
        let row = sqlx::query_as::<_, BomComponentRow>(
            "SELECT * FROM manufacturing_bom_components WHERE id = $1",
        )
        .bind(component_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(Self::row_to_component(row))
    }

    /// Remove component (async)
    pub async fn remove_component_async(&self, component_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM manufacturing_bom_components WHERE id = $1")
            .bind(component_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        Ok(())
    }

    /// Activate BOM (async)
    pub async fn activate_async(&self, id: Uuid) -> Result<BillOfMaterials> {
        self.update_async(id, UpdateBom { status: Some(BomStatus::Active), ..Default::default() })
            .await
    }

    /// Count BOMs (async)
    pub async fn count_async(&self, filter: BomFilter) -> Result<u64> {
        let count: (i64,) = if let Some(product_id) = filter.product_id {
            sqlx::query_as("SELECT COUNT(*) FROM manufacturing_boms WHERE product_id = $1")
                .bind(product_id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?
        } else if let Some(status) = filter.status {
            sqlx::query_as("SELECT COUNT(*) FROM manufacturing_boms WHERE status = $1")
                .bind(status.to_string())
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?
        } else {
            sqlx::query_as("SELECT COUNT(*) FROM manufacturing_boms")
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?
        };

        Ok(count.0 as u64)
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Create multiple BOMs - partial success allowed (async)
    pub async fn create_batch_async(
        &self,
        inputs: Vec<CreateBom>,
    ) -> Result<BatchResult<BillOfMaterials>> {
        validate_batch_size(&inputs)?;

        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create_async(input).await {
                Ok(bom) => result.record_success(bom),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    /// Create multiple BOMs - atomic (all-or-nothing) (async)
    pub async fn create_batch_atomic_async(
        &self,
        inputs: Vec<CreateBom>,
    ) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&inputs)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut boms = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let bom_number = BillOfMaterials::generate_bom_number();
            let now = Utc::now();
            let revision = input.revision.unwrap_or_else(|| "A".to_string());

            sqlx::query(
                r#"
                INSERT INTO manufacturing_boms (id, bom_number, product_id, name, description, revision, status, created_by, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, 'draft', $7, $8, $9)
                "#,
            )
            .bind(id)
            .bind(&bom_number)
            .bind(input.product_id)
            .bind(&input.name)
            .bind(&input.description)
            .bind(&revision)
            .bind(input.created_by)
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Create components if provided
            let mut components = Vec::new();
            if let Some(comp_inputs) = input.components {
                for comp_input in comp_inputs {
                    let comp_id = Uuid::new_v4();
                    let uom =
                        comp_input.unit_of_measure.clone().unwrap_or_else(|| "each".to_string());

                    sqlx::query(
                        r#"
                        INSERT INTO manufacturing_bom_components (id, bom_id, component_product_id, component_sku, name, quantity, unit_of_measure, position, notes, created_at, updated_at)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                        "#,
                    )
                    .bind(comp_id)
                    .bind(id)
                    .bind(comp_input.component_product_id)
                    .bind(&comp_input.component_sku)
                    .bind(&comp_input.name)
                    .bind(comp_input.quantity)
                    .bind(&uom)
                    .bind(&comp_input.position)
                    .bind(&comp_input.notes)
                    .bind(now)
                    .bind(now)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

                    components.push(BomComponent {
                        id: comp_id,
                        bom_id: id,
                        component_product_id: comp_input.component_product_id,
                        component_sku: comp_input.component_sku,
                        name: comp_input.name,
                        quantity: comp_input.quantity,
                        unit_of_measure: uom,
                        position: comp_input.position,
                        notes: comp_input.notes,
                        created_at: now,
                        updated_at: now,
                    });
                }
            }

            boms.push(BillOfMaterials {
                id,
                bom_number,
                product_id: input.product_id,
                name: input.name,
                description: input.description,
                revision,
                status: BomStatus::Draft,
                components,
                created_by: input.created_by,
                updated_by: None,
                created_at: now,
                updated_at: now,
            });
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(boms)
    }

    /// Update multiple BOMs - partial success allowed (async)
    pub async fn update_batch_async(
        &self,
        updates: Vec<(Uuid, UpdateBom)>,
    ) -> Result<BatchResult<BillOfMaterials>> {
        validate_batch_size(&updates)?;

        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update_async(id, input).await {
                Ok(bom) => result.record_success(bom),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Update multiple BOMs - atomic (all-or-nothing) (async)
    pub async fn update_batch_atomic_async(
        &self,
        updates: Vec<(Uuid, UpdateBom)>,
    ) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&updates)?;

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut boms = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            // Fetch existing BOM
            let existing_row =
                sqlx::query_as::<_, BomRow>("SELECT * FROM manufacturing_boms WHERE id = $1")
                    .bind(id)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(map_db_error)?
                    .ok_or(CommerceError::NotFound)?;

            let existing_components = sqlx::query_as::<_, BomComponentRow>(
                "SELECT * FROM manufacturing_bom_components WHERE bom_id = $1 ORDER BY position, created_at"
            )
            .bind(id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            let existing = Self::row_to_bom(
                existing_row,
                existing_components.into_iter().map(Self::row_to_component).collect(),
            )?;

            let new_name = input.name.unwrap_or(existing.name);
            let new_description = input.description.or(existing.description);
            let new_revision = input.revision.unwrap_or(existing.revision);
            let new_status = input.status.unwrap_or(existing.status);

            sqlx::query(
                "UPDATE manufacturing_boms SET name = $1, description = $2, revision = $3, status = $4, updated_by = $5, updated_at = $6 WHERE id = $7",
            )
            .bind(&new_name)
            .bind(&new_description)
            .bind(&new_revision)
            .bind(new_status.to_string())
            .bind(input.updated_by)
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            // Fetch updated BOM
            let updated_row =
                sqlx::query_as::<_, BomRow>("SELECT * FROM manufacturing_boms WHERE id = $1")
                    .bind(id)
                    .fetch_one(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;

            let updated_components = sqlx::query_as::<_, BomComponentRow>(
                "SELECT * FROM manufacturing_bom_components WHERE bom_id = $1 ORDER BY position, created_at"
            )
            .bind(id)
            .fetch_all(tx.as_mut())
            .await
            .map_err(map_db_error)?;

            boms.push(Self::row_to_bom(
                updated_row,
                updated_components.into_iter().map(Self::row_to_component).collect(),
            )?);
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(boms)
    }

    /// Delete multiple BOMs - partial success allowed (async)
    pub async fn delete_batch_async(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;

        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete_async(id).await {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    /// Delete multiple BOMs - atomic (all-or-nothing) (async)
    pub async fn delete_batch_atomic_async(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        sqlx::query(
            "UPDATE manufacturing_boms SET status = 'obsolete', updated_at = $1 WHERE id = ANY($2)",
        )
        .bind(now)
        .bind(&ids)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(())
    }

    /// Get multiple BOMs by ID (async)
    pub async fn get_batch_async(&self, ids: Vec<Uuid>) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&ids)?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows =
            sqlx::query_as::<_, BomRow>("SELECT * FROM manufacturing_boms WHERE id = ANY($1)")
                .bind(&ids)
                .fetch_all(&self.pool)
                .await
                .map_err(map_db_error)?;

        let mut boms = Vec::with_capacity(rows.len());
        for row in rows {
            let components = self.get_components_async(row.id).await?;
            boms.push(Self::row_to_bom(row, components)?);
        }

        Ok(boms)
    }
}

impl BomRepository for PgBomRepository {
    fn create(&self, input: CreateBom) -> Result<BillOfMaterials> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        super::block_on(self.get_async(id))
    }

    fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        super::block_on(self.get_by_number_async(bom_number))
    }

    fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        super::block_on(self.update_async(id, input))
    }

    fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        super::block_on(self.add_component_async(bom_id, component))
    }

    fn update_component(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        super::block_on(self.update_component_async(component_id, component))
    }

    fn remove_component(&self, component_id: Uuid) -> Result<()> {
        super::block_on(self.remove_component_async(component_id))
    }

    fn get_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        super::block_on(self.get_components_async(bom_id))
    }

    fn activate(&self, id: Uuid) -> Result<BillOfMaterials> {
        super::block_on(self.activate_async(id))
    }

    fn count(&self, filter: BomFilter) -> Result<u64> {
        super::block_on(self.count_async(filter))
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    fn create_batch(&self, inputs: Vec<CreateBom>) -> Result<BatchResult<BillOfMaterials>> {
        super::block_on(self.create_batch_async(inputs))
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateBom>) -> Result<Vec<BillOfMaterials>> {
        super::block_on(self.create_batch_atomic_async(inputs))
    }

    fn update_batch(
        &self,
        updates: Vec<(Uuid, UpdateBom)>,
    ) -> Result<BatchResult<BillOfMaterials>> {
        super::block_on(self.update_batch_async(updates))
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateBom)>) -> Result<Vec<BillOfMaterials>> {
        super::block_on(self.update_batch_atomic_async(updates))
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        super::block_on(self.delete_batch_async(ids))
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        super::block_on(self.delete_batch_atomic_async(ids))
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<BillOfMaterials>> {
        super::block_on(self.get_batch_async(ids))
    }
}
