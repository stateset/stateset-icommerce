//! SQLite BOM (Bill of Materials) repository implementation

use super::parse_helpers::{
    parse_datetime, parse_datetime_row, parse_decimal_row, parse_uuid, parse_uuid_opt_row,
    parse_uuid_row,
};
use super::{build_in_clause, params_refs, uuid_params};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    validate_batch_size, BatchResult, BillOfMaterials, BomComponent, BomFilter, BomRepository,
    BomStatus, CommerceError, CreateBom, CreateBomComponent, Result, UpdateBom,
};
use uuid::Uuid;

/// SQLite implementation of BomRepository
pub struct SqliteBomRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteBomRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn parse_bom_status(s: &str) -> Result<BomStatus> {
        s.parse()
            .map_err(|e| CommerceError::DatabaseError(format!("Invalid bom.status '{}': {}", s, e)))
    }

    fn load_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, bom_id, component_product_id, component_sku, name, quantity,
                        unit_of_measure, position, notes, created_at, updated_at
                 FROM manufacturing_bom_components WHERE bom_id = ?",
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map([bom_id.to_string()], |row| {
                Ok(BomComponent {
                    id: parse_uuid_row(&row.get::<_, String>(0)?, "bom_component", "id")?,
                    bom_id: parse_uuid_row(&row.get::<_, String>(1)?, "bom_component", "bom_id")?,
                    component_product_id: parse_uuid_opt_row(
                        row.get::<_, Option<String>>(2)?,
                        "bom_component",
                        "component_product_id",
                    )?,
                    component_sku: row.get(3)?,
                    name: row.get(4)?,
                    quantity: parse_decimal_row(
                        &row.get::<_, String>(5)?,
                        "bom_component",
                        "quantity",
                    )?,
                    unit_of_measure: row.get(6)?,
                    position: row.get(7)?,
                    notes: row.get(8)?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>(9)?,
                        "bom_component",
                        "created_at",
                    )?,
                    updated_at: parse_datetime_row(
                        &row.get::<_, String>(10)?,
                        "bom_component",
                        "updated_at",
                    )?,
                })
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut components = Vec::new();
        for row in rows {
            components.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        Ok(components)
    }
}

impl BomRepository for SqliteBomRepository {
    fn create(&self, input: CreateBom) -> Result<BillOfMaterials> {
        let id = Uuid::new_v4();
        let bom_number = BillOfMaterials::generate_bom_number();
        let now = Utc::now();
        let revision = input.revision.clone().unwrap_or_else(|| "A".to_string());

        // Insert BOM in a scoped block to release the connection before adding components
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO manufacturing_boms (id, bom_number, product_id, name, description, revision, status, created_by, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    bom_number,
                    input.product_id.to_string(),
                    input.name,
                    input.description,
                    revision,
                    input.created_by.map(|u| u.to_string()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Create components if provided (after releasing the main connection)
        let mut components = Vec::new();
        if let Some(comp_inputs) = input.components {
            for comp_input in comp_inputs {
                let comp = self.add_component(id, comp_input)?;
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

    fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        // Query BOM in a scoped block to release connection before loading components
        let bom_data = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id, bom_number, product_id, name, description, revision, status,
                        created_by, updated_by, created_at, updated_at
                 FROM manufacturing_boms WHERE id = ?",
                [id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            );

            match result {
                Ok(data) => Some(data),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        }; // Connection released here

        match bom_data {
            Some((
                id_str,
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
            )) => {
                let bom_id = parse_uuid(&id_str, "bom", "id")?;
                let components = self.load_components(bom_id)?;

                Ok(Some(BillOfMaterials {
                    id: bom_id,
                    bom_number,
                    product_id: parse_uuid(&product_id, "bom", "product_id")?,
                    name,
                    description,
                    revision,
                    status: Self::parse_bom_status(&status)?,
                    components,
                    created_by: match created_by {
                        Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "created_by")?),
                        _ => None,
                    },
                    updated_by: match updated_by {
                        Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "updated_by")?),
                        _ => None,
                    },
                    created_at: parse_datetime(&created_at, "bom", "created_at")?,
                    updated_at: parse_datetime(&updated_at, "bom", "updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        // Query ID in a scoped block to release connection before calling self.get()
        let id_result = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM manufacturing_boms WHERE bom_number = ?",
                [bom_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(parse_uuid(&id_str, "bom", "id")?),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(CommerceError::DatabaseError(e.to_string())),
            }
        }; // Connection released here

        match id_result {
            Some(id) => self.get(id),
            None => Ok(None),
        }
    }

    fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        // Get existing BOM first (releases connection after)
        let existing = self.get(id)?.ok_or(CommerceError::NotFound)?;
        let now = Utc::now();

        let new_name = input.name.unwrap_or(existing.name);
        let new_description = input.description.or(existing.description);
        let new_revision = input.revision.unwrap_or(existing.revision);
        let new_status = input.status.unwrap_or(existing.status);

        // Do the update in a scoped block
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE manufacturing_boms SET name = ?, description = ?, revision = ?, status = ?, updated_by = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_name,
                    new_description,
                    new_revision,
                    new_status.to_string(),
                    input.updated_by.map(|u| u.to_string()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        } // Connection released here

        // Fetch and return the updated BOM
        self.get(id)?.ok_or(CommerceError::NotFound)
    }

    fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        // Collect all IDs in a scoped block to release connection before calling self.get()
        let ids = {
            let conn = self
                .pool
                .get()
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let limit = filter.limit.unwrap_or(100) as i64;
            let offset = filter.offset.unwrap_or(0) as i64;

            let mut sql = "SELECT id FROM manufacturing_boms WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(product_id) = filter.product_id {
                sql.push_str(" AND product_id = ?");
                params.push(Box::new(product_id.to_string()));
            }

            if let Some(status) = filter.status {
                sql.push_str(" AND status = ?");
                params.push(Box::new(status.to_string()));
            }

            if let Some(search) = filter.search {
                sql.push_str(" AND (name LIKE ? OR bom_number LIKE ?)");
                let search_pattern = format!("%{}%", search);
                params.push(Box::new(search_pattern.clone()));
                params.push(Box::new(search_pattern));
            }

            sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
            params.push(Box::new(limit));
            params.push(Box::new(offset));

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt
                .query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let mut id_list = Vec::new();
            for row in rows {
                let id_str = row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
                id_list.push(parse_uuid(&id_str, "bom", "id")?);
            }
            id_list
        }; // Connection released here

        // Now fetch each BOM (each call gets its own connection)
        let mut boms = Vec::new();
        for id in ids {
            if let Some(bom) = self.get(id)? {
                boms.push(bom);
            }
        }

        Ok(boms)
    }

    fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Mark as obsolete instead of hard delete
        conn.execute(
            "UPDATE manufacturing_boms SET status = 'obsolete', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let uom = component
            .unit_of_measure
            .unwrap_or_else(|| "each".to_string());

        conn.execute(
            "INSERT INTO manufacturing_bom_components (id, bom_id, component_product_id, component_sku, name, quantity, unit_of_measure, position, notes, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                bom_id.to_string(),
                component.component_product_id.map(|u| u.to_string()),
                component.component_sku,
                component.name,
                component.quantity.to_string(),
                uom,
                component.position,
                component.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

    fn update_component(
        &self,
        component_id: Uuid,
        component: CreateBomComponent,
    ) -> Result<BomComponent> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();
        let uom = component
            .unit_of_measure
            .unwrap_or_else(|| "each".to_string());

        conn.execute(
            "UPDATE manufacturing_bom_components SET component_product_id = ?, component_sku = ?, name = ?, quantity = ?, unit_of_measure = ?, position = ?, notes = ?, updated_at = ? WHERE id = ?",
            rusqlite::params![
                component.component_product_id.map(|u| u.to_string()),
                component.component_sku,
                component.name,
                component.quantity.to_string(),
                uom,
                component.position,
                component.notes,
                now.to_rfc3339(),
                component_id.to_string(),
            ],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Fetch the updated component
        let row = conn
            .query_row(
                "SELECT bom_id, created_at FROM manufacturing_bom_components WHERE id = ?",
                [component_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(BomComponent {
            id: component_id,
            bom_id: parse_uuid(&row.0, "bom_component", "bom_id")?,
            component_product_id: component.component_product_id,
            component_sku: component.component_sku,
            name: component.name,
            quantity: component.quantity,
            unit_of_measure: uom,
            position: component.position,
            notes: component.notes,
            created_at: parse_datetime(&row.1, "bom_component", "created_at")?,
            updated_at: now,
        })
    }

    fn remove_component(&self, component_id: Uuid) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "DELETE FROM manufacturing_bom_components WHERE id = ?",
            [component_id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn get_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        self.load_components(bom_id)
    }

    fn activate(&self, id: Uuid) -> Result<BillOfMaterials> {
        self.update(
            id,
            UpdateBom {
                status: Some(BomStatus::Active),
                ..Default::default()
            },
        )
    }

    fn count(&self, filter: BomFilter) -> Result<u64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut sql = "SELECT COUNT(*) FROM manufacturing_boms WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(product_id) = filter.product_id {
            sql.push_str(" AND product_id = ?");
            params.push(Box::new(product_id.to_string()));
        }

        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let count: i64 = conn
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(count as u64)
    }

    // === Batch Operations ===

    fn create_batch(&self, inputs: Vec<CreateBom>) -> Result<BatchResult<BillOfMaterials>> {
        validate_batch_size(&inputs)?;
        let mut result = BatchResult::with_capacity(inputs.len());

        for (index, input) in inputs.into_iter().enumerate() {
            match self.create(input) {
                Ok(bom) => result.record_success(bom),
                Err(e) => result.record_failure(index, None, &e),
            }
        }

        Ok(result)
    }

    fn create_batch_atomic(&self, inputs: Vec<CreateBom>) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&inputs)?;
        if inputs.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut results = Vec::with_capacity(inputs.len());

        for input in inputs {
            let id = Uuid::new_v4();
            let bom_number = BillOfMaterials::generate_bom_number();
            let now = Utc::now();
            let revision = input.revision.clone().unwrap_or_else(|| "A".to_string());

            tx.execute(
                "INSERT INTO manufacturing_boms (id, bom_number, product_id, name, description, revision, status, created_by, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    bom_number,
                    input.product_id.to_string(),
                    input.name,
                    input.description,
                    revision,
                    input.created_by.map(|u| u.to_string()),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            // Insert components if provided
            let mut components = Vec::new();
            if let Some(comp_inputs) = input.components {
                for comp_input in comp_inputs {
                    let comp_id = Uuid::new_v4();
                    let uom = comp_input
                        .unit_of_measure
                        .clone()
                        .unwrap_or_else(|| "each".to_string());

                    tx.execute(
                        "INSERT INTO manufacturing_bom_components (id, bom_id, component_product_id, component_sku, name, quantity, unit_of_measure, position, notes, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![
                            comp_id.to_string(),
                            id.to_string(),
                            comp_input.component_product_id.map(|u| u.to_string()),
                            comp_input.component_sku,
                            comp_input.name,
                            comp_input.quantity.to_string(),
                            uom,
                            comp_input.position,
                            comp_input.notes,
                            now.to_rfc3339(),
                            now.to_rfc3339(),
                        ],
                    )
                    .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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

            results.push(BillOfMaterials {
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

        tx.commit()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(results)
    }

    fn update_batch(
        &self,
        updates: Vec<(Uuid, UpdateBom)>,
    ) -> Result<BatchResult<BillOfMaterials>> {
        validate_batch_size(&updates)?;
        let mut result = BatchResult::with_capacity(updates.len());

        for (index, (id, input)) in updates.into_iter().enumerate() {
            match self.update(id, input) {
                Ok(bom) => result.record_success(bom),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn update_batch_atomic(&self, updates: Vec<(Uuid, UpdateBom)>) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&updates)?;
        if updates.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = self.conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut results = Vec::with_capacity(updates.len());

        for (id, input) in updates {
            let now = Utc::now();

            // Fetch existing BOM data within the transaction
            let existing_data: (String, Option<String>, String, String) = tx
                .query_row(
                    "SELECT name, description, revision, status FROM manufacturing_boms WHERE id = ?",
                    [id.to_string()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => CommerceError::NotFound,
                    _ => CommerceError::DatabaseError(e.to_string()),
                })?;

            let new_name = input.name.unwrap_or(existing_data.0);
            let new_description = input.description.or(existing_data.1);
            let new_revision = input.revision.unwrap_or(existing_data.2);
            let existing_status = Self::parse_bom_status(&existing_data.3)?;
            let new_status = input.status.unwrap_or(existing_status);

            tx.execute(
                "UPDATE manufacturing_boms SET name = ?, description = ?, revision = ?, status = ?, updated_by = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    new_name,
                    new_description,
                    new_revision,
                    new_status.to_string(),
                    input.updated_by.map(|u| u.to_string()),
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            // Fetch updated BOM with full data for the result
            let bom_data: (
                String,
                String,
                Option<String>,
                Option<String>,
                String,
                String,
            ) = tx
                .query_row(
                    "SELECT bom_number, product_id, created_by, updated_by, created_at, updated_at
                     FROM manufacturing_boms WHERE id = ?",
                    [id.to_string()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                        ))
                    },
                )
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            results.push(BillOfMaterials {
                id,
                bom_number: bom_data.0,
                product_id: parse_uuid(&bom_data.1, "bom", "product_id")?,
                name: new_name,
                description: new_description,
                revision: new_revision,
                status: new_status,
                components: vec![], // Components loaded separately after commit
                created_by: match bom_data.2 {
                    Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "created_by")?),
                    _ => None,
                },
                updated_by: match bom_data.3 {
                    Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "updated_by")?),
                    _ => None,
                },
                created_at: parse_datetime(&bom_data.4, "bom", "created_at")?,
                updated_at: parse_datetime(&bom_data.5, "bom", "updated_at")?,
            });
        }

        tx.commit()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Load components for each BOM after commit
        for bom in &mut results {
            bom.components = self.load_components(bom.id)?;
        }

        Ok(results)
    }

    fn delete_batch(&self, ids: Vec<Uuid>) -> Result<BatchResult<Uuid>> {
        validate_batch_size(&ids)?;
        let mut result = BatchResult::with_capacity(ids.len());

        for (index, id) in ids.into_iter().enumerate() {
            match self.delete(id) {
                Ok(()) => result.record_success(id),
                Err(e) => result.record_failure(index, Some(id.to_string()), &e),
            }
        }

        Ok(result)
    }

    fn delete_batch_atomic(&self, ids: Vec<Uuid>) -> Result<()> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn()?;
        let tx = conn
            .transaction()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let placeholders = build_in_clause(ids.len());

        // Mark BOMs as obsolete (soft delete) using IN clause
        let sql = format!(
            "UPDATE manufacturing_boms SET status = 'obsolete', updated_at = ? WHERE id IN ({})",
            placeholders
        );

        // Build params with timestamp first, then IDs
        let now = Utc::now().to_rfc3339();
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        all_params.extend(uuid_params(&ids));
        let all_params_refs: Vec<&dyn rusqlite::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        tx.execute(&sql, all_params_refs.as_slice())
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        tx.commit()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<BillOfMaterials>> {
        validate_batch_size(&ids)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn()?;
        let placeholders = build_in_clause(ids.len());
        let sql = format!(
            "SELECT id, bom_number, product_id, name, description, revision, status,
                    created_by, updated_by, created_at, updated_at
             FROM manufacturing_boms WHERE id IN ({})",
            placeholders
        );

        let params = uuid_params(&ids);
        let params_refs = params_refs(&params);

        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let rows = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ))
            })
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let mut bom_data_list = Vec::new();
        for row in rows {
            bom_data_list.push(row.map_err(|e| CommerceError::DatabaseError(e.to_string()))?);
        }

        // Release connection before loading components
        drop(stmt);
        drop(conn);

        // Build BOMs and load components
        let mut boms = Vec::with_capacity(bom_data_list.len());
        for (
            id_str,
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
        ) in bom_data_list
        {
            let bom_id = parse_uuid(&id_str, "bom", "id")?;
            let components = self.load_components(bom_id)?;

            boms.push(BillOfMaterials {
                id: bom_id,
                bom_number,
                product_id: parse_uuid(&product_id, "bom", "product_id")?,
                name,
                description,
                revision,
                status: Self::parse_bom_status(&status)?,
                components,
                created_by: match created_by {
                    Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "created_by")?),
                    _ => None,
                },
                updated_by: match updated_by {
                    Some(ref s) if !s.is_empty() => Some(parse_uuid(s, "bom", "updated_by")?),
                    _ => None,
                },
                created_at: parse_datetime(&created_at, "bom", "created_at")?,
                updated_at: parse_datetime(&updated_at, "bom", "updated_at")?,
            });
        }

        Ok(boms)
    }
}
