//! Product domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductId;
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ProductStatus {
    Draft,
    Active,
    Archived,
}

impl Default for ProductStatus {
    fn default() -> Self {
        Self::Draft
    }
}

/// Product type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ProductType {
    Simple,
    Variable,
    Bundle,
    Digital,
}

impl Default for ProductType {
    fn default() -> Self {
        Self::Simple
    }
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
}

impl Product {
    /// Generate slug from name if not provided
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
    pub fn is_purchasable(&self) -> bool {
        self.status == ProductStatus::Active
    }
}

impl ProductVariant {
    /// Calculate profit margin
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
    pub fn is_on_sale(&self) -> bool {
        self.compare_at_price.map(|compare| compare > self.price).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

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
