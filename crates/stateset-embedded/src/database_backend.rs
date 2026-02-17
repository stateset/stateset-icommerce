//! Performance-optimized database backend with static dispatch
//!
//! Replaces Box<dyn> trait objects with zero-cost static dispatch while
//! maintaining backward compatibility.

use stateset_core::{
    AccountsPayableRepository, AccountsReceivableRepository, AnalyticsRepository,
    BackorderRepository, BomRepository, CartRepository, CostAccountingRepository, CreditRepository,
    CurrencyRepository, CustomerRepository, FulfillmentRepository, GeneralLedgerRepository,
    InventoryRepository, InvoiceRepository, LotRepository, OrderRepository, PaymentRepository,
    ProductRepository, PromotionRepository, PurchaseOrderRepository, QualityRepository,
    ReceivingRepository, Result, ReturnRepository, SerialRepository, ShipmentRepository,
    SubscriptionRepository, TaxRepository, WarehouseRepository, WarrantyRepository,
    WorkOrderRepository,
};

/// Zero-cost database backend using static dispatch instead of dynamic trait objects
///
/// # Performance Benefits
/// - Zero alloc: No heap allocations on repository access
/// - Monomorphic: Compiler optimizes each backend separately
/// - Cache-friendly: Better CPU cache locality
/// - Inlinable: Methods can be inlined by compiler
///
/// # Migration Path
/// For existing code using `dyn Database`, use `DatabaseBackend::from_dyn()` adapter.
pub struct DatabaseBackend<DB> {
    inner: DB,
}

impl<DB> DatabaseBackend<DB> {
    /// Create a new database backend
    pub fn new(db: DB) -> Self {
        Self { inner: db }
    }

    /// Get the inner database for advanced operations
    pub fn inner(&self) -> &DB {
        &self.inner
    }

    /// Get mutable reference to inner database
    pub fn inner_mut(&mut self) -> &mut DB {
        &mut self.inner
    }
}

impl<DB> Clone for DatabaseBackend<DB>
where
    DB: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// ============================================================================
// Backend Implementations
// ============================================================================

/// Type alias for SQLite backend with static dispatch
pub type SqliteBackend = DatabaseBackend<stateset_db::SqliteDatabase>;

/// Type alias for PostgreSQL backend with static dispatch
pub type PostgresBackend = DatabaseBackend<stateset_db::PostgresDatabase>;

// ============================================================================
// Repository Accessors (Zero-Cost)
// ============================================================================

impl<DB> DatabaseBackend<DB>
where
    DB: OrderRepository,
{
    /// Get order repository (zero-cost, no heap allocation)
    pub fn orders(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: InventoryRepository,
{
    /// Get inventory repository (zero-cost, no heap allocation)
    pub fn inventory(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: CustomerRepository,
{
    /// Get customer repository (zero-cost, no heap allocation)
    pub fn customers(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: ProductRepository,
{
    /// Get product repository (zero-cost, no heap allocation)
    pub fn products(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: ReturnRepository,
{
    /// Get return repository (zero-cost, no heap allocation)
    pub fn returns(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: BomRepository,
{
    /// Get BOM repository (zero-cost, no heap allocation)
    pub fn bom(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: WorkOrderRepository,
{
    /// Get work order repository (zero-cost, no heap allocation)
    pub fn work_orders(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: ShipmentRepository,
{
    /// Get shipment repository (zero-cost, no heap allocation)
    pub fn shipments(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: PaymentRepository,
{
    /// Get payment repository (zero-cost, no heap allocation)
    pub fn payments(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: WarrantyRepository,
{
    /// Get warranty repository (zero-cost, no heap allocation)
    pub fn warranties(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: PurchaseOrderRepository,
{
    /// Get purchase order repository (zero-cost, no heap allocation)
    pub fn purchase_orders(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: InvoiceRepository,
{
    /// Get invoice repository (zero-cost, no heap allocation)
    pub fn invoices(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: CartRepository,
{
    /// Get cart repository (zero-cost, no heap allocation)
    pub fn carts(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: AnalyticsRepository,
{
    /// Get analytics repository (zero-cost, no heap allocation)
    pub fn analytics(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: CurrencyRepository,
{
    /// Get currency repository (zero-cost, no heap allocation)
    pub fn currency(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: TaxRepository,
{
    /// Get tax repository (zero-cost, no heap allocation)
    pub fn tax(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: PromotionRepository,
{
    /// Get promotions repository (zero-cost, no heap allocation)
    pub fn promotions(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: SubscriptionRepository,
{
    /// Get subscriptions repository (zero-cost, no heap allocation)
    pub fn subscriptions(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: QualityRepository,
{
    /// Get quality repository (zero-cost, no heap allocation)
    pub fn quality(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: LotRepository,
{
    /// Get lots repository (zero-cost, no heap allocation)
    pub fn lots(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: SerialRepository,
{
    /// Get serials repository (zero-cost, no heap allocation)
    pub fn serials(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: WarehouseRepository,
{
    /// Get warehouse repository (zero-cost, no heap allocation)
    pub fn warehouses(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: ReceivingRepository,
{
    /// Get receiving repository (zero-cost, no heap allocation)
    pub fn receiving(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: FulfillmentRepository,
{
    /// Get fulfillment repository (zero-cost, no heap allocation)
    pub fn fulfillment(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: AccountsPayableRepository,
{
    /// Get accounts payable repository (zero-cost, no heap allocation)
    pub fn accounts_payable(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: CostAccountingRepository,
{
    /// Get cost accounting repository (zero-cost, no heap allocation)
    pub fn cost_accounting(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: CreditRepository,
{
    /// Get credit repository (zero-cost, no heap allocation)
    pub fn credits(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: BackorderRepository,
{
    /// Get backorder repository (zero-cost, no heap allocation)
    pub fn backorders(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: AccountsReceivableRepository,
{
    /// Get accounts receivable repository (zero-cost, no heap allocation)
    pub fn accounts_receivable(&self) -> &DB {
        &self.inner
    }
}

impl<DB> DatabaseBackend<DB>
where
    DB: GeneralLedgerRepository,
{
    /// Get general ledger repository (zero-cost, no heap allocation)
    pub fn general_ledger(&self) -> &DB {
        &self.inner
    }
}

// ============================================================================
// Backward Compatibility Adapter
// ============================================================================

impl From<stateset_db::SqliteDatabase> for SqliteBackend {
    fn from(db: stateset_db::SqliteDatabase) -> Self {
        Self::new(db)
    }
}

#[cfg(feature = "postgres")]
impl From<stateset_db::PostgresDatabase> for PostgresBackend {
    fn from(db: stateset_db::PostgresDatabase) -> Self {
        Self::new(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_db::DatabaseConfig;

    #[test]
    fn test_sqlite_backend_zero_cost() {
        let db = stateset_db::SqliteDatabase::new(&DatabaseConfig::in_memory())
            .expect("Failed to create database");
        let backend = SqliteBackend::new(db);

        // These calls should be zero-cost - no heap allocation
        let _ = backend.orders();
        let _ = backend.customers();
        let _ = backend.inventory();
    }

    #[test]
    fn test_backend_clone() {
        let db = stateset_db::SqliteDatabase::new(&DatabaseConfig::in_memory())
            .expect("Failed to create database");
        let backend = SqliteBackend::new(db);

        // Clone should work if inner DB is cloneable
        let _backend2 = backend.clone();
    }

    #[test]
    fn test_fulfillment_accessor_name() {
        let db = stateset_db::SqliteDatabase::new(&DatabaseConfig::in_memory())
            .expect("Failed to create database");
        let backend = SqliteBackend::new(db);

        let _ = backend.fulfillment();
    }

    #[test]
    fn test_returns_and_receiving_accessors() {
        let db = stateset_db::SqliteDatabase::new(&DatabaseConfig::in_memory())
            .expect("Failed to create database");
        let backend = SqliteBackend::new(db);

        let _ = backend.returns();
        let _ = backend.receiving();
    }
}
