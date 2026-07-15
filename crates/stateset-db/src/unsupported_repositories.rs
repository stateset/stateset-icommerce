//! Placeholder repositories for backends that do not implement a domain.
//!
//! The B2B / ERP-ops domains (channels, companies, transfer orders, units of
//! measure, production batches) currently have a SQLite backend only. The
//! Postgres `NewDomainRepositoryFactory` returns these shims, which reject every
//! operation with [`CommerceError::NotPermitted`]. The corresponding
//! [`DatabaseCapability`](crate::DatabaseCapability) reports `false` for Postgres,
//! so the embedded layer's capability gate normally short-circuits before these
//! are ever called — they exist to satisfy the trait's return type.

use rust_decimal::Decimal;
use stateset_core::{
    ActivityLogEntry, ActivityLogFilter, ActivityLogId, ActivityLogRepository, ApplyPrepayment,
    ApplyVendorCredit, BulkSupplierSkuItem, CaptureStockSnapshot, CaptureTopologySnapshot, Channel,
    ChannelFilter, ChannelId, ChannelProductMapping, ChannelProductSyncItem, ChannelRepository,
    CommerceError, Company, CompanyFilter, CompanyId, CompanyPriceOverride, CompanyRepository,
    CompanyShippingAddress, Contact, ContactId, CreateChannel, CreateCompany, CreateContact,
    CreateEdiDocument, CreateInboundShipment, CreateIntegrationFieldMapping,
    CreateIntegrationMapping, CreatePaymentObligation, CreatePrepayment, CreatePriceLevel,
    CreatePriceSchedule, CreatePrintStation, CreateProductionBatch, CreateSupplierSku,
    CreateTransferOrder, CreateUnitClass, CreateUnitConversionRule, CreateUnitOfMeasure,
    CreateVendorCredit, CreateVendorReturn, EdiAggregateSummary, EdiDocument, EdiDocumentFilter,
    EdiDocumentId, EdiDocumentRepository, EdiStatus, EnqueuePrintJob, InboundShipment,
    InboundShipmentFilter, InboundShipmentId, InboundShipmentItemId, InboundShipmentRepository,
    IngestOrder, IntegrationFieldMapping, IntegrationFieldMappingFilter, IntegrationFieldMappingId,
    IntegrationFieldMappingRepository, IntegrationMapping, IntegrationMappingFilter,
    IntegrationMappingId, IntegrationMappingRepository, MapPurgatoryLine, MappingLookup,
    PairStationResult, PaymentObligation, PaymentObligationDashboard, PaymentObligationFilter,
    PaymentObligationId, PaymentObligationRepository, PaymentObligationStatus, Prepayment,
    PrepaymentApplication, PrepaymentApplicationId, PrepaymentFilter, PrepaymentId,
    PrepaymentRepository, PriceLevel, PriceLevelEntry, PriceLevelFilter, PriceLevelId,
    PriceLevelRepository, PriceSchedule, PriceScheduleEntry, PriceScheduleFilter, PriceScheduleId,
    PriceScheduleRepository, PrintJob, PrintJobFilter, PrintJobId, PrintStation, PrintStationId,
    PrintStationRepository, ProductId, ProductionBatch, ProductionBatchFilter, ProductionBatchId,
    ProductionBatchRepository, PurgatoryFilter, PurgatoryLineItemId, PurgatoryOrder,
    PurgatoryOrderId, PurgatoryRepository, RecordActivity, Result, StockSnapshot,
    StockSnapshotFilter, StockSnapshotId, StockSnapshotRepository, SupplierSku, SupplierSkuFilter,
    SupplierSkuId, SupplierSkuRepository, TopologySnapshot, TopologySnapshotFilter,
    TopologySnapshotId, TopologySnapshotRepository, TransferOrder, TransferOrderFilter,
    TransferOrderId, TransferOrderItemId, TransferOrderRepository, UnitClass, UnitClassId,
    UnitConversionRule, UnitConversionRuleId, UnitOfMeasure, UnitOfMeasureId,
    UnitOfMeasureRepository, UpdateChannel, UpdateCompany, UpdateIntegrationFieldMapping,
    UpdateIntegrationMapping, UpdatePriceLevel, UpdatePriceSchedule, UpdateProductionBatch,
    UpdateSupplierSku, VendorCredit, VendorCreditApplication, VendorCreditApplicationId,
    VendorCreditFilter, VendorCreditId, VendorCreditRepository, VendorReturn, VendorReturnFilter,
    VendorReturnId, VendorReturnRepository,
};
use uuid::Uuid;

fn unsupported<T>(domain: &str) -> Result<T> {
    Err(CommerceError::NotPermitted(format!(
        "{domain} are not supported by the PostgreSQL backend"
    )))
}

/// Channel repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedChannelRepository;

impl ChannelRepository for UnsupportedChannelRepository {
    fn create(&self, _input: CreateChannel) -> Result<Channel> {
        unsupported("channels")
    }
    fn get(&self, _id: ChannelId) -> Result<Option<Channel>> {
        unsupported("channels")
    }
    fn update(&self, _id: ChannelId, _input: UpdateChannel) -> Result<Channel> {
        unsupported("channels")
    }
    fn list(&self, _filter: ChannelFilter) -> Result<Vec<Channel>> {
        unsupported("channels")
    }
    fn delete(&self, _id: ChannelId) -> Result<()> {
        unsupported("channels")
    }
    fn set_lock(&self, _id: ChannelId, _locked: bool) -> Result<Channel> {
        unsupported("channels")
    }
    fn sync_products(&self, _id: ChannelId, _items: Vec<ChannelProductSyncItem>) -> Result<u64> {
        unsupported("channels")
    }
    fn list_product_mappings(&self, _id: ChannelId) -> Result<Vec<ChannelProductMapping>> {
        unsupported("channels")
    }
}

/// Company repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedCompanyRepository;

impl CompanyRepository for UnsupportedCompanyRepository {
    fn create(&self, _input: CreateCompany) -> Result<Company> {
        unsupported("companies")
    }
    fn get(&self, _id: CompanyId) -> Result<Option<Company>> {
        unsupported("companies")
    }
    fn update(&self, _id: CompanyId, _input: UpdateCompany) -> Result<Company> {
        unsupported("companies")
    }
    fn list(&self, _filter: CompanyFilter) -> Result<Vec<Company>> {
        unsupported("companies")
    }
    fn delete(&self, _id: CompanyId) -> Result<()> {
        unsupported("companies")
    }
    fn list_addresses(&self, _id: CompanyId) -> Result<Vec<CompanyShippingAddress>> {
        unsupported("companies")
    }
    fn list_price_overrides(&self, _id: CompanyId) -> Result<Vec<CompanyPriceOverride>> {
        unsupported("companies")
    }
    fn create_contact(&self, _input: CreateContact) -> Result<Contact> {
        unsupported("companies")
    }
    fn get_contact(&self, _id: ContactId) -> Result<Option<Contact>> {
        unsupported("companies")
    }
    fn list_contacts(&self, _company_id: CompanyId) -> Result<Vec<Contact>> {
        unsupported("companies")
    }
}

/// Transfer order repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedTransferOrderRepository;

impl TransferOrderRepository for UnsupportedTransferOrderRepository {
    fn create(&self, _input: CreateTransferOrder) -> Result<TransferOrder> {
        unsupported("transfer orders")
    }
    fn get(&self, _id: TransferOrderId) -> Result<Option<TransferOrder>> {
        unsupported("transfer orders")
    }
    fn list(&self, _filter: TransferOrderFilter) -> Result<Vec<TransferOrder>> {
        unsupported("transfer orders")
    }
    fn ship(&self, _id: TransferOrderId) -> Result<TransferOrder> {
        unsupported("transfer orders")
    }
    fn receive_line(
        &self,
        _id: TransferOrderId,
        _item_id: TransferOrderItemId,
        _quantity: Decimal,
    ) -> Result<TransferOrder> {
        unsupported("transfer orders")
    }
    fn cancel(&self, _id: TransferOrderId) -> Result<TransferOrder> {
        unsupported("transfer orders")
    }
}

/// Units-of-measure repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedUnitOfMeasureRepository;

impl UnitOfMeasureRepository for UnsupportedUnitOfMeasureRepository {
    fn create_class(&self, _input: CreateUnitClass) -> Result<UnitClass> {
        unsupported("units of measure")
    }
    fn list_classes(&self) -> Result<Vec<UnitClass>> {
        unsupported("units of measure")
    }
    fn delete_class(&self, _id: UnitClassId) -> Result<()> {
        unsupported("units of measure")
    }
    fn create_uom(&self, _input: CreateUnitOfMeasure) -> Result<UnitOfMeasure> {
        unsupported("units of measure")
    }
    fn list_uoms(&self, _class_id: Option<UnitClassId>) -> Result<Vec<UnitOfMeasure>> {
        unsupported("units of measure")
    }
    fn set_base_uom(&self, _id: UnitOfMeasureId) -> Result<UnitOfMeasure> {
        unsupported("units of measure")
    }
    fn delete_uom(&self, _id: UnitOfMeasureId) -> Result<()> {
        unsupported("units of measure")
    }
    fn create_rule(&self, _input: CreateUnitConversionRule) -> Result<UnitConversionRule> {
        unsupported("units of measure")
    }
    fn list_rules(&self) -> Result<Vec<UnitConversionRule>> {
        unsupported("units of measure")
    }
    fn delete_rule(&self, _id: UnitConversionRuleId) -> Result<()> {
        unsupported("units of measure")
    }
}

/// Production batch repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedProductionBatchRepository;

impl ProductionBatchRepository for UnsupportedProductionBatchRepository {
    fn create(&self, _input: CreateProductionBatch) -> Result<ProductionBatch> {
        unsupported("production batches")
    }
    fn get(&self, _id: ProductionBatchId) -> Result<Option<ProductionBatch>> {
        unsupported("production batches")
    }
    fn update(
        &self,
        _id: ProductionBatchId,
        _input: UpdateProductionBatch,
    ) -> Result<ProductionBatch> {
        unsupported("production batches")
    }
    fn list(&self, _filter: ProductionBatchFilter) -> Result<Vec<ProductionBatch>> {
        unsupported("production batches")
    }
    fn delete(&self, _id: ProductionBatchId) -> Result<()> {
        unsupported("production batches")
    }
    fn add_work_orders(
        &self,
        _id: ProductionBatchId,
        _work_order_ids: Vec<Uuid>,
    ) -> Result<ProductionBatch> {
        unsupported("production batches")
    }
    fn remove_work_order(
        &self,
        _id: ProductionBatchId,
        _work_order_id: Uuid,
    ) -> Result<ProductionBatch> {
        unsupported("production batches")
    }
}

/// Supplier SKU repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedSupplierSkuRepository;

impl SupplierSkuRepository for UnsupportedSupplierSkuRepository {
    fn create(&self, _input: CreateSupplierSku) -> Result<SupplierSku> {
        unsupported("supplier SKUs")
    }
    fn get(&self, _id: SupplierSkuId) -> Result<Option<SupplierSku>> {
        unsupported("supplier SKUs")
    }
    fn update(&self, _id: SupplierSkuId, _input: UpdateSupplierSku) -> Result<SupplierSku> {
        unsupported("supplier SKUs")
    }
    fn list(&self, _filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>> {
        unsupported("supplier SKUs")
    }
    fn delete(&self, _id: SupplierSkuId) -> Result<()> {
        unsupported("supplier SKUs")
    }
    fn bulk_upsert(&self, _supplier_id: Uuid, _items: Vec<BulkSupplierSkuItem>) -> Result<u64> {
        unsupported("supplier SKUs")
    }
}

/// Vendor return repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedVendorReturnRepository;

impl VendorReturnRepository for UnsupportedVendorReturnRepository {
    fn create(&self, _input: CreateVendorReturn) -> Result<VendorReturn> {
        unsupported("vendor returns")
    }
    fn get(&self, _id: VendorReturnId) -> Result<Option<VendorReturn>> {
        unsupported("vendor returns")
    }
    fn list(&self, _filter: VendorReturnFilter) -> Result<Vec<VendorReturn>> {
        unsupported("vendor returns")
    }
    fn submit(&self, _id: VendorReturnId) -> Result<VendorReturn> {
        unsupported("vendor returns")
    }
    fn process(&self, _id: VendorReturnId, _generate_credit: bool) -> Result<VendorReturn> {
        unsupported("vendor returns")
    }
    fn cancel(&self, _id: VendorReturnId) -> Result<VendorReturn> {
        unsupported("vendor returns")
    }
}

/// Vendor credit repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedVendorCreditRepository;

impl VendorCreditRepository for UnsupportedVendorCreditRepository {
    fn create(&self, _input: CreateVendorCredit) -> Result<VendorCredit> {
        unsupported("vendor credits")
    }
    fn get(&self, _id: VendorCreditId) -> Result<Option<VendorCredit>> {
        unsupported("vendor credits")
    }
    fn list(&self, _filter: VendorCreditFilter) -> Result<Vec<VendorCredit>> {
        unsupported("vendor credits")
    }
    fn apply(&self, _id: VendorCreditId, _input: ApplyVendorCredit) -> Result<VendorCredit> {
        unsupported("vendor credits")
    }
    fn list_applications(&self, _id: VendorCreditId) -> Result<Vec<VendorCreditApplication>> {
        unsupported("vendor credits")
    }
    fn reverse_application(
        &self,
        _id: VendorCreditId,
        _application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit> {
        unsupported("vendor credits")
    }
    fn cancel(&self, _id: VendorCreditId) -> Result<VendorCredit> {
        unsupported("vendor credits")
    }
}

/// Payment obligation repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPaymentObligationRepository;

impl PaymentObligationRepository for UnsupportedPaymentObligationRepository {
    fn create(&self, _input: CreatePaymentObligation) -> Result<PaymentObligation> {
        unsupported("payment obligations")
    }
    fn get(&self, _id: PaymentObligationId) -> Result<Option<PaymentObligation>> {
        unsupported("payment obligations")
    }
    fn list(&self, _filter: PaymentObligationFilter) -> Result<Vec<PaymentObligation>> {
        unsupported("payment obligations")
    }
    fn record_payment(
        &self,
        _id: PaymentObligationId,
        _amount: Decimal,
    ) -> Result<PaymentObligation> {
        unsupported("payment obligations")
    }
    fn set_status(
        &self,
        _id: PaymentObligationId,
        _status: PaymentObligationStatus,
    ) -> Result<PaymentObligation> {
        unsupported("payment obligations")
    }
    fn link_bill(&self, _id: PaymentObligationId, _bill_id: Uuid) -> Result<PaymentObligation> {
        unsupported("payment obligations")
    }
    fn dashboard(&self, _today: chrono::NaiveDate) -> Result<PaymentObligationDashboard> {
        unsupported("payment obligations")
    }
}

/// Price level repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPriceLevelRepository;

impl PriceLevelRepository for UnsupportedPriceLevelRepository {
    fn create(&self, _input: CreatePriceLevel) -> Result<PriceLevel> {
        unsupported("price levels")
    }
    fn get(&self, _id: PriceLevelId) -> Result<Option<PriceLevel>> {
        unsupported("price levels")
    }
    fn update(&self, _id: PriceLevelId, _input: UpdatePriceLevel) -> Result<PriceLevel> {
        unsupported("price levels")
    }
    fn list(&self, _filter: PriceLevelFilter) -> Result<Vec<PriceLevel>> {
        unsupported("price levels")
    }
    fn delete(&self, _id: PriceLevelId) -> Result<()> {
        unsupported("price levels")
    }
    fn set_entry(
        &self,
        _id: PriceLevelId,
        _product_id: ProductId,
        _price: Decimal,
    ) -> Result<PriceLevelEntry> {
        unsupported("price levels")
    }
    fn delete_entry(&self, _id: PriceLevelId, _product_id: ProductId) -> Result<()> {
        unsupported("price levels")
    }
    fn list_entries(&self, _id: PriceLevelId) -> Result<Vec<PriceLevelEntry>> {
        unsupported("price levels")
    }
}

/// Prepayment repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPrepaymentRepository;

impl PrepaymentRepository for UnsupportedPrepaymentRepository {
    fn create(&self, _input: CreatePrepayment) -> Result<Prepayment> {
        unsupported("prepayments")
    }
    fn get(&self, _id: PrepaymentId) -> Result<Option<Prepayment>> {
        unsupported("prepayments")
    }
    fn list(&self, _filter: PrepaymentFilter) -> Result<Vec<Prepayment>> {
        unsupported("prepayments")
    }
    fn apply(&self, _id: PrepaymentId, _input: ApplyPrepayment) -> Result<Prepayment> {
        unsupported("prepayments")
    }
    fn list_applications(&self, _id: PrepaymentId) -> Result<Vec<PrepaymentApplication>> {
        unsupported("prepayments")
    }
    fn reverse_application(
        &self,
        _id: PrepaymentId,
        _application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment> {
        unsupported("prepayments")
    }
    fn refund(&self, _id: PrepaymentId) -> Result<Prepayment> {
        unsupported("prepayments")
    }
}

/// Price schedule repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPriceScheduleRepository;

impl PriceScheduleRepository for UnsupportedPriceScheduleRepository {
    fn create(&self, _input: CreatePriceSchedule) -> Result<PriceSchedule> {
        unsupported("price schedules")
    }
    fn get(&self, _id: PriceScheduleId) -> Result<Option<PriceSchedule>> {
        unsupported("price schedules")
    }
    fn update(&self, _id: PriceScheduleId, _input: UpdatePriceSchedule) -> Result<PriceSchedule> {
        unsupported("price schedules")
    }
    fn list(&self, _filter: PriceScheduleFilter) -> Result<Vec<PriceSchedule>> {
        unsupported("price schedules")
    }
    fn delete(&self, _id: PriceScheduleId) -> Result<()> {
        unsupported("price schedules")
    }
    fn set_entry(
        &self,
        _id: PriceScheduleId,
        _product_id: ProductId,
        _price: Decimal,
    ) -> Result<PriceScheduleEntry> {
        unsupported("price schedules")
    }
    fn delete_entry(&self, _id: PriceScheduleId, _product_id: ProductId) -> Result<()> {
        unsupported("price schedules")
    }
    fn list_entries(&self, _id: PriceScheduleId) -> Result<Vec<PriceScheduleEntry>> {
        unsupported("price schedules")
    }
    fn resolve_price(
        &self,
        _product_id: ProductId,
        _at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<Decimal>> {
        unsupported("price schedules")
    }
}

/// Activity log repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedActivityLogRepository;

impl ActivityLogRepository for UnsupportedActivityLogRepository {
    fn record(&self, _input: RecordActivity) -> Result<ActivityLogEntry> {
        unsupported("activity logs")
    }
    fn get(&self, _id: ActivityLogId) -> Result<Option<ActivityLogEntry>> {
        unsupported("activity logs")
    }
    fn list(&self, _filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>> {
        unsupported("activity logs")
    }
    fn history_for_subject(
        &self,
        _subject_type: &str,
        _subject_id: Uuid,
    ) -> Result<Vec<ActivityLogEntry>> {
        unsupported("activity logs")
    }
}

/// Integration mapping repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedIntegrationMappingRepository;

impl IntegrationMappingRepository for UnsupportedIntegrationMappingRepository {
    fn create(&self, _input: CreateIntegrationMapping) -> Result<IntegrationMapping> {
        unsupported("integration mappings")
    }
    fn get(&self, _id: IntegrationMappingId) -> Result<Option<IntegrationMapping>> {
        unsupported("integration mappings")
    }
    fn update(
        &self,
        _id: IntegrationMappingId,
        _input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping> {
        unsupported("integration mappings")
    }
    fn list(&self, _filter: IntegrationMappingFilter) -> Result<Vec<IntegrationMapping>> {
        unsupported("integration mappings")
    }
    fn delete(&self, _id: IntegrationMappingId) -> Result<()> {
        unsupported("integration mappings")
    }
    fn bulk_upsert(&self, _items: Vec<CreateIntegrationMapping>) -> Result<u64> {
        unsupported("integration mappings")
    }
    fn resolve(&self, _lookup: &MappingLookup) -> Result<Option<String>> {
        unsupported("integration mappings")
    }
}

/// Inbound shipment repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedInboundShipmentRepository;

impl InboundShipmentRepository for UnsupportedInboundShipmentRepository {
    fn create(&self, _input: CreateInboundShipment) -> Result<InboundShipment> {
        unsupported("inbound shipments")
    }
    fn get(&self, _id: InboundShipmentId) -> Result<Option<InboundShipment>> {
        unsupported("inbound shipments")
    }
    fn list(&self, _filter: InboundShipmentFilter) -> Result<Vec<InboundShipment>> {
        unsupported("inbound shipments")
    }
    fn mark_in_transit(&self, _id: InboundShipmentId) -> Result<InboundShipment> {
        unsupported("inbound shipments")
    }
    fn mark_arrived(&self, _id: InboundShipmentId) -> Result<InboundShipment> {
        unsupported("inbound shipments")
    }
    fn receive_line(
        &self,
        _id: InboundShipmentId,
        _item_id: InboundShipmentItemId,
        _quantity: Decimal,
    ) -> Result<InboundShipment> {
        unsupported("inbound shipments")
    }
    fn cancel(&self, _id: InboundShipmentId) -> Result<InboundShipment> {
        unsupported("inbound shipments")
    }
}

/// Purgatory repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPurgatoryRepository;

impl PurgatoryRepository for UnsupportedPurgatoryRepository {
    fn ingest(&self, _input: IngestOrder) -> Result<PurgatoryOrder> {
        unsupported("purgatory")
    }
    fn get(&self, _id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>> {
        unsupported("purgatory")
    }
    fn list(&self, _filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>> {
        unsupported("purgatory")
    }
    fn map_line(
        &self,
        _id: PurgatoryOrderId,
        _line_id: PurgatoryLineItemId,
        _input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder> {
        unsupported("purgatory")
    }
    fn post(&self, _id: PurgatoryOrderId) -> Result<PurgatoryOrder> {
        unsupported("purgatory")
    }
    fn delete(&self, _id: PurgatoryOrderId) -> Result<()> {
        unsupported("purgatory")
    }
}

/// Print station repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedPrintStationRepository;

impl PrintStationRepository for UnsupportedPrintStationRepository {
    fn pair(&self, _input: CreatePrintStation) -> Result<PairStationResult> {
        unsupported("print stations")
    }
    fn list_stations(&self) -> Result<Vec<PrintStation>> {
        unsupported("print stations")
    }
    fn get_station(&self, _id: PrintStationId) -> Result<Option<PrintStation>> {
        unsupported("print stations")
    }
    fn revoke_station(&self, _id: PrintStationId) -> Result<PrintStation> {
        unsupported("print stations")
    }
    fn enqueue_job(
        &self,
        _station_id: PrintStationId,
        _input: EnqueuePrintJob,
    ) -> Result<PrintJob> {
        unsupported("print stations")
    }
    fn next_job(&self, _station_id: PrintStationId) -> Result<Option<PrintJob>> {
        unsupported("print stations")
    }
    fn complete_job(&self, _job_id: PrintJobId, _success: bool) -> Result<PrintJob> {
        unsupported("print stations")
    }
    fn list_jobs(
        &self,
        _station_id: PrintStationId,
        _filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>> {
        unsupported("print stations")
    }
}

/// EDI document repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedEdiDocumentRepository;

impl EdiDocumentRepository for UnsupportedEdiDocumentRepository {
    fn create(&self, _input: CreateEdiDocument) -> Result<EdiDocument> {
        unsupported("EDI documents")
    }
    fn get(&self, _id: EdiDocumentId) -> Result<Option<EdiDocument>> {
        unsupported("EDI documents")
    }
    fn list(&self, _filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>> {
        unsupported("EDI documents")
    }
    fn set_status(
        &self,
        _id: EdiDocumentId,
        _status: EdiStatus,
        _error_message: Option<String>,
    ) -> Result<EdiDocument> {
        unsupported("EDI documents")
    }
    fn summary(&self) -> Result<EdiAggregateSummary> {
        unsupported("EDI documents")
    }
}

/// Integration field-mapping repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedIntegrationFieldMappingRepository;

impl IntegrationFieldMappingRepository for UnsupportedIntegrationFieldMappingRepository {
    fn create(&self, _input: CreateIntegrationFieldMapping) -> Result<IntegrationFieldMapping> {
        unsupported("integration field mappings")
    }
    fn get(&self, _id: IntegrationFieldMappingId) -> Result<Option<IntegrationFieldMapping>> {
        unsupported("integration field mappings")
    }
    fn update(
        &self,
        _id: IntegrationFieldMappingId,
        _input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping> {
        unsupported("integration field mappings")
    }
    fn list(&self, _filter: IntegrationFieldMappingFilter) -> Result<Vec<IntegrationFieldMapping>> {
        unsupported("integration field mappings")
    }
    fn delete(&self, _id: IntegrationFieldMappingId) -> Result<()> {
        unsupported("integration field mappings")
    }
    fn bulk_create(&self, _items: Vec<CreateIntegrationFieldMapping>) -> Result<u64> {
        unsupported("integration field mappings")
    }
    fn bulk_delete(&self, _ids: Vec<IntegrationFieldMappingId>) -> Result<u64> {
        unsupported("integration field mappings")
    }
    fn distinct_groups(&self, _integration_account: &str) -> Result<Vec<String>> {
        unsupported("integration field mappings")
    }
}

/// Topology snapshot repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedTopologySnapshotRepository;

impl TopologySnapshotRepository for UnsupportedTopologySnapshotRepository {
    fn capture(&self, _input: CaptureTopologySnapshot) -> Result<TopologySnapshot> {
        unsupported("topology snapshots")
    }
    fn get(&self, _id: TopologySnapshotId) -> Result<Option<TopologySnapshot>> {
        unsupported("topology snapshots")
    }
    fn latest(&self) -> Result<Option<TopologySnapshot>> {
        unsupported("topology snapshots")
    }
    fn list(&self, _filter: TopologySnapshotFilter) -> Result<Vec<TopologySnapshot>> {
        unsupported("topology snapshots")
    }
    fn delete(&self, _id: TopologySnapshotId) -> Result<()> {
        unsupported("topology snapshots")
    }
}

/// Stock snapshot repository shim that rejects all operations.
#[derive(Debug, Default)]
pub(crate) struct UnsupportedStockSnapshotRepository;

impl StockSnapshotRepository for UnsupportedStockSnapshotRepository {
    fn capture(&self, _input: CaptureStockSnapshot) -> Result<StockSnapshot> {
        unsupported("stock snapshots")
    }
    fn get(&self, _id: StockSnapshotId) -> Result<Option<StockSnapshot>> {
        unsupported("stock snapshots")
    }
    fn latest(&self) -> Result<Option<StockSnapshot>> {
        unsupported("stock snapshots")
    }
    fn list(&self, _filter: StockSnapshotFilter) -> Result<Vec<StockSnapshot>> {
        unsupported("stock snapshots")
    }
    fn delete(&self, _id: StockSnapshotId) -> Result<()> {
        unsupported("stock snapshots")
    }
}
