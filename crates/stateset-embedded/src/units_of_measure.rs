//! Units of measure, unit classes, and conversion rule operations.

use stateset_core::{
    CreateUnitClass, CreateUnitConversionRule, CreateUnitOfMeasure, Result, UnitClass, UnitClassId,
    UnitConversionRule, UnitConversionRuleId, UnitOfMeasure, UnitOfMeasureId,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Units-of-measure operations.
pub struct UnitsOfMeasure {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for UnitsOfMeasure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitsOfMeasure").finish_non_exhaustive()
    }
}

impl UnitsOfMeasure {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether units of measure are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::UnitsOfMeasure)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::UnitsOfMeasure)
    }

    /// Create a unit class.
    pub fn create_class(&self, input: CreateUnitClass) -> Result<UnitClass> {
        self.ensure()?;
        self.db.units_of_measure().create_class(input)
    }

    /// List unit classes.
    pub fn list_classes(&self) -> Result<Vec<UnitClass>> {
        self.ensure()?;
        self.db.units_of_measure().list_classes()
    }

    /// Delete a unit class.
    pub fn delete_class(&self, id: UnitClassId) -> Result<()> {
        self.ensure()?;
        self.db.units_of_measure().delete_class(id)
    }

    /// Create a unit of measure.
    pub fn create_uom(&self, input: CreateUnitOfMeasure) -> Result<UnitOfMeasure> {
        self.ensure()?;
        self.db.units_of_measure().create_uom(input)
    }

    /// List units of measure, optionally scoped to a class.
    ///
    /// A server-side pagination policy applies when the filter has no limit.
    pub fn list_uoms(
        &self,
        filter: stateset_core::UnitOfMeasureFilter,
    ) -> Result<Vec<UnitOfMeasure>> {
        self.ensure()?;
        self.db.units_of_measure().list_uoms(filter)
    }

    /// Mark a UOM as the base unit for its class.
    pub fn set_base_uom(&self, id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        self.ensure()?;
        self.db.units_of_measure().set_base_uom(id)
    }

    /// Delete a unit of measure.
    pub fn delete_uom(&self, id: UnitOfMeasureId) -> Result<()> {
        self.ensure()?;
        self.db.units_of_measure().delete_uom(id)
    }

    /// Create a conversion rule.
    pub fn create_rule(&self, input: CreateUnitConversionRule) -> Result<UnitConversionRule> {
        self.ensure()?;
        self.db.units_of_measure().create_rule(input)
    }

    /// List conversion rules.
    pub fn list_rules(&self) -> Result<Vec<UnitConversionRule>> {
        self.ensure()?;
        self.db.units_of_measure().list_rules()
    }

    /// Delete a conversion rule.
    pub fn delete_rule(&self, id: UnitConversionRuleId) -> Result<()> {
        self.ensure()?;
        self.db.units_of_measure().delete_rule(id)
    }
}
