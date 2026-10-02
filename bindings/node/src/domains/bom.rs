//! Bill of Materials API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Bill of Materials API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBomInput {
    pub name: String,
    pub product_id: String,
    pub description: Option<String>,
    pub revision: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BomOutput {
    pub id: String,
    pub bom_number: String,
    pub name: String,
    pub product_id: String,
    #[napi(ts_type = "BomStatus")]
    pub status: String,
    pub revision: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::BillOfMaterials> for BomOutput {
    fn from(bom: stateset_core::BillOfMaterials) -> Self {
        Self {
            id: bom.id.to_string(),
            bom_number: bom.bom_number,
            name: bom.name,
            product_id: bom.product_id.to_string(),
            status: format!("{}", bom.status),
            revision: bom.revision,
            created_at: bom.created_at.to_rfc3339(),
            updated_at: bom.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBomComponentInput {
    pub component_sku: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit_of_measure: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BomComponentOutput {
    pub id: String,
    pub bom_id: String,
    pub component_sku: Option<String>,
    pub name: String,
    pub quantity: f64,
    pub unit_of_measure: String,
}

impl TryFrom<stateset_core::BomComponent> for BomComponentOutput {
    type Error = Error;

    fn try_from(c: stateset_core::BomComponent) -> Result<Self> {
        Ok(Self {
            id: c.id.to_string(),
            bom_id: c.bom_id.to_string(),
            component_sku: c.component_sku,
            name: c.name,
            quantity: to_f64_checked(c.quantity, "bom component quantity")?,
            unit_of_measure: c.unit_of_measure,
        })
    }
}

#[napi]
pub struct Bom {
    pub(crate) commerce: Handle,
}

#[napi]
impl Bom {
    #[napi]
    pub async fn create(&self, input: CreateBomInput) -> Result<BomOutput> {
        let commerce = self.commerce.get()?;

        let product_id = input
            .product_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid product UUID"))?;

        let bom = commerce
            .bom()
            .create(stateset_core::CreateBom {
                name: input.name,
                product_id,
                description: input.description,
                revision: input.revision,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create BOM", e))?;

        Ok(bom.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<BomOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let bom = commerce
            .bom()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get BOM", e))?;

        Ok(bom.map(|b| b.into()))
    }

    /// List BOMs, optionally filtered/paginated.
    ///
    /// Calling with no argument keeps the previous behaviour (every BOM).
    #[napi]
    pub async fn list(&self, filter: Option<BomFilterInput>) -> Result<Vec<BomOutput>> {
        let commerce = self.commerce.get()?;
        let filter = bom_filter_from_input(filter)?;
        let boms = commerce
            .bom()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list BOMs", e))?;

        Ok(boms.into_iter().map(|b| b.into()).collect())
    }

    #[napi]
    pub async fn add_component(
        &self,
        bom_id: String,
        input: CreateBomComponentInput,
    ) -> Result<BomComponentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = bom_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid BOM UUID"))?;

        let component = commerce
            .bom()
            .add_component(
                uuid,
                stateset_core::CreateBomComponent {
                    component_sku: input.component_sku,
                    name: input.name,
                    quantity: decimal_from_f64(input.quantity, "bom component quantity")?,
                    unit_of_measure: input.unit_of_measure,
                    ..Default::default()
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to add component", e))?;

        convert_output(component)
    }

    #[napi]
    pub async fn get_components(&self, bom_id: String) -> Result<Vec<BomComponentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = bom_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid BOM UUID"))?;

        let components = commerce
            .bom()
            .get_components(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get components", e))?;

        convert_outputs(components)
    }

    #[napi]
    pub async fn activate(&self, id: String) -> Result<BomOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        let bom = commerce
            .bom()
            .activate(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to activate BOM", e))?;

        Ok(bom.into())
    }

    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .bom()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count BOMs", e))?;

        Ok(count as u32)
    }
}
