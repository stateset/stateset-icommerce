//! PostgreSQL implementation of the units-of-measure repository
//!
//! Covers unit classes, units of measure, and conversion rules.

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, ConversionRuleType, CreateUnitClass, CreateUnitConversionRule,
    CreateUnitOfMeasure, Result, UnitClass, UnitClassId, UnitConversionRule, UnitConversionRuleId,
    UnitOfMeasure, UnitOfMeasureId, UnitOfMeasureRepository,
};

/// PostgreSQL-backed [`UnitOfMeasureRepository`].
#[derive(Debug, Clone)]
pub struct PgUnitOfMeasureRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ClassRow {
    id: UnitClassId,
    name: String,
    description: Option<String>,
    base_uom_id: Option<UnitOfMeasureId>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct UomRow {
    id: UnitOfMeasureId,
    unit_class_id: UnitClassId,
    name: String,
    abbreviation: String,
    factor: Decimal,
    is_base: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct RuleRow {
    id: UnitConversionRuleId,
    rule_type: String,
    product_id: Option<uuid::Uuid>,
    from_uom_id: UnitOfMeasureId,
    to_uom_id: UnitOfMeasureId,
    factor: Decimal,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgUnitOfMeasureRepository {
    /// Create a new repository over the given pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_class(row: ClassRow) -> UnitClass {
        UnitClass {
            id: row.id,
            name: row.name,
            description: row.description,
            base_uom_id: row.base_uom_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_uom(row: UomRow) -> UnitOfMeasure {
        UnitOfMeasure {
            id: row.id,
            unit_class_id: row.unit_class_id,
            name: row.name,
            abbreviation: row.abbreviation,
            factor: row.factor,
            is_base: row.is_base,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }

    fn row_to_rule(row: RuleRow) -> Result<UnitConversionRule> {
        let rule_type: ConversionRuleType = row.rule_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid conversion_rule.rule_type '{}': {}",
                row.rule_type, e
            ))
        })?;
        Ok(UnitConversionRule {
            id: row.id,
            rule_type,
            product_id: row.product_id.map(Into::into),
            from_uom_id: row.from_uom_id,
            to_uom_id: row.to_uom_id,
            factor: row.factor,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_class(&self, id: UnitClassId) -> Result<UnitClass> {
        let row = sqlx::query_as::<_, ClassRow>("SELECT * FROM unit_classes WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_class).ok_or(CommerceError::NotFound)
    }

    async fn fetch_uom(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        let row = sqlx::query_as::<_, UomRow>("SELECT * FROM units_of_measure WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_uom).ok_or(CommerceError::NotFound)
    }

    /// Create a unit class.
    pub async fn create_class_async(&self, input: CreateUnitClass) -> Result<UnitClass> {
        let id = UnitClassId::new();
        sqlx::query(
            "INSERT INTO unit_classes (id, name, description, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $4)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        self.fetch_class(id).await
    }

    /// List unit classes.
    pub async fn list_classes_async(&self) -> Result<Vec<UnitClass>> {
        let rows = sqlx::query_as::<_, ClassRow>("SELECT * FROM unit_classes ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_class).collect())
    }

    /// Delete a unit class (fails if still referenced).
    pub async fn delete_class_async(&self, id: UnitClassId) -> Result<()> {
        let (referenced,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM units_of_measure WHERE unit_class_id = $1")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(map_db_error)?;
        if referenced > 0 {
            return Err(CommerceError::Conflict("unit class still has units of measure".into()));
        }
        sqlx::query("DELETE FROM unit_classes WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Create a unit of measure under a class.
    pub async fn create_uom_async(&self, input: CreateUnitOfMeasure) -> Result<UnitOfMeasure> {
        let id = UnitOfMeasureId::new();
        sqlx::query(
            "INSERT INTO units_of_measure (id, unit_class_id, name, abbreviation, factor, is_base, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, FALSE, $6, $6)",
        )
        .bind(id)
        .bind(input.unit_class_id)
        .bind(&input.name)
        .bind(&input.abbreviation)
        .bind(input.factor)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        self.fetch_uom(id).await
    }

    /// List units of measure, optionally scoped to a class.
    pub async fn list_uoms_async(
        &self,
        class_id: Option<UnitClassId>,
    ) -> Result<Vec<UnitOfMeasure>> {
        let rows = match class_id {
            Some(class_id) => {
                sqlx::query_as::<_, UomRow>(
                    "SELECT * FROM units_of_measure WHERE unit_class_id = $1 ORDER BY name",
                )
                .bind(class_id)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as::<_, UomRow>("SELECT * FROM units_of_measure ORDER BY name")
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(map_db_error)?;
        Ok(rows.into_iter().map(Self::row_to_uom).collect())
    }

    /// Mark a UOM as the base unit for its class.
    pub async fn set_base_uom_async(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let class_id: Option<UnitClassId> =
            sqlx::query_scalar("SELECT unit_class_id FROM units_of_measure WHERE id = $1")
                .bind(id)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;
        let class_id = class_id.ok_or(CommerceError::NotFound)?;

        // Unset any current base in the class, set the new one.
        sqlx::query(
            "UPDATE units_of_measure SET is_base = FALSE, updated_at = $1 WHERE unit_class_id = $2",
        )
        .bind(now)
        .bind(class_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        sqlx::query("UPDATE units_of_measure SET is_base = TRUE, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        sqlx::query("UPDATE unit_classes SET base_uom_id = $1, updated_at = $2 WHERE id = $3")
            .bind(id)
            .bind(now)
            .bind(class_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        self.fetch_uom(id).await
    }

    /// Delete a unit of measure (fails if still referenced).
    pub async fn delete_uom_async(&self, id: UnitOfMeasureId) -> Result<()> {
        let (referenced,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM unit_conversion_rules WHERE from_uom_id = $1 OR to_uom_id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;
        if referenced > 0 {
            return Err(CommerceError::Conflict(
                "unit of measure is still referenced by a conversion rule".into(),
            ));
        }
        sqlx::query("DELETE FROM units_of_measure WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Create a conversion rule.
    pub async fn create_rule_async(
        &self,
        input: CreateUnitConversionRule,
    ) -> Result<UnitConversionRule> {
        // Enforce the rule_type / product_id invariant.
        match input.rule_type {
            ConversionRuleType::Sku if input.product_id.is_none() => {
                return Err(CommerceError::ValidationError(
                    "SKU conversion rules require a product_id".into(),
                ));
            }
            ConversionRuleType::System if input.product_id.is_some() => {
                return Err(CommerceError::ValidationError(
                    "SYSTEM conversion rules must not carry a product_id".into(),
                ));
            }
            _ => {}
        }
        let id = UnitConversionRuleId::new();
        sqlx::query(
            "INSERT INTO unit_conversion_rules (id, rule_type, product_id, from_uom_id, to_uom_id, factor, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
        )
        .bind(id)
        .bind(input.rule_type.to_string())
        .bind(input.product_id.map(|p| *p.as_uuid()))
        .bind(input.from_uom_id)
        .bind(input.to_uom_id)
        .bind(input.factor)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let row = sqlx::query_as::<_, RuleRow>("SELECT * FROM unit_conversion_rules WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;
        Self::row_to_rule(row)
    }

    /// List conversion rules.
    pub async fn list_rules_async(&self) -> Result<Vec<UnitConversionRule>> {
        let rows = sqlx::query_as::<_, RuleRow>(
            "SELECT * FROM unit_conversion_rules ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_rule).collect::<Result<Vec<_>>>()
    }

    /// Delete a conversion rule.
    pub async fn delete_rule_async(&self, id: UnitConversionRuleId) -> Result<()> {
        sqlx::query("DELETE FROM unit_conversion_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

impl UnitOfMeasureRepository for PgUnitOfMeasureRepository {
    fn create_class(&self, input: CreateUnitClass) -> Result<UnitClass> {
        block_on(self.create_class_async(input))
    }

    fn list_classes(&self) -> Result<Vec<UnitClass>> {
        block_on(self.list_classes_async())
    }

    fn delete_class(&self, id: UnitClassId) -> Result<()> {
        block_on(self.delete_class_async(id))
    }

    fn create_uom(&self, input: CreateUnitOfMeasure) -> Result<UnitOfMeasure> {
        block_on(self.create_uom_async(input))
    }

    fn list_uoms(&self, class_id: Option<UnitClassId>) -> Result<Vec<UnitOfMeasure>> {
        block_on(self.list_uoms_async(class_id))
    }

    fn set_base_uom(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        block_on(self.set_base_uom_async(id))
    }

    fn delete_uom(&self, id: UnitOfMeasureId) -> Result<()> {
        block_on(self.delete_uom_async(id))
    }

    fn create_rule(&self, input: CreateUnitConversionRule) -> Result<UnitConversionRule> {
        block_on(self.create_rule_async(input))
    }

    fn list_rules(&self) -> Result<Vec<UnitConversionRule>> {
        block_on(self.list_rules_async())
    }

    fn delete_rule(&self, id: UnitConversionRuleId) -> Result<()> {
        block_on(self.delete_rule_async(id))
    }
}
