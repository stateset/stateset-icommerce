//! SQLite implementation of the units-of-measure repository
//!
//! Covers unit classes, units of measure, and conversion rules.

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_uuid_opt_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, ConversionRuleType, CreateUnitClass, CreateUnitConversionRule,
    CreateUnitOfMeasure, Result, UnitClass, UnitClassId, UnitConversionRule, UnitConversionRuleId,
    UnitOfMeasure, UnitOfMeasureId, UnitOfMeasureRepository,
};

#[derive(Debug)]
pub struct SqliteUnitOfMeasureRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteUnitOfMeasureRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_class(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnitClass> {
        Ok(UnitClass {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "unit_class", "id")?.into(),
            name: row.get("name")?,
            description: row.get("description")?,
            base_uom_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("base_uom_id")?,
                "unit_class",
                "base_uom_id",
            )?
            .map(Into::into),
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "unit_class",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "unit_class",
                "updated_at",
            )?,
        })
    }

    fn row_to_uom(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnitOfMeasure> {
        Ok(UnitOfMeasure {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "uom", "id")?.into(),
            unit_class_id: parse_uuid_row(
                &row.get::<_, String>("unit_class_id")?,
                "uom",
                "unit_class_id",
            )?
            .into(),
            name: row.get("name")?,
            abbreviation: row.get("abbreviation")?,
            factor: parse_decimal_row(&row.get::<_, String>("factor")?, "uom", "factor")?,
            is_base: row.get::<_, i32>("is_base")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "uom",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "uom",
                "updated_at",
            )?,
        })
    }

    fn row_to_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnitConversionRule> {
        Ok(UnitConversionRule {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "conversion_rule", "id")?.into(),
            rule_type: parse_enum_row::<ConversionRuleType>(
                &row.get::<_, String>("rule_type")?,
                "conversion_rule",
                "rule_type",
            )?,
            product_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("product_id")?,
                "conversion_rule",
                "product_id",
            )?
            .map(Into::into),
            from_uom_id: parse_uuid_row(
                &row.get::<_, String>("from_uom_id")?,
                "conversion_rule",
                "from_uom_id",
            )?
            .into(),
            to_uom_id: parse_uuid_row(
                &row.get::<_, String>("to_uom_id")?,
                "conversion_rule",
                "to_uom_id",
            )?
            .into(),
            factor: parse_decimal_row(
                &row.get::<_, String>("factor")?,
                "conversion_rule",
                "factor",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "conversion_rule",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "conversion_rule",
                "updated_at",
            )?,
        })
    }
}

impl UnitOfMeasureRepository for SqliteUnitOfMeasureRepository {
    fn create_class(&self, input: CreateUnitClass) -> Result<UnitClass> {
        let id = UnitClassId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO unit_classes (id, name, description, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?)",
                rusqlite::params![&id_str, &input.name, &input.description, &now_str, &now_str],
            )?;
            tx.query_row("SELECT * FROM unit_classes WHERE id = ?", [&id_str], Self::row_to_class)
        })
    }

    fn list_classes(&self) -> Result<Vec<UnitClass>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT * FROM unit_classes ORDER BY name").map_err(map_db_error)?;
        let rows = stmt
            .query_map([], Self::row_to_class)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete_class(&self, id: UnitClassId) -> Result<()> {
        let conn = self.conn()?;
        let referenced: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM units_of_measure WHERE unit_class_id = ?",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(map_db_error)?;
        if referenced > 0 {
            return Err(CommerceError::Conflict("unit class still has units of measure".into()));
        }
        conn.execute("DELETE FROM unit_classes WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn create_uom(&self, input: CreateUnitOfMeasure) -> Result<UnitOfMeasure> {
        let id = UnitOfMeasureId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO units_of_measure (id, unit_class_id, name, abbreviation, factor, is_base, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, 0, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.unit_class_id.to_string(),
                    &input.name,
                    &input.abbreviation,
                    input.factor.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row("SELECT * FROM units_of_measure WHERE id = ?", [&id_str], Self::row_to_uom)
        })
    }

    fn list_uoms(&self, class_id: Option<UnitClassId>) -> Result<Vec<UnitOfMeasure>> {
        let conn = self.conn()?;
        let (sql, param): (&str, Option<String>) = match class_id {
            Some(c) => (
                "SELECT * FROM units_of_measure WHERE unit_class_id = ? ORDER BY name",
                Some(c.to_string()),
            ),
            None => ("SELECT * FROM units_of_measure ORDER BY name", None),
        };
        let mut stmt = conn.prepare(sql).map_err(map_db_error)?;
        let rows = if let Some(p) = param {
            stmt.query_map([p], Self::row_to_uom)
        } else {
            stmt.query_map([], Self::row_to_uom)
        }
        .map_err(map_db_error)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(map_db_error)?;
        Ok(rows)
    }

    fn set_base_uom(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let class_id: String = tx.query_row(
                "SELECT unit_class_id FROM units_of_measure WHERE id = ?",
                [&id_str],
                |r| r.get(0),
            )?;
            // Unset any current base in the class, set the new one.
            tx.execute(
                "UPDATE units_of_measure SET is_base = 0, updated_at = ? WHERE unit_class_id = ?",
                rusqlite::params![&now_str, &class_id],
            )?;
            tx.execute(
                "UPDATE units_of_measure SET is_base = 1, updated_at = ? WHERE id = ?",
                rusqlite::params![&now_str, &id_str],
            )?;
            tx.execute(
                "UPDATE unit_classes SET base_uom_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![&id_str, &now_str, &class_id],
            )?;
            tx.query_row("SELECT * FROM units_of_measure WHERE id = ?", [&id_str], Self::row_to_uom)
        })
    }

    fn delete_uom(&self, id: UnitOfMeasureId) -> Result<()> {
        let conn = self.conn()?;
        let referenced: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM unit_conversion_rules WHERE from_uom_id = ?1 OR to_uom_id = ?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .map_err(map_db_error)?;
        if referenced > 0 {
            return Err(CommerceError::Conflict(
                "unit of measure is still referenced by a conversion rule".into(),
            ));
        }
        conn.execute("DELETE FROM units_of_measure WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn create_rule(&self, input: CreateUnitConversionRule) -> Result<UnitConversionRule> {
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
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO unit_conversion_rules (id, rule_type, product_id, from_uom_id, to_uom_id, factor, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.rule_type.to_string(),
                    input.product_id.map(|p| p.to_string()),
                    input.from_uom_id.to_string(),
                    input.to_uom_id.to_string(),
                    input.factor.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;
            tx.query_row(
                "SELECT * FROM unit_conversion_rules WHERE id = ?",
                [&id_str],
                Self::row_to_rule,
            )
        })
    }

    fn list_rules(&self) -> Result<Vec<UnitConversionRule>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM unit_conversion_rules ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([], Self::row_to_rule)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete_rule(&self, id: UnitConversionRuleId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM unit_conversion_rules WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::ProductId;

    fn test_repo() -> SqliteUnitOfMeasureRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteUnitOfMeasureRepository::new(db.pool().clone())
    }

    #[test]
    fn class_uom_lifecycle() {
        let repo = test_repo();
        let class = repo
            .create_class(CreateUnitClass { name: "Weight".into(), description: None })
            .expect("class");
        let g = repo
            .create_uom(CreateUnitOfMeasure {
                unit_class_id: class.id,
                name: "Gram".into(),
                abbreviation: "g".into(),
                factor: dec!(1),
            })
            .expect("uom");
        let _kg = repo
            .create_uom(CreateUnitOfMeasure {
                unit_class_id: class.id,
                name: "Kilogram".into(),
                abbreviation: "kg".into(),
                factor: dec!(1000),
            })
            .expect("uom2");

        assert_eq!(repo.list_uoms(Some(class.id)).expect("list").len(), 2);

        let based = repo.set_base_uom(g.id).expect("base");
        assert!(based.is_base);

        // class can't be deleted while UOMs exist
        assert!(repo.delete_class(class.id).is_err());
    }

    #[test]
    fn sku_rule_requires_product() {
        let repo = test_repo();
        let res = repo.create_rule(CreateUnitConversionRule {
            rule_type: ConversionRuleType::Sku,
            product_id: None,
            from_uom_id: UnitOfMeasureId::new(),
            to_uom_id: UnitOfMeasureId::new(),
            factor: dec!(2),
        });
        assert!(res.is_err());
    }

    #[test]
    fn system_rule_rejects_product() {
        let repo = test_repo();
        let res = repo.create_rule(CreateUnitConversionRule {
            rule_type: ConversionRuleType::System,
            product_id: Some(ProductId::new()),
            from_uom_id: UnitOfMeasureId::new(),
            to_uom_id: UnitOfMeasureId::new(),
            factor: dec!(2),
        });
        assert!(res.is_err());
    }

    #[test]
    fn create_and_list_rules() {
        let repo = test_repo();
        let rule = repo
            .create_rule(CreateUnitConversionRule {
                rule_type: ConversionRuleType::System,
                product_id: None,
                from_uom_id: UnitOfMeasureId::new(),
                to_uom_id: UnitOfMeasureId::new(),
                factor: dec!(12),
            })
            .expect("rule");
        assert_eq!(rule.factor, dec!(12));
        assert_eq!(repo.list_rules().expect("list").len(), 1);
        repo.delete_rule(rule.id).expect("delete");
        assert_eq!(repo.list_rules().expect("list").len(), 0);
    }
}
