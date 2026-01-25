//! # Database trait improvements
//!
//! This module provides performance improvements by avoiding dynamic dispatch
//! and Box<dyn> trait objects. Use the generic Database type for compile-time
//! optimization.

use crate::{PostgresDatabase, SqliteDatabase};
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

/// Type-erased database trait for runtime flexibility.
/// This is the old API maintained for backward compatibility.
#[cfg(feature = "legacy-database-trait")]
pub trait Database: Send + Sync {
    fn orders(&self) -> Box<dyn OrderRepository + '_>;
    fn inventory(&self) -> Box<dyn InventoryRepository + '_>;
    fn customers(&self) -> Box<dyn CustomerRepository + '_>;
    fn products(&self) -> Box<dyn ProductRepository + '_>;
    fn returns(&self) -> Box<dyn ReturnRepository + '_>;
    fn bom(&self) -> Box<dyn BomRepository + '_>;
    fn work_orders(&self) -> Box<dyn WorkOrderRepository + '_>;
    fn shipments(&self) -> Box<dyn ShipmentRepository + '_>;
    fn payments(&self) -> Box<dyn PaymentRepository + '_>;
    fn warranties(&self) -> Box<dyn WarrantyRepository + '_>;
    fn purchase_orders(&self) -> Box<dyn PurchaseOrderRepository + '_>;
    fn invoices(&self) -> Box<dyn InvoiceRepository + '_>;
    fn carts(&self) -> Box<dyn CartRepository + '_>;
    fn analytics(&self) -> Box<dyn AnalyticsRepository + '_>;
    fn currency(&self) -> Box<dyn CurrencyRepository + '_>;
    fn tax(&self) -> Box<dyn TaxRepository + '_>;
    fn promotions(&self) -> Box<dyn PromotionRepository + '_>;
    fn subscriptions(&self) -> Box<dyn SubscriptionRepository + '_>;
    fn quality(&self) -> Box<dyn QualityRepository + '_>;
    fn lots(&self) -> Box<dyn LotRepository + '_>;
    fn serials(&self) -> Box<dyn SerialRepository + '_>;
    fn warehouse(&self) -> Box<dyn WarehouseRepository + '_>;
    fn receiving(&self) -> Box<dyn ReceivingRepository + '_>;
    fn fulfillment(&self) -> Box<dyn FulfillmentRepository + '_>;
    fn accounts_payable(&self) -> Box<dyn AccountsPayableRepository + '_>;
    fn cost_accounting(&self) -> Box<dyn CostAccountingRepository + '_>;
    fn credit(&self) -> Box<dyn CreditRepository + '_>;
    fn backorder(&self) -> Box<dyn BackorderRepository + '_>;
    fn accounts_receivable(&self) -> Box<dyn AccountsReceivableRepository + '_>;
    fn general_ledger(&self) -> Box<dyn GeneralLedgerRepository + '_>;
}

/// Generic database accessor using static dispatch for zero-cost abstractions.
///
/// This is the NEW preferred API - use this for best performance.
///
/// # Example
/// ```
/// use stateset_db::DatabaseAccessor;
/// use stateset_db::SqliteDatabase;
///
/// let db = SqliteDatabase::new(&config)?;
/// let accessor = DatabaseAccessor::new(&db);
///
/// // Zero-cost static dispatch - no heap allocations
/// let order = accessor.orders().get(id)?;
/// ```
#[derive(Clone, Copy)]
pub struct DatabaseAccessor<'a, DB> {
    db: &'a DB,
}

impl<'a, DB> DatabaseAccessor<'a, DB> {
    pub fn new(db: &'a DB) -> Self {
        Self { db }
    }

    pub fn orders(&self) -> DB::OrdersRepo {
        self.db.orders_repo()
    }

    pub fn inventory(&self) -> DB::InventoryRepo {
        self.db.inventory_repo()
    }

    pub fn customers(&self) -> DB::CustomersRepo {
        self.db.customers_repo()
    }

    pub fn products(&self) -> DB::ProductsRepo {
        self.db.products_repo()
    }

    pub fn returns(&self) -> DB::ReturnsRepo {
        self.db.returns_repo()
    }

    pub fn bom(&self) -> DB::BomRepo {
        self.db.bom_repo()
    }

    pub fn work_orders(&self) -> DB::WorkOrdersRepo {
        self.db.work_orders_repo()
    }

    pub fn shipments(&self) -> DB::ShipmentsRepo {
        self.db.shipments_repo()
    }

    pub fn payments(&self) -> DB::PaymentsRepo {
        self.db.payments_repo()
    }

    pub fn warranties(&self) -> DB::WarrantiesRepo {
        self.db.warranties_repo()
    }

    pub fn purchase_orders(&self) -> DB::PurchaseOrdersRepo {
        self.db.purchase_orders_repo()
    }

    pub fn invoices(&self) -> DB::InvoicesRepo {
        self.db.invoices_repo()
    }

    pub fn carts(&self) -> DB::CartsRepo {
        self.db.carts_repo()
    }

    pub fn analytics(&self) -> DB::AnalyticsRepo {
        self.db.analytics_repo()
    }

    pub fn currency(&self) -> DB::CurrencyRepo {
        self.db.currency_repo()
    }

    pub fn tax(&self) -> DB::TaxRepo {
        self.db.tax_repo()
    }

    pub fn promotions(&self) -> DB::PromotionsRepo {
        self.db.promotions_repo()
    }

    pub fn subscriptions(&self) -> DB::SubscriptionsRepo {
        self.db.subscriptions_repo()
    }

    pub fn quality(&self) -> DB::QualityRepo {
        self.db.quality_repo()
    }

    pub fn lots(&self) -> DB::LotsRepo {
        self.db.lots_repo()
    }

    pub fn serials(&self) -> DB::SerialsRepo {
        self.db.serials_repo()
    }

    pub fn warehouse(&self) -> DB::WarehouseRepo {
        self.db.warehouse_repo()
    }

    pub fn receiving(&self) -> DB::ReceivingRepo {
        self.db.receiving_repo()
    }

    pub fn fulfillment(&self) -> DB::FulfillmentRepo {
        self.db.fulfillment_repo()
    }

    pub fn accounts_payable(&self) -> DB::AccountsPayableRepo {
        self.db.accounts_payable_repo()
    }

    pub fn cost_accounting(&self) -> DB::CostAccountingRepo {
        self.db.cost_accounting_repo()
    }

    pub fn credit(&self) -> DB::CreditRepo {
        self.db.credit_repo()
    }

    pub fn backorder(&self) -> DB::BackorderRepo {
        self.db.backorder_repo()
    }

    pub fn accounts_receivable(&self) -> DB::AccountsReceivableRepo {
        self.db.accounts_receivable_repo()
    }

    pub fn general_ledger(&self) -> DB::GeneralLedgerRepo {
        self.db.general_ledger_repo()
    }
}

/// Repository accessor trait - must be implemented by each database type.
///
/// This allows us to use static dispatch instead of dynamic dispatch.
/// Each database implementation (SQLite, PostgreSQL) implements this trait
/// to return concrete repository types.
pub trait RepositoryAccessor {
    type OrdersRepo: OrderRepository;
    type InventoryRepo: InventoryRepository;
    type CustomersRepo: CustomerRepository;
    type ProductsRepo: ProductRepository;
    type ReturnsRepo: ReturnRepository;
    type BomRepo: BomRepository;
    type WorkOrdersRepo: WorkOrderRepository;
    type ShipmentsRepo: ShipmentRepository;
    type PaymentsRepo: PaymentRepository;
    type WarrantiesRepo: WarrantyRepository;
    type PurchaseOrdersRepo: PurchaseOrderRepository;
    type InvoicesRepo: InvoiceRepository;
    type CartsRepo: CartRepository;
    type AnalyticsRepo: AnalyticsRepository;
    type CurrencyRepo: CurrencyRepository;
    type TaxRepo: TaxRepository;
    type PromotionsRepo: PromotionRepository;
    type SubscriptionsRepo: SubscriptionRepository;
    type QualityRepo: QualityRepository;
    type LotsRepo: LotRepository;
    type SerialsRepo: SerialRepository;
    type WarehouseRepo: WarehouseRepository;
    type ReceivingRepo: ReceivingRepository;
    type FulfillmentRepo: FulfillmentRepository;
    type AccountsPayableRepo: AccountsPayableRepository;
    type CostAccountingRepo: CostAccountingRepository;
    type CreditRepo: CreditRepository;
    type BackorderRepo: BackorderRepository;
    type AccountsReceivableRepo: AccountsReceivableRepository;
    type GeneralLedgerRepo: GeneralLedgerRepository;

    fn orders_repo(&self) -> Self::OrdersRepo;
    fn inventory_repo(&self) -> Self::InventoryRepo;
    fn customers_repo(&self) -> Self::CustomersRepo;
    fn products_repo(&self) -> Self::ProductsRepo;
    fn returns_repo(&self) -> Self::ReturnsRepo;
    fn bom_repo(&self) -> Self::BomRepo;
    fn work_orders_repo(&self) -> Self::WorkOrdersRepo;
    fn shipments_repo(&self) -> Self::ShipmentsRepo;
    fn payments_repo(&self) -> Self::PaymentsRepo;
    fn warranties_repo(&self) -> Self::WarrantiesRepo;
    fn purchase_orders_repo(&self) -> Self::PurchaseOrdersRepo;
    fn invoices_repo(&self) -> Self::InvoicesRepo;
    fn carts_repo(&self) -> Self::CartsRepo;
    fn analytics_repo(&self) -> Self::AnalyticsRepo;
    fn currency_repo(&self) -> Self::CurrencyRepo;
    fn tax_repo(&self) -> Self::TaxRepo;
    fn promotions_repo(&self) -> Self::PromotionsRepo;
    fn subscriptions_repo(&self) -> Self::SubscriptionsRepo;
    fn quality_repo(&self) -> Self::QualityRepo;
    fn lots_repo(&self) -> Self::LotsRepo;
    fn serials_repo(&self) -> Self::SerialsRepo;
    fn warehouse_repo(&self) -> Self::WarehouseRepo;
    fn receiving_repo(&self) -> Self::ReceivingRepo;
    fn fulfillment_repo(&self) -> Self::FulfillmentRepo;
    fn accounts_payable_repo(&self) -> Self::AccountsPayableRepo;
    fn cost_accounting_repo(&self) -> Self::CostAccountingRepo;
    fn credit_repo(&self) -> Self::CreditRepo;
    fn backorder_repo(&self) -> Self::BackorderRepo;
    fn accounts_receivable_repo(&self) -> Self::AccountsReceivableRepo;
    fn general_ledger_repo(&self) -> Self::GeneralLedgerRepo;
}

/// Convenience constructor for DatabaseAccessor
///
/// # Example
/// ```
/// use stateset_db::accessor;
///
/// let db = SqliteDatabase::new(&config)?;
/// let accessor = accessor(&db);
/// ```
#[inline]
pub fn accessor<DB>(db: &DB) -> DatabaseAccessor<DB> {
    DatabaseAccessor::new(db)
}
