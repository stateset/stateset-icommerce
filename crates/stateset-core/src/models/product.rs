//! Product domain models

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductId;
use std::collections::HashSet;
use strum::{Display, EnumString};
use uuid::Uuid;

/// Product entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub status: ProductStatus,
    pub product_type: ProductType,
    pub attributes: Vec<ProductAttribute>,
    pub seo: Option<SeoMetadata>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Product variant (SKU-level)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductVariant {
    pub id: Uuid,
    pub product_id: ProductId,
    pub sku: String,
    pub name: String,
    pub price: Decimal,
    pub compare_at_price: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub barcode: Option<String>,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub options: Vec<VariantOption>,
    pub is_default: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Product status enumeration
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ProductStatus {
    #[default]
    Draft,
    Active,
    Archived,
}

impl ProductStatus {
    /// Whether a product may move from `self` to `next`.
    ///
    /// The catalogue state machine is deliberately small:
    ///
    /// | from       | to                  |
    /// |------------|---------------------|
    /// | `Draft`    | `Active`, `Archived` |
    /// | `Active`   | `Draft`, `Archived`  |
    /// | `Archived` | (terminal)           |
    ///
    /// A same-state transition is always allowed (idempotent updates).
    /// `Archived` is terminal: an archived product has been withdrawn from
    /// sale and its slug may already be reused by a replacement, so it can
    /// never be resurrected in place.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        if matches!((self, next), (Self::Draft, Self::Draft))
            || matches!((self, next), (Self::Active, Self::Active))
            || matches!((self, next), (Self::Archived, Self::Archived))
        {
            return true;
        }
        match self {
            Self::Draft => matches!(next, Self::Active | Self::Archived),
            Self::Active => matches!(next, Self::Draft | Self::Archived),
            Self::Archived => false,
        }
    }

    /// Whether this status is terminal (no outgoing transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Archived)
    }

    /// The catalogue action a status change performs when it withdraws the
    /// product's SKUs from sale, or `None` when nothing becomes unsellable.
    ///
    /// Only `Active` is purchasable ([`Product::is_purchasable`]), so both
    /// `Active -> Draft` (unpublish) and any move to `Archived` take a SKU off
    /// sale while live carts, open orders and reservations may still hold it.
    /// The repositories use the returned verb to run the same
    /// `SkuReferenceCounts::ensure_none` guard for either withdrawal, so
    /// unpublishing can no longer strand a cart holding a SKU that checkout
    /// would go on to accept.
    ///
    /// The match is exhaustive on purpose: a new status must be classified
    /// here rather than falling through a wildcard as "harmless".
    #[must_use]
    pub const fn withdrawal_action(self, next: Self) -> Option<&'static str> {
        match (self, next) {
            // No-ops and publications leave everything sellable.
            (Self::Draft, Self::Draft | Self::Active)
            | (Self::Active, Self::Active)
            | (Self::Archived, Self::Archived) => None,
            // Withdrawals.
            (Self::Active, Self::Draft) => Some("unpublish"),
            (Self::Draft | Self::Active, Self::Archived) => Some("archive"),
            // `Archived` is terminal; `can_transition_to` refuses these first.
            (Self::Archived, Self::Draft | Self::Active) => None,
        }
    }

    /// Validate a transition, returning a typed error when it is not allowed.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CommerceError::ValidationError`] naming both states
    /// when [`Self::can_transition_to`] is false.
    pub fn ensure_can_transition_to(self, next: Self) -> Result<()> {
        if self.can_transition_to(next) {
            Ok(())
        } else {
            Err(crate::CommerceError::ValidationError(format!(
                "product status cannot transition from {self} to {next}"
            )))
        }
    }
}

/// Product type enumeration
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ProductType {
    #[default]
    Simple,
    Variable,
    Bundle,
    Digital,
}

/// Product attribute
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductAttribute {
    pub name: String,
    pub value: String,
    pub group: Option<String>,
    pub is_visible: bool,
    pub is_variation: bool,
}

/// Variant option (e.g., size: Large, color: Blue)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariantOption {
    pub name: String,
    pub value: String,
}

/// SEO metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeoMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
}

/// Input for creating a product
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateProduct {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub product_type: Option<ProductType>,
    pub attributes: Option<Vec<ProductAttribute>>,
    pub seo: Option<SeoMetadata>,
    pub variants: Option<Vec<CreateProductVariant>>,
}

impl Validate for CreateProduct {
    /// Validate a product create request.
    ///
    /// Requires a non-empty product name and validates each supplied variant
    /// (valid SKU, non-negative price/cost). A product may be created without
    /// variants, which are commonly added afterward.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new().required("name", &self.name).build()?;

        let slug = self.slug.clone().unwrap_or_else(|| Product::generate_slug(&self.name));
        ValidationBuilder::new().required("slug", &slug).build()?;

        if let Some(variants) = &self.variants {
            let mut skus = HashSet::with_capacity(variants.len());
            for variant in variants {
                variant.validate()?;
                if !skus.insert(variant.sku.trim().to_owned()) {
                    return Err(crate::CommerceError::ValidationError(format!(
                        "duplicate product variant SKU: {}",
                        variant.sku
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Input for creating a product variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductVariant {
    pub sku: String,
    pub name: Option<String>,
    pub price: Decimal,
    pub compare_at_price: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub barcode: Option<String>,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub options: Option<Vec<VariantOption>>,
    pub is_default: Option<bool>,
}

impl Default for CreateProductVariant {
    fn default() -> Self {
        Self {
            sku: String::new(),
            name: None,
            price: Decimal::ZERO,
            compare_at_price: None,
            cost: None,
            barcode: None,
            weight: None,
            weight_unit: None,
            options: None,
            is_default: None,
        }
    }
}

impl Validate for CreateProductVariant {
    /// Validate a product-variant create request.
    ///
    /// Requires a valid SKU and rejects negative monetary amounts (price, the
    /// optional compare-at price, cost) and a negative weight. A zero price is
    /// permitted (e.g. a free sample); only negative amounts are rejected.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .sku("sku", &self.sku)
            .non_negative("price", self.price)
            .non_negative("compare_at_price", self.compare_at_price.unwrap_or(Decimal::ZERO))
            .non_negative("cost", self.cost.unwrap_or(Decimal::ZERO))
            .non_negative("weight", self.weight.unwrap_or(Decimal::ZERO))
            .custom(
                "price",
                || money_scale_fits_storage(self.price),
                "price cannot carry more than 4 decimal places",
            )
            .custom(
                "compare_at_price",
                || self.compare_at_price.is_none_or(money_scale_fits_storage),
                "compare_at_price cannot carry more than 4 decimal places",
            )
            .custom(
                "cost",
                || self.cost.is_none_or(money_scale_fits_storage),
                "cost cannot carry more than 4 decimal places",
            )
            .custom(
                "compare_at_price",
                || self.compare_at_price.is_none_or(|compare| compare >= self.price),
                "compare_at_price must be greater than or equal to price",
            )
            .build()
    }
}

/// Maximum number of decimal places a variant amount may carry.
///
/// Four fractional digits is the canonical money scale for catalogue amounts:
/// it is what every downstream money column (order and cart lines, invoices,
/// the general ledger) settles at, and it is the precision the SQLite
/// price-range filter compares at. Storage is not the binding constraint —
/// SQLite keeps variant money as exact TEXT and Postgres as unbounded
/// `NUMERIC` since migration 079 — but an amount finer than this cannot
/// survive the first line it is copied onto, so it is refused at the door
/// rather than rounded silently later.
///
/// Enforced by [`CreateProductVariant::validate`], which both the SQLite and
/// the Postgres repository run on every variant write.
pub const VARIANT_MONEY_SCALE: u32 = 4;

/// Whether `amount` survives a round trip through the variant money columns.
#[must_use]
pub fn money_scale_fits_storage(amount: Decimal) -> bool {
    crate::validation::significant_scale(amount) <= VARIANT_MONEY_SCALE
}

/// Input for updating a product
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub status: Option<ProductStatus>,
    pub attributes: Option<Vec<ProductAttribute>>,
    pub seo: Option<SeoMetadata>,
}

/// Product filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProductFilter {
    pub status: Option<ProductStatus>,
    pub product_type: Option<ProductType>,
    pub search: Option<String>,
    /// Matches product attributes with name/group "category" and the given value.
    pub category: Option<String>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub in_stock: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `name` (ASC ordering).
    pub after_cursor: Option<(String, String)>,
}

impl Product {
    /// Generate slug from name if not provided
    #[must_use]
    pub fn generate_slug(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Check if product is purchasable
    #[must_use]
    pub fn is_purchasable(&self) -> bool {
        self.status == ProductStatus::Active
    }
}

/// Why a variant cannot be sold right now.
///
/// Produced by the repositories' purchasability checks (see
/// `variant_is_purchasable_with_conn` in the SQLite/Postgres product
/// repositories) so that cart and order code can refuse a line with a precise
/// reason instead of a generic conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VariantPurchasability {
    /// The SKU exists in the catalogue and can be sold.
    Purchasable,
    /// The SKU is not in the catalogue at all (ad-hoc / external line).
    NotInCatalog,
    /// The variant row is soft-deleted (`is_active = false`).
    VariantInactive,
    /// The parent product is not `Active` (draft or archived).
    ProductNotActive(ProductStatus),
}

impl VariantPurchasability {
    /// Whether a line for this SKU may be added to a cart or order.
    ///
    /// `NotInCatalog` is treated as sellable so that ad-hoc lines (services,
    /// external marketplace SKUs) keep working; only a *known* variant that has
    /// been withdrawn is refused.
    #[must_use]
    pub const fn is_sellable(self) -> bool {
        matches!(self, Self::Purchasable | Self::NotInCatalog)
    }

    /// Convert into a typed error for a `sku` when not sellable.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CommerceError::ValidationError`] naming the SKU and the
    /// reason when [`Self::is_sellable`] is false.
    pub fn ensure_sellable(self, sku: &str) -> Result<()> {
        match self {
            Self::Purchasable | Self::NotInCatalog => Ok(()),
            Self::VariantInactive => Err(crate::CommerceError::ValidationError(format!(
                "SKU {sku} is no longer available (variant deleted)"
            ))),
            Self::ProductNotActive(status) => Err(crate::CommerceError::ValidationError(format!(
                "SKU {sku} is not purchasable: product status is {status}"
            ))),
        }
    }
}

impl ProductVariant {
    /// Calculate profit margin
    #[must_use]
    pub fn profit_margin(&self) -> Option<Decimal> {
        self.cost.map(|cost| {
            if cost > Decimal::ZERO {
                ((self.price - cost) / cost) * Decimal::from(100)
            } else {
                Decimal::ZERO
            }
        })
    }

    /// Check if on sale
    #[must_use]
    pub fn is_on_sale(&self) -> bool {
        self.compare_at_price.is_some_and(|compare| compare > self.price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn withdrawal_action_classifies_every_status_pair() {
        use ProductStatus::{Active, Archived, Draft};

        // Nothing becomes unsellable.
        for (from, to) in [(Draft, Draft), (Draft, Active), (Active, Active), (Archived, Archived)]
        {
            assert_eq!(from.withdrawal_action(to), None, "{from} -> {to}");
        }

        // Withdrawals must be guarded by the caller.
        assert_eq!(Active.withdrawal_action(Draft), Some("unpublish"));
        assert_eq!(Draft.withdrawal_action(Archived), Some("archive"));
        assert_eq!(Active.withdrawal_action(Archived), Some("archive"));

        // Refused transitions never reach the guard.
        assert_eq!(Archived.withdrawal_action(Draft), None);
        assert_eq!(Archived.withdrawal_action(Active), None);
        assert!(!Archived.can_transition_to(Draft));

        // A withdrawal is exactly "was sellable, is not any more".
        for from in [Draft, Active, Archived] {
            for to in [Draft, Active, Archived] {
                let withdraws = from == Active && to != Active;
                let archives = to == Archived && from != Archived;
                assert_eq!(
                    from.withdrawal_action(to).is_some(),
                    (withdraws || archives) && from.can_transition_to(to),
                    "{from} -> {to}"
                );
            }
        }
    }

    // ============================================================================
    // Test Helpers
    // ============================================================================

    #[test]
    fn product_status_display() {
        assert_eq!(ProductStatus::Draft.to_string(), "draft");
        assert_eq!(ProductStatus::Active.to_string(), "active");
        assert_eq!(ProductStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn product_status_from_str() {
        assert_eq!(ProductStatus::from_str("draft").unwrap(), ProductStatus::Draft);
        assert_eq!(ProductStatus::from_str("Active").unwrap(), ProductStatus::Active);
        assert!(ProductStatus::from_str("unknown").is_err());
    }

    #[test]
    fn product_type_from_str() {
        assert_eq!(ProductType::from_str("simple").unwrap(), ProductType::Simple);
        assert_eq!(ProductType::from_str("Bundle").unwrap(), ProductType::Bundle);
        assert!(ProductType::from_str("physical").is_err());
    }

    fn create_test_product(status: ProductStatus) -> Product {
        let now = Utc::now();
        Product {
            id: ProductId::new(),
            name: "Test Product".to_string(),
            slug: "test-product".to_string(),
            description: "A great test product".to_string(),
            status,
            product_type: ProductType::Simple,
            attributes: vec![ProductAttribute {
                name: "Color".to_string(),
                value: "Blue".to_string(),
                group: Some("Appearance".to_string()),
                is_visible: true,
                is_variation: true,
            }],
            seo: Some(SeoMetadata {
                title: Some("Test Product | Store".to_string()),
                description: Some("Buy Test Product".to_string()),
                keywords: vec!["test".to_string(), "product".to_string()],
            }),
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_variant(
        price: Decimal,
        cost: Option<Decimal>,
        compare_at: Option<Decimal>,
    ) -> ProductVariant {
        let now = Utc::now();
        ProductVariant {
            id: Uuid::new_v4(),
            product_id: ProductId::new(),
            sku: "TEST-SKU-001".to_string(),
            name: "Test Variant".to_string(),
            price,
            compare_at_price: compare_at,
            cost,
            barcode: Some("1234567890123".to_string()),
            weight: Some(dec!(0.5)),
            weight_unit: Some("kg".to_string()),
            options: vec![VariantOption { name: "Size".to_string(), value: "Large".to_string() }],
            is_default: true,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    // ============================================================================
    // Product Tests
    // ============================================================================

    #[test]
    fn test_product_generate_slug_simple() {
        let slug = Product::generate_slug("Test Product");
        assert_eq!(slug, "test-product");
    }

    #[test]
    fn test_product_generate_slug_with_special_chars() {
        let slug = Product::generate_slug("Test! Product @ 2024");
        assert_eq!(slug, "test-product-2024");
    }

    #[test]
    fn test_product_generate_slug_with_multiple_spaces() {
        let slug = Product::generate_slug("Test   Product   Name");
        assert_eq!(slug, "test-product-name");
    }

    #[test]
    fn test_product_generate_slug_already_lowercase() {
        let slug = Product::generate_slug("already-lowercase");
        assert_eq!(slug, "already-lowercase");
    }

    #[test]
    fn test_product_is_purchasable_when_active() {
        let product = create_test_product(ProductStatus::Active);
        assert!(product.is_purchasable());
    }

    #[test]
    fn test_product_not_purchasable_when_draft() {
        let product = create_test_product(ProductStatus::Draft);
        assert!(!product.is_purchasable());
    }

    #[test]
    fn test_product_not_purchasable_when_archived() {
        let product = create_test_product(ProductStatus::Archived);
        assert!(!product.is_purchasable());
    }

    // ============================================================================
    // ProductVariant Tests
    // ============================================================================

    #[test]
    fn test_variant_profit_margin_with_cost() {
        let variant = create_test_variant(dec!(100.00), Some(dec!(60.00)), None);
        let margin = variant.profit_margin().unwrap();
        // (100 - 60) / 60 * 100 = 66.666...%
        assert!(margin > dec!(66) && margin < dec!(67));
    }

    #[test]
    fn test_variant_profit_margin_zero_cost() {
        let variant = create_test_variant(dec!(100.00), Some(dec!(0.00)), None);
        let margin = variant.profit_margin().unwrap();
        assert_eq!(margin, dec!(0));
    }

    #[test]
    fn test_variant_profit_margin_no_cost() {
        let variant = create_test_variant(dec!(100.00), None, None);
        assert!(variant.profit_margin().is_none());
    }

    #[test]
    fn test_variant_profit_margin_100_percent() {
        let variant = create_test_variant(dec!(100.00), Some(dec!(50.00)), None);
        let margin = variant.profit_margin().unwrap();
        assert_eq!(margin, dec!(100));
    }

    #[test]
    fn test_variant_is_on_sale_true() {
        let variant = create_test_variant(dec!(79.99), None, Some(dec!(99.99)));
        assert!(variant.is_on_sale());
    }

    #[test]
    fn test_variant_is_on_sale_false_no_compare_price() {
        let variant = create_test_variant(dec!(79.99), None, None);
        assert!(!variant.is_on_sale());
    }

    #[test]
    fn test_variant_is_on_sale_false_same_price() {
        let variant = create_test_variant(dec!(99.99), None, Some(dec!(99.99)));
        assert!(!variant.is_on_sale());
    }

    #[test]
    fn test_variant_is_on_sale_false_compare_lower() {
        let variant = create_test_variant(dec!(99.99), None, Some(dec!(79.99)));
        assert!(!variant.is_on_sale());
    }

    // ============================================================================
    // ProductStatus Tests
    // ============================================================================

    #[test]
    fn test_product_status_default() {
        assert_eq!(ProductStatus::default(), ProductStatus::Draft);
    }

    #[test]
    fn test_product_status_display() {
        assert_eq!(format!("{}", ProductStatus::Draft), "draft");
        assert_eq!(format!("{}", ProductStatus::Active), "active");
        assert_eq!(format!("{}", ProductStatus::Archived), "archived");
    }

    #[test]
    fn test_product_status_serialization() {
        let status = ProductStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"active\"");

        let deserialized: ProductStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    // ============================================================================
    // ProductType Tests
    // ============================================================================

    #[test]
    fn test_product_type_default() {
        assert_eq!(ProductType::default(), ProductType::Simple);
    }

    #[test]
    fn test_product_type_display() {
        assert_eq!(format!("{}", ProductType::Simple), "simple");
        assert_eq!(format!("{}", ProductType::Variable), "variable");
        assert_eq!(format!("{}", ProductType::Bundle), "bundle");
        assert_eq!(format!("{}", ProductType::Digital), "digital");
    }

    #[test]
    fn test_product_type_serialization() {
        let ptype = ProductType::Bundle;
        let json = serde_json::to_string(&ptype).unwrap();
        assert_eq!(json, "\"bundle\"");

        let deserialized: ProductType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ptype);
    }

    // ============================================================================
    // CreateProduct Tests
    // ============================================================================

    #[test]
    fn test_create_product_default() {
        let create = CreateProduct::default();
        assert!(create.name.is_empty());
        assert!(create.slug.is_none());
        assert!(create.description.is_none());
        assert!(create.product_type.is_none());
    }

    // ============================================================================
    // CreateProductVariant Tests
    // ============================================================================

    #[test]
    fn test_create_product_variant_default() {
        let create = CreateProductVariant::default();
        assert!(create.sku.is_empty());
        assert_eq!(create.price, Decimal::ZERO);
        assert!(create.name.is_none());
        assert!(create.cost.is_none());
    }

    // ============================================================================
    // Validation Tests
    // ============================================================================

    fn valid_create_variant() -> CreateProductVariant {
        CreateProductVariant {
            sku: "WIDGET-001".to_string(),
            price: dec!(49.99),
            ..Default::default()
        }
    }

    #[test]
    fn create_product_variant_rejects_negative_price() {
        let variant = CreateProductVariant { price: dec!(-1.00), ..valid_create_variant() };
        let err = variant.validate().expect_err("negative price must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "price")
        );
    }

    #[test]
    fn create_product_variant_accepts_zero_price() {
        // A free sample ($0) is legitimate; only negatives are rejected.
        let variant = CreateProductVariant { price: Decimal::ZERO, ..valid_create_variant() };
        assert!(variant.validate().is_ok());
    }

    #[test]
    fn create_product_variant_rejects_empty_sku() {
        let variant = CreateProductVariant { sku: String::new(), ..valid_create_variant() };
        let err = variant.validate().expect_err("empty sku must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "sku")
        );
    }

    #[test]
    fn create_product_variant_rejects_negative_cost_and_weight() {
        assert!(
            CreateProductVariant { cost: Some(dec!(-5)), ..valid_create_variant() }
                .validate()
                .is_err()
        );
        assert!(
            CreateProductVariant { weight: Some(dec!(-0.5)), ..valid_create_variant() }
                .validate()
                .is_err()
        );
    }

    #[test]
    fn create_product_rejects_empty_name() {
        let input = CreateProduct { name: "  ".to_string(), ..Default::default() };
        let err = input.validate().expect_err("empty product name must be rejected");
        assert!(
            matches!(err, crate::CommerceError::InvalidInput { ref field, .. } if field == "name")
        );
    }

    #[test]
    fn create_product_rejects_variant_with_negative_price() {
        let input = CreateProduct {
            name: "Premium Widget".to_string(),
            variants: Some(vec![CreateProductVariant {
                sku: "WIDGET-001".to_string(),
                price: dec!(-10),
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn create_product_accepts_valid_input() {
        // Bare product (no variants) is valid.
        assert!(
            CreateProduct { name: "Premium Widget".to_string(), ..Default::default() }
                .validate()
                .is_ok()
        );
        // Product with a valid variant is valid.
        let with_variant = CreateProduct {
            name: "Premium Widget".to_string(),
            variants: Some(vec![valid_create_variant()]),
            ..Default::default()
        };
        assert!(with_variant.validate().is_ok());
    }

    #[test]
    fn create_product_variant_rejects_compare_at_below_price() {
        let variant = CreateProductVariant {
            price: dec!(50),
            compare_at_price: Some(dec!(40)),
            ..valid_create_variant()
        };
        let err = variant.validate().expect_err("compare_at below price must be rejected");
        assert!(matches!(
            err,
            crate::CommerceError::InvalidInput { ref field, .. } if field == "compare_at_price"
        ));
        assert!(
            CreateProductVariant { compare_at_price: Some(dec!(50)), ..variant }.validate().is_ok(),
            "equal compare-at is allowed"
        );
    }

    #[test]
    fn create_product_variant_rejects_money_finer_than_storage_scale() {
        for (field, variant) in [
            ("price", CreateProductVariant { price: dec!(1.00001), ..valid_create_variant() }),
            (
                "compare_at_price",
                CreateProductVariant {
                    compare_at_price: Some(dec!(99.99999)),
                    ..valid_create_variant()
                },
            ),
            ("cost", CreateProductVariant { cost: Some(dec!(0.123456)), ..valid_create_variant() }),
        ] {
            let err = variant.validate().expect_err("sub-0.0001 amount must be rejected");
            assert!(
                matches!(err, crate::CommerceError::InvalidInput { field: ref f, .. } if f == field),
                "{field}: {err:?}"
            );
        }
        // Four decimals and trailing zeros survive the NUMERIC(19,4) round trip.
        assert!(
            CreateProductVariant {
                price: dec!(1.2345),
                cost: Some(dec!(1.10000)),
                ..valid_create_variant()
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn product_status_transition_table_is_exhaustive() {
        use ProductStatus::{Active, Archived, Draft};
        let allowed = [
            (Draft, Draft),
            (Draft, Active),
            (Draft, Archived),
            (Active, Active),
            (Active, Draft),
            (Active, Archived),
            (Archived, Archived),
        ];
        for from in [Draft, Active, Archived] {
            for to in [Draft, Active, Archived] {
                let expected = allowed.contains(&(from, to));
                assert_eq!(from.can_transition_to(to), expected, "{from} -> {to}");
                assert_eq!(from.ensure_can_transition_to(to).is_ok(), expected, "{from} -> {to}");
            }
        }
        assert!(Archived.is_terminal());
        assert!(!Active.is_terminal());
        assert!(matches!(
            Archived.ensure_can_transition_to(Active),
            Err(crate::CommerceError::ValidationError(_))
        ));
    }

    #[test]
    fn variant_purchasability_sellability() {
        assert!(VariantPurchasability::Purchasable.is_sellable());
        assert!(VariantPurchasability::NotInCatalog.is_sellable());
        assert!(!VariantPurchasability::VariantInactive.is_sellable());
        assert!(!VariantPurchasability::ProductNotActive(ProductStatus::Archived).is_sellable());
        let err = VariantPurchasability::ProductNotActive(ProductStatus::Draft)
            .ensure_sellable("SKU-1")
            .expect_err("draft product is not sellable");
        assert!(err.to_string().contains("SKU-1"), "{err}");
        assert!(VariantPurchasability::VariantInactive.ensure_sellable("SKU-1").is_err());
    }

    // ============================================================================
    // UpdateProduct Tests
    // ============================================================================

    #[test]
    fn test_update_product_default() {
        let update = UpdateProduct::default();
        assert!(update.name.is_none());
        assert!(update.slug.is_none());
        assert!(update.status.is_none());
    }

    #[test]
    fn test_update_product_partial() {
        let update = UpdateProduct {
            status: Some(ProductStatus::Archived),
            name: Some("Updated Name".to_string()),
            ..Default::default()
        };

        assert_eq!(update.status, Some(ProductStatus::Archived));
        assert_eq!(update.name, Some("Updated Name".to_string()));
        assert!(update.description.is_none());
    }

    // ============================================================================
    // ProductFilter Tests
    // ============================================================================

    #[test]
    fn test_product_filter_default() {
        let filter = ProductFilter::default();
        assert!(filter.status.is_none());
        assert!(filter.product_type.is_none());
        assert!(filter.search.is_none());
        assert!(filter.min_price.is_none());
    }

    #[test]
    fn test_product_filter_with_price_range() {
        let filter = ProductFilter {
            min_price: Some(dec!(10.00)),
            max_price: Some(dec!(100.00)),
            in_stock: Some(true),
            ..Default::default()
        };

        assert_eq!(filter.min_price, Some(dec!(10.00)));
        assert_eq!(filter.max_price, Some(dec!(100.00)));
        assert_eq!(filter.in_stock, Some(true));
    }

    // ============================================================================
    // Serialization Tests
    // ============================================================================

    #[test]
    fn test_product_serialization_roundtrip() {
        let product = create_test_product(ProductStatus::Active);
        let json = serde_json::to_string(&product).unwrap();
        let deserialized: Product = serde_json::from_str(&json).unwrap();
        assert_eq!(product, deserialized);
    }

    #[test]
    fn test_product_variant_serialization_roundtrip() {
        let variant = create_test_variant(dec!(99.99), Some(dec!(50.00)), Some(dec!(129.99)));
        let json = serde_json::to_string(&variant).unwrap();
        let deserialized: ProductVariant = serde_json::from_str(&json).unwrap();
        assert_eq!(variant, deserialized);
    }

    #[test]
    fn test_product_attribute_serialization() {
        let attr = ProductAttribute {
            name: "Material".to_string(),
            value: "Cotton".to_string(),
            group: Some("Fabric".to_string()),
            is_visible: true,
            is_variation: false,
        };

        let json = serde_json::to_string(&attr).unwrap();
        let deserialized: ProductAttribute = serde_json::from_str(&json).unwrap();
        assert_eq!(attr, deserialized);
    }

    #[test]
    fn test_variant_option_serialization() {
        let option = VariantOption { name: "Color".to_string(), value: "Red".to_string() };

        let json = serde_json::to_string(&option).unwrap();
        let deserialized: VariantOption = serde_json::from_str(&json).unwrap();
        assert_eq!(option, deserialized);
    }

    #[test]
    fn test_seo_metadata_serialization() {
        let seo = SeoMetadata {
            title: Some("Great Product".to_string()),
            description: Some("Buy now!".to_string()),
            keywords: vec!["great".to_string(), "product".to_string()],
        };

        let json = serde_json::to_string(&seo).unwrap();
        let deserialized: SeoMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(seo, deserialized);
    }
}
