//! Products API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Products API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductVariantInput {
    pub sku: String,
    pub name: Option<String>,
    /// Float price. Optional: send `price_exact` instead for exact money.
    pub price: Option<f64>,
    /// Exact base-10 price. Takes precedence over `price` when present.
    pub price_exact: Option<String>,
    pub compare_at_price: Option<f64>,
    /// Exact base-10 comparison price. Takes precedence over `compare_at_price` when present.
    pub compare_at_price_exact: Option<String>,
    pub is_default: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateProductInput {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub variants: Option<Vec<CreateProductVariantInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductOutput {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub category: Option<String>,
    #[napi(ts_type = "ProductStatus")]
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Product> for ProductOutput {
    fn from(p: stateset_core::Product) -> Self {
        let category = p
            .attributes
            .iter()
            .find(|attribute| attribute.name.eq_ignore_ascii_case("category"))
            .map(|attribute| attribute.value.clone());
        Self {
            id: p.id.to_string(),
            name: p.name,
            slug: p.slug,
            description: p.description,
            category,
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ProductVariantOutput {
    pub id: String,
    pub product_id: String,
    pub sku: String,
    pub name: String,
    /// @deprecated Use the `priceExact` twin; float money will be removed in 2.0.
    pub price: f64,
    /// Exact base-10 price. Prefer this field for calculations.
    pub price_exact: String,
    /// @deprecated Use the `compareAtPriceExact` twin; float money will be removed in 2.0.
    pub compare_at_price: Option<f64>,
    /// Exact base-10 comparison price.
    pub compare_at_price_exact: Option<String>,
    pub is_default: bool,
}

impl TryFrom<stateset_core::ProductVariant> for ProductVariantOutput {
    type Error = Error;

    fn try_from(v: stateset_core::ProductVariant) -> Result<Self> {
        let (price, price_exact) = money_pair(v.price, "variant price")?;
        let (compare_at_price, compare_at_price_exact) =
            optional_money_pair(v.compare_at_price, "variant compare-at price")?;
        Ok(Self {
            id: v.id.to_string(),
            product_id: v.product_id.to_string(),
            sku: v.sku,
            name: v.name,
            price,
            price_exact,
            compare_at_price,
            compare_at_price_exact,
            is_default: v.is_default,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateProductInput {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    #[napi(ts_type = "ProductStatus")]
    pub status: Option<String>,
}

pub(crate) fn create_variant_from_input(
    v: CreateProductVariantInput,
) -> Result<stateset_core::CreateProductVariant> {
    Ok(stateset_core::CreateProductVariant {
        sku: v.sku,
        name: v.name,
        price: money_input(v.price_exact.as_deref(), v.price, "variant price")?,
        compare_at_price: optional_money_input(
            v.compare_at_price_exact.as_deref(),
            v.compare_at_price,
            "variant compare at price",
        )?,
        is_default: v.is_default,
        ..Default::default()
    })
}

#[napi]
pub struct Products {
    pub(crate) commerce: Handle,
}

#[napi]
impl Products {
    #[napi]
    pub async fn create(&self, input: CreateProductInput) -> Result<ProductOutput> {
        let commerce = self.commerce.get()?;

        let variants = input
            .variants
            .map(|vs| {
                vs.into_iter()
                    .map(|v| {
                        Ok(stateset_core::CreateProductVariant {
                            sku: v.sku,
                            name: v.name,
                            price: money_input(v.price_exact.as_deref(), v.price, "variant price")?,
                            compare_at_price: optional_money_input(
                                v.compare_at_price_exact.as_deref(),
                                v.compare_at_price,
                                "variant compare at price",
                            )?,
                            is_default: v.is_default,
                            ..Default::default()
                        })
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        let product = commerce
            .products()
            .create(stateset_core::CreateProduct {
                name: input.name,
                slug: input.slug,
                description: input.description,
                attributes: input.category.map(|category| {
                    vec![stateset_core::ProductAttribute {
                        name: "category".to_string(),
                        value: category,
                        group: Some("storefront".to_string()),
                        is_visible: true,
                        is_variation: false,
                    }]
                }),
                variants,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create product", e))?;

        Ok(product.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<ProductOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let product = commerce
            .products()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get product", e))?;

        Ok(product.map(|p| p.into()))
    }

    #[napi]
    pub async fn get_variant_by_sku(&self, sku: String) -> Result<Option<ProductVariantOutput>> {
        let commerce = self.commerce.get()?;
        let variant = commerce
            .products()
            .get_variant_by_sku(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get variant", e))?;

        convert_optional_output(variant)
    }

    /// List products, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every product).
    #[napi]
    pub async fn list(&self, filter: Option<ProductFilterInput>) -> Result<Vec<ProductOutput>> {
        let commerce = self.commerce.get()?;
        let filter = product_filter_from_input(filter)?;
        let products = commerce
            .products()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list products", e))?;

        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .products()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count products", e))?;

        Ok(count as u32)
    }

    #[napi]
    pub async fn update(&self, id: String, input: UpdateProductInput) -> Result<ProductOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let status = input
            .status
            .map(|s| s.parse::<stateset_core::ProductStatus>())
            .transpose()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product status"))?;
        let product = commerce
            .products()
            .update(
                uuid.into(),
                stateset_core::UpdateProduct {
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    status,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update product", e))?;
        Ok(product.into())
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .products()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete product", e))?;
        Ok(())
    }

    #[napi]
    pub async fn get_by_slug(&self, slug: String) -> Result<Option<ProductOutput>> {
        let commerce = self.commerce.get()?;
        let product = commerce
            .products()
            .get_by_slug(&slug)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get product", e))?;
        Ok(product.map(|p| p.into()))
    }

    #[napi]
    pub async fn activate(&self, id: String) -> Result<ProductOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let product = commerce
            .products()
            .activate(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to activate product", e))?;
        Ok(product.into())
    }

    #[napi]
    pub async fn archive(&self, id: String) -> Result<ProductOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let product = commerce
            .products()
            .archive(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to archive product", e))?;
        Ok(product.into())
    }

    #[napi]
    pub async fn search(&self, query: String) -> Result<Vec<ProductOutput>> {
        let commerce = self.commerce.get()?;
        let products = commerce
            .products()
            .search(&query)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to search products", e))?;
        Ok(products.into_iter().map(|p| p.into()).collect())
    }

    #[napi]
    pub async fn get_variant(&self, id: String) -> Result<Option<ProductVariantOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let variant = commerce
            .products()
            .get_variant(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get variant", e))?;
        convert_optional_output(variant)
    }

    #[napi]
    pub async fn get_variants(&self, product_id: String) -> Result<Vec<ProductVariantOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let variants = commerce
            .products()
            .get_variants(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get variants", e))?;
        variants.into_iter().map(|v| v.try_into()).collect::<Result<Vec<_>>>()
    }

    #[napi]
    pub async fn add_variant(
        &self,
        product_id: String,
        input: CreateProductVariantInput,
    ) -> Result<ProductVariantOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            product_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let variant = commerce
            .products()
            .add_variant(uuid.into(), create_variant_from_input(input)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add variant", e))?;
        variant.try_into()
    }

    #[napi]
    pub async fn update_variant(
        &self,
        id: String,
        input: CreateProductVariantInput,
    ) -> Result<ProductVariantOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let variant = commerce
            .products()
            .update_variant(uuid, create_variant_from_input(input)?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update variant", e))?;
        variant.try_into()
    }

    #[napi]
    pub async fn delete_variant(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .products()
            .delete_variant(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete variant", e))?;
        Ok(())
    }
}
