//! Units of measure  (unit classes, UOMs, conversion rules).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Units of measure  (unit classes, UOMs, conversion rules)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitClassInput {
    pub name: String,
    pub description: Option<String>,
}

/// Window for `listClasses()`; omit for every class.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UnitClassFilterInput {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for `listRules()`; omit for every rule.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct UnitConversionRuleFilterInput {
    /// SYSTEM or SKU
    #[napi(ts_type = "ConversionRuleType")]
    pub rule_type: Option<String>,
    /// Only rules scoped to this product (SKU rules)
    pub product_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitOfMeasureInput {
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    /// Exact decimal string relative to the class base unit
    pub factor: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitOfMeasureFilterInput {
    pub class_id: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateUnitConversionRuleInput {
    /// SYSTEM or SKU
    #[napi(ts_type = "ConversionRuleType")]
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    /// Exact decimal string
    pub factor: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitClassOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub base_uom_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitClass> for UnitClassOutput {
    fn from(c: stateset_core::UnitClass) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            description: c.description,
            base_uom_id: c.base_uom_id.map(|id| id.to_string()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitOfMeasureOutput {
    pub id: String,
    pub unit_class_id: String,
    pub name: String,
    pub abbreviation: String,
    /// Exact decimal string
    pub factor: String,
    pub is_base: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitOfMeasure> for UnitOfMeasureOutput {
    fn from(u: stateset_core::UnitOfMeasure) -> Self {
        Self {
            id: u.id.to_string(),
            unit_class_id: u.unit_class_id.to_string(),
            name: u.name,
            abbreviation: u.abbreviation,
            factor: u.factor.to_string(),
            is_base: u.is_base,
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UnitConversionRuleOutput {
    pub id: String,
    /// SYSTEM or SKU
    #[napi(ts_type = "ConversionRuleType")]
    pub rule_type: String,
    pub product_id: Option<String>,
    pub from_uom_id: String,
    pub to_uom_id: String,
    /// Exact decimal string
    pub factor: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::UnitConversionRule> for UnitConversionRuleOutput {
    fn from(r: stateset_core::UnitConversionRule) -> Self {
        Self {
            id: r.id.to_string(),
            rule_type: r.rule_type.to_string(),
            product_id: r.product_id.map(|id| id.to_string()),
            from_uom_id: r.from_uom_id.to_string(),
            to_uom_id: r.to_uom_id.to_string(),
            factor: r.factor.to_string(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

pub(crate) fn parse_conversion_rule_type(s: &str) -> Result<stateset_core::ConversionRuleType> {
    s.parse::<stateset_core::ConversionRuleType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid conversion rule type: {s}")))
}

#[napi]
pub struct UnitsOfMeasure {
    pub(crate) commerce: Handle,
}

#[napi]
impl UnitsOfMeasure {
    /// Whether the units-of-measure backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.units_of_measure().is_supported())
    }

    #[napi]
    pub async fn create_class(&self, input: CreateUnitClassInput) -> Result<UnitClassOutput> {
        let commerce = self.commerce.get()?;
        let class = commerce
            .units_of_measure()
            .create_class(stateset_core::CreateUnitClass {
                name: input.name,
                description: input.description,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create unit class", e))?;
        Ok(class.into())
    }

    /// List unit classes, optionally windowed by `limit`/`offset`.
    #[napi]
    pub async fn list_classes(
        &self,
        filter: Option<UnitClassFilterInput>,
    ) -> Result<Vec<UnitClassOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let classes = commerce
            .units_of_measure()
            .list_classes()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list unit classes", e))?;
        Ok(page(classes, filter.limit, filter.offset).into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_class(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "unit_class")?;
        commerce
            .units_of_measure()
            .delete_class(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete unit class", e))
    }

    #[napi]
    pub async fn create_uom(&self, input: CreateUnitOfMeasureInput) -> Result<UnitOfMeasureOutput> {
        let commerce = self.commerce.get()?;
        let uom = commerce
            .units_of_measure()
            .create_uom(stateset_core::CreateUnitOfMeasure {
                unit_class_id: parse_uuid_str(&input.unit_class_id, "unit_class_id")?.into(),
                name: input.name,
                abbreviation: input.abbreviation,
                factor: parse_decimal_str(&input.factor, "factor")?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create unit of measure", e))?;
        Ok(uom.into())
    }

    #[napi]
    pub async fn list_uoms(
        &self,
        filter: Option<UnitOfMeasureFilterInput>,
    ) -> Result<Vec<UnitOfMeasureOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::UnitOfMeasureFilter::default()),
            |f| -> Result<stateset_core::UnitOfMeasureFilter> {
                Ok(stateset_core::UnitOfMeasureFilter {
                    class_id: parse_optional_uuid(f.class_id, "class_id")?.map(Into::into),
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let uoms = commerce
            .units_of_measure()
            .list_uoms(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list units of measure", e))?;
        Ok(uoms.into_iter().map(Into::into).collect())
    }

    /// Mark a UOM as the base unit for its class.
    #[napi]
    pub async fn set_base_uom(&self, id: String) -> Result<UnitOfMeasureOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "unit_of_measure")?;
        let uom = commerce
            .units_of_measure()
            .set_base_uom(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set base unit of measure", e))?;
        Ok(uom.into())
    }

    #[napi]
    pub async fn delete_uom(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "unit_of_measure")?;
        commerce
            .units_of_measure()
            .delete_uom(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete unit of measure", e))
    }

    #[napi]
    pub async fn create_rule(
        &self,
        input: CreateUnitConversionRuleInput,
    ) -> Result<UnitConversionRuleOutput> {
        let commerce = self.commerce.get()?;
        let rule = commerce
            .units_of_measure()
            .create_rule(stateset_core::CreateUnitConversionRule {
                rule_type: parse_conversion_rule_type(&input.rule_type)?,
                product_id: parse_optional_uuid(input.product_id, "product_id")?.map(Into::into),
                from_uom_id: parse_uuid_str(&input.from_uom_id, "from_uom_id")?.into(),
                to_uom_id: parse_uuid_str(&input.to_uom_id, "to_uom_id")?.into(),
                factor: parse_decimal_str(&input.factor, "factor")?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create conversion rule", e))?;
        Ok(rule.into())
    }

    /// List conversion rules, optionally filtered by scope / product and
    /// windowed by `limit`/`offset`.
    #[napi]
    pub async fn list_rules(
        &self,
        filter: Option<UnitConversionRuleFilterInput>,
    ) -> Result<Vec<UnitConversionRuleOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let rule_type = filter.rule_type.as_deref().map(parse_conversion_rule_type).transpose()?;
        let product_id: Option<stateset_core::ProductId> =
            parse_optional_id::<uuid::Uuid>(filter.product_id, "product_id")?.map(Into::into);
        let rules = commerce
            .units_of_measure()
            .list_rules()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list conversion rules", e))?;
        let rules: Vec<_> = rules
            .into_iter()
            .filter(|r| rule_type.is_none_or(|t| r.rule_type == t))
            .filter(|r| product_id.is_none_or(|p| r.product_id == Some(p)))
            .collect();
        Ok(page(rules, filter.limit, filter.offset).into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_rule(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "unit_conversion_rule")?;
        commerce
            .units_of_measure()
            .delete_rule(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete conversion rule", e))
    }
}
