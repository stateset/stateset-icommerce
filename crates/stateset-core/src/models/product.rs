//! Product domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Product entity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
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
    pub product_id: Uuid,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Active => write!(f, "active"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

/// Product type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for ProductType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Simple => write!(f, "simple"),
            Self::Variable => write!(f, "variable"),
            Self::Bundle => write!(f, "bundle"),
            Self::Digital => write!(f, "digital"),
        }
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProduct {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub product_type: Option<ProductType>,
    pub attributes: Option<Vec<ProductAttribute>>,
    pub seo: Option<SeoMetadata>,
    pub variants: Option<Vec<CreateProductVariant>>,
}

impl Default for CreateProduct {
    fn default() -> Self {
        Self {
            name: String::new(),
            slug: None,
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        }
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
        self.compare_at_price
            .map(|compare| compare > self.price)
            .unwrap_or(false)
    }
}
