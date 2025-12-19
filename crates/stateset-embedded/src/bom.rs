//! Bill of Materials (BOM) operations

use stateset_core::{
    BillOfMaterials, BomComponent, BomFilter, CreateBom,
    CreateBomComponent, Result, UpdateBom,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Bill of Materials operations.
///
/// Access via `commerce.bom()`.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::{Commerce, CreateBom, CreateBomComponent};
/// use rust_decimal_macros::dec;
/// use uuid::Uuid;
///
/// let commerce = Commerce::new("./store.db")?;
///
/// // Create a BOM
/// let bom = commerce.bom().create(CreateBom {
///     product_id: Uuid::new_v4(),
///     name: "Widget Assembly".into(),
///     description: Some("Assembly instructions for widget".into()),
///     components: Some(vec![
///         CreateBomComponent {
///             name: "Screw M3x10".into(),
///             component_sku: Some("SCREW-M3-10".into()),
///             quantity: dec!(4),
///             ..Default::default()
///         },
///     ]),
///     ..Default::default()
/// })?;
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct Bom {
    db: Arc<dyn Database>,
}

impl Bom {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new Bill of Materials.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateBom, CreateBomComponent};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let bom = commerce.bom().create(CreateBom {
    ///     product_id: Uuid::new_v4(),
    ///     name: "Widget Assembly".into(),
    ///     components: Some(vec![
    ///         CreateBomComponent {
    ///             name: "Part A".into(),
    ///             quantity: dec!(2),
    ///             ..Default::default()
    ///         },
    ///     ]),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateBom) -> Result<BillOfMaterials> {
        self.db.bom().create(input)
    }

    /// Get a BOM by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get(id)
    }

    /// Get a BOM by its BOM number.
    pub fn get_by_number(&self, bom_number: &str) -> Result<Option<BillOfMaterials>> {
        self.db.bom().get_by_number(bom_number)
    }

    /// Update a BOM.
    pub fn update(&self, id: Uuid, input: UpdateBom) -> Result<BillOfMaterials> {
        self.db.bom().update(id, input)
    }

    /// List BOMs with optional filter.
    pub fn list(&self, filter: BomFilter) -> Result<Vec<BillOfMaterials>> {
        self.db.bom().list(filter)
    }

    /// Delete a BOM (marks as obsolete).
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.bom().delete(id)
    }

    /// Add a component to a BOM.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateBomComponent};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let bom_id = Uuid::new_v4(); // Existing BOM ID
    ///
    /// let component = commerce.bom().add_component(bom_id, CreateBomComponent {
    ///     name: "Resistor 10K".into(),
    ///     component_sku: Some("RES-10K".into()),
    ///     quantity: dec!(4),
    ///     position: Some("R1, R2, R3, R4".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_component(&self, bom_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        self.db.bom().add_component(bom_id, component)
    }

    /// Update a component in a BOM.
    pub fn update_component(&self, component_id: Uuid, component: CreateBomComponent) -> Result<BomComponent> {
        self.db.bom().update_component(component_id, component)
    }

    /// Remove a component from a BOM.
    pub fn remove_component(&self, component_id: Uuid) -> Result<()> {
        self.db.bom().remove_component(component_id)
    }

    /// Get all components for a BOM.
    pub fn get_components(&self, bom_id: Uuid) -> Result<Vec<BomComponent>> {
        self.db.bom().get_components(bom_id)
    }

    /// Activate a BOM (make it ready for production use).
    ///
    /// Changes the BOM status from Draft to Active.
    pub fn activate(&self, id: Uuid) -> Result<BillOfMaterials> {
        self.db.bom().activate(id)
    }

    /// Count BOMs matching filter.
    pub fn count(&self, filter: BomFilter) -> Result<u64> {
        self.db.bom().count(filter)
    }

    /// Get BOMs for a specific product.
    pub fn for_product(&self, product_id: Uuid) -> Result<Vec<BillOfMaterials>> {
        self.list(BomFilter {
            product_id: Some(product_id),
            ..Default::default()
        })
    }
}
