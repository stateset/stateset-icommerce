//! SQLite BOM (Bill of Materials) repository implementation

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use std::str::FromStr;
use stateset_core::{
    BillOfMaterials, BomComponent, BomFilter, BomRepository, BomStatus, CommerceError,
    CreateBom, CreateBomComponent, Result, UpdateBom,
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

    fn parse_bom_status(s: &str) -> BomStatus {
        match s {
            "active" => BomStatus::Active,
            "obsolete" => BomStatus::Obsolete,
            _ => BomStatus::Draft,
        }
    }

    fn parse_datetime(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now())
    }

    fn load_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                    id: Uuid::parse_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    bom_id: Uuid::parse_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    component_product_id: row
                        .get::<_, Option<String>>(2)?
                        .and_then(|s| Uuid::parse_str(&s).ok()),
                    component_sku: row.get(3)?,
                    name: row.get(4)?,
                    quantity: Decimal::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
                    unit_of_measure: row.get(6)?,
                    position: row.get(7)?,
                    notes: row.get(8)?,
                    created_at: Self::parse_datetime(&row.get::<_, String>(9)?),
                    updated_at: Self::parse_datetime(&row.get::<_, String>(10)?),
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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
            Some((id_str, bom_number, product_id, name, description, revision, status, created_by, updated_by, created_at, updated_at)) => {
                let bom_id = Uuid::parse_str(&id_str).unwrap_or_default();
                let components = self.load_components(bom_id)?;

                Ok(Some(BillOfMaterials {
                    id: bom_id,
                    bom_number,
                    product_id: Uuid::parse_str(&product_id).unwrap_or_default(),
                    name,
                    description,
                    revision,
                    status: Self::parse_bom_status(&status),
                    components,
                    created_by: created_by.and_then(|s| Uuid::parse_str(&s).ok()),
                    updated_by: updated_by.and_then(|s| Uuid::parse_str(&s).ok()),
                    created_at: Self::parse_datetime(&created_at),
                    updated_at: Self::parse_datetime(&updated_at),
                }))
            }
            None => Ok(None),
        }
    }

    fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        // Query ID in a scoped block to release connection before calling self.get()
        let id_result = {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

            let result = conn.query_row(
                "SELECT id FROM manufacturing_boms WHERE bom_number = ?",
                [bom_number],
                |row| row.get::<_, String>(0),
            );

            match result {
                Ok(id_str) => Some(Uuid::parse_str(&id_str).unwrap_or_default()),
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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
                id_list.push(Uuid::parse_str(&id_str).unwrap_or_default());
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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Mark as obsolete instead of hard delete
        conn.execute(
            "UPDATE manufacturing_boms SET status = 'obsolete', updated_at = ? WHERE id = ?",
            rusqlite::params![Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let uom = component.unit_of_measure.unwrap_or_else(|| "each".to_string());

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

    fn update_component(&self, component_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let now = Utc::now();
        let uom = component.unit_of_measure.unwrap_or_else(|| "each".to_string());

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
        let row = conn.query_row(
            "SELECT bom_id, created_at FROM manufacturing_bom_components WHERE id = ?",
            [component_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        Ok(BomComponent {
            id: component_id,
            bom_id: Uuid::parse_str(&row.0).unwrap_or_default(),
            component_product_id: component.component_product_id,
            component_sku: component.component_sku,
            name: component.name,
            quantity: component.quantity,
            unit_of_measure: uom,
            position: component.position,
            notes: component.notes,
            created_at: Self::parse_datetime(&row.1),
            updated_at: now,
        })
    }

    fn remove_component(&self, component_id: Uuid) -> Result<()> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
        self.update(id, UpdateBom {
            status: Some(BomStatus::Active),
            ..Default::default()
        })
    }

    fn count(&self, filter: BomFilter) -> Result<u64> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

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
}
