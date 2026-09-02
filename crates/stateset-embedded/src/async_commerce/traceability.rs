//! Traceability accessors: quality, lots, serials.

use super::*;

/// Async quality operations.
pub struct AsyncQuality {
    db: Arc<PostgresDatabase>,
}

impl AsyncQuality {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_inspection(&self, input: CreateInspection) -> Result<Inspection> {
        self.db.quality().create_inspection_async(input).await
    }

    pub async fn get_inspection(&self, id: Uuid) -> Result<Option<Inspection>> {
        self.db.quality().get_inspection_async(id).await
    }

    pub async fn get_inspection_by_number(&self, number: &str) -> Result<Option<Inspection>> {
        self.db.quality().get_inspection_by_number_async(number).await
    }

    pub async fn update_inspection(&self, id: Uuid, input: UpdateInspection) -> Result<Inspection> {
        self.db.quality().update_inspection_async(id, input).await
    }

    pub async fn list_inspections(&self, filter: InspectionFilter) -> Result<Vec<Inspection>> {
        self.db.quality().list_inspections_async(filter).await
    }

    pub async fn delete_inspection(&self, id: Uuid) -> Result<()> {
        self.db.quality().delete_inspection_async(id).await
    }

    pub async fn start_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.db.quality().start_inspection_async(id).await
    }

    pub async fn complete_inspection(&self, id: Uuid) -> Result<Inspection> {
        self.db.quality().complete_inspection_async(id).await
    }

    pub async fn record_inspection_result(
        &self,
        input: RecordInspectionResult,
    ) -> Result<InspectionItem> {
        self.db.quality().record_inspection_result_async(input).await
    }

    pub async fn get_inspection_items(&self, inspection_id: Uuid) -> Result<Vec<InspectionItem>> {
        self.db.quality().get_inspection_items_async(inspection_id).await
    }

    pub async fn count_inspections(&self, filter: InspectionFilter) -> Result<u64> {
        self.db.quality().count_inspections_async(filter).await
    }

    pub async fn create_ncr(&self, input: CreateNonConformance) -> Result<NonConformance> {
        self.db.quality().create_ncr_async(input).await
    }

    pub async fn get_ncr(&self, id: Uuid) -> Result<Option<NonConformance>> {
        self.db.quality().get_ncr_async(id).await
    }

    pub async fn get_ncr_by_number(&self, number: &str) -> Result<Option<NonConformance>> {
        self.db.quality().get_ncr_by_number_async(number).await
    }

    pub async fn update_ncr(
        &self,
        id: Uuid,
        input: UpdateNonConformance,
    ) -> Result<NonConformance> {
        self.db.quality().update_ncr_async(id, input).await
    }

    pub async fn list_ncrs(&self, filter: NonConformanceFilter) -> Result<Vec<NonConformance>> {
        self.db.quality().list_ncrs_async(filter).await
    }

    pub async fn close_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.db.quality().close_ncr_async(id).await
    }

    pub async fn cancel_ncr(&self, id: Uuid) -> Result<NonConformance> {
        self.db.quality().cancel_ncr_async(id).await
    }

    pub async fn count_ncrs(&self, filter: NonConformanceFilter) -> Result<u64> {
        self.db.quality().count_ncrs_async(filter).await
    }

    pub async fn create_hold(&self, input: CreateQualityHold) -> Result<QualityHold> {
        self.db.quality().create_hold_async(input).await
    }

    pub async fn get_hold(&self, id: Uuid) -> Result<Option<QualityHold>> {
        self.db.quality().get_hold_async(id).await
    }

    pub async fn list_holds(&self, filter: QualityHoldFilter) -> Result<Vec<QualityHold>> {
        self.db.quality().list_holds_async(filter).await
    }

    pub async fn release_hold(&self, id: Uuid, input: ReleaseQualityHold) -> Result<QualityHold> {
        self.db.quality().release_hold_async(id, input).await
    }

    pub async fn get_active_holds_for_sku(&self, sku: &str) -> Result<Vec<QualityHold>> {
        self.db.quality().get_active_holds_for_sku_async(sku).await
    }

    pub async fn get_active_holds_for_lot(&self, lot_number: &str) -> Result<Vec<QualityHold>> {
        self.db.quality().get_active_holds_for_lot_async(lot_number).await
    }

    pub async fn count_active_holds(&self) -> Result<u64> {
        self.db.quality().count_active_holds_async().await
    }

    pub async fn create_defect_code(&self, input: CreateDefectCode) -> Result<DefectCode> {
        self.db.quality().create_defect_code_async(input).await
    }

    pub async fn get_defect_code(&self, code: &str) -> Result<Option<DefectCode>> {
        self.db.quality().get_defect_code_async(code).await
    }

    pub async fn list_defect_codes(&self, category: Option<&str>) -> Result<Vec<DefectCode>> {
        self.db.quality().list_defect_codes_async(category).await
    }

    pub async fn deactivate_defect_code(&self, id: Uuid) -> Result<()> {
        self.db.quality().deactivate_defect_code_async(id).await
    }
}

// ============================================================================
// Async Lots
// ============================================================================

/// Async lot operations.
pub struct AsyncLots {
    db: Arc<PostgresDatabase>,
}

impl AsyncLots {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateLot) -> Result<Lot> {
        self.db.lots().create_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Lot>> {
        self.db.lots().get_async(id).await
    }

    pub async fn get_by_number(&self, lot_number: &str) -> Result<Option<Lot>> {
        self.db.lots().get_by_number_async(lot_number).await
    }

    pub async fn update(&self, id: Uuid, input: UpdateLot) -> Result<Lot> {
        self.db.lots().update_async(id, input).await
    }

    pub async fn list(&self, filter: LotFilter) -> Result<Vec<Lot>> {
        self.db.lots().list_async(filter).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.lots().delete_async(id).await
    }

    pub async fn adjust(&self, input: AdjustLot) -> Result<LotTransaction> {
        self.db.lots().adjust_async(input).await
    }

    pub async fn consume(&self, input: ConsumeLot) -> Result<LotTransaction> {
        self.db.lots().consume_async(input).await
    }

    pub async fn reserve(&self, input: ReserveLot) -> Result<Uuid> {
        self.db.lots().reserve_async(input).await
    }

    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.lots().release_reservation_async(reservation_id).await
    }

    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<LotTransaction> {
        self.db.lots().confirm_reservation_async(reservation_id).await
    }

    pub async fn transfer(&self, input: TransferLot) -> Result<LotTransaction> {
        self.db.lots().transfer_async(input).await
    }

    pub async fn split(&self, input: SplitLot) -> Result<Lot> {
        self.db.lots().split_async(input).await
    }

    pub async fn merge(&self, input: MergeLots) -> Result<Lot> {
        self.db.lots().merge_async(input).await
    }

    pub async fn quarantine(&self, id: Uuid, reason: &str) -> Result<Lot> {
        self.db.lots().quarantine_async(id, reason).await
    }

    pub async fn release_quarantine(&self, id: Uuid) -> Result<Lot> {
        self.db.lots().release_quarantine_async(id).await
    }

    pub async fn get_transactions(&self, lot_id: Uuid, limit: u32) -> Result<Vec<LotTransaction>> {
        self.db.lots().get_transactions_async(lot_id, limit).await
    }

    pub async fn get_quantity_at_location(
        &self,
        lot_id: Uuid,
        location_id: i32,
    ) -> Result<Option<Decimal>> {
        self.db.lots().get_quantity_at_location_async(lot_id, location_id).await
    }

    pub async fn get_lot_locations(&self, lot_id: Uuid) -> Result<Vec<LotLocation>> {
        self.db.lots().get_lot_locations_async(lot_id).await
    }

    pub async fn add_certificate(&self, input: AddLotCertificate) -> Result<LotCertificate> {
        self.db.lots().add_certificate_async(input).await
    }

    pub async fn get_certificates(&self, lot_id: Uuid) -> Result<Vec<LotCertificate>> {
        self.db.lots().get_certificates_async(lot_id).await
    }

    pub async fn delete_certificate(&self, certificate_id: Uuid) -> Result<()> {
        self.db.lots().delete_certificate_async(certificate_id).await
    }

    pub async fn get_expiring_lots(&self, days: i32) -> Result<Vec<Lot>> {
        self.db.lots().get_expiring_lots_async(days).await
    }

    pub async fn get_expired_lots(&self) -> Result<Vec<Lot>> {
        self.db.lots().get_expired_lots_async().await
    }

    /// Sweep `Active` lots past their `expiration_date` into `Expired`;
    /// returns how many were flipped. Idempotent.
    pub async fn expire_lots(&self) -> Result<u64> {
        self.db.lots().expire_lots_async(chrono::Utc::now()).await
    }

    pub async fn get_available_lots_for_sku(&self, sku: &str) -> Result<Vec<Lot>> {
        self.db.lots().get_available_lots_for_sku_async(sku).await
    }

    pub async fn trace(&self, lot_id: Uuid) -> Result<TraceabilityResult> {
        self.db.lots().trace_async(lot_id).await
    }

    pub async fn count(&self, filter: LotFilter) -> Result<u64> {
        self.db.lots().count_async(filter).await
    }

    pub async fn create_batch(&self, inputs: Vec<CreateLot>) -> Result<BatchResult<Lot>> {
        self.db.lots().create_batch_async(inputs).await
    }

    pub async fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Lot>> {
        self.db.lots().get_batch_async(ids).await
    }
}

// ============================================================================
// Async Serials
// ============================================================================

/// Async serial operations.
pub struct AsyncSerials {
    db: Arc<PostgresDatabase>,
}

impl AsyncSerials {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create(&self, input: CreateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().create_async(input).await
    }

    pub async fn create_bulk(&self, input: CreateSerialNumbersBulk) -> Result<Vec<SerialNumber>> {
        self.db.serials().create_bulk_async(input).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<SerialNumber>> {
        self.db.serials().get_async(id).await
    }

    pub async fn get_by_serial(&self, serial: &str) -> Result<Option<SerialNumber>> {
        self.db.serials().get_by_serial_async(serial).await
    }

    pub async fn update(&self, id: Uuid, input: UpdateSerialNumber) -> Result<SerialNumber> {
        self.db.serials().update_async(id, input).await
    }

    pub async fn list(&self, filter: SerialFilter) -> Result<Vec<SerialNumber>> {
        self.db.serials().list_async(filter).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.serials().delete_async(id).await
    }

    pub async fn change_status(&self, input: ChangeSerialStatus) -> Result<SerialNumber> {
        self.db.serials().change_status_async(input).await
    }

    pub async fn reserve(&self, input: ReserveSerialNumber) -> Result<SerialReservation> {
        self.db.serials().reserve_async(input).await
    }

    pub async fn release_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().release_reservation_async(reservation_id).await
    }

    pub async fn get_reservation(&self, reservation_id: Uuid) -> Result<Option<SerialReservation>> {
        self.db.serials().get_reservation_async(reservation_id).await
    }

    pub async fn release_expired_reservations(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64> {
        self.db.serials().release_expired_reservations_async(now).await
    }

    pub async fn quarantine_for_lot(&self, lot_id: Uuid, reason: &str) -> Result<u64> {
        self.db.serials().quarantine_for_lot_async(lot_id, reason).await
    }

    pub async fn release_quarantine_for_lot(&self, lot_id: Uuid) -> Result<u64> {
        self.db.serials().release_quarantine_for_lot_async(lot_id).await
    }

    pub async fn confirm_reservation(&self, reservation_id: Uuid) -> Result<()> {
        self.db.serials().confirm_reservation_async(reservation_id).await
    }

    pub async fn move_serial(&self, input: MoveSerial) -> Result<SerialNumber> {
        self.db.serials().move_serial_async(input).await
    }

    pub async fn transfer_ownership(&self, input: TransferSerialOwnership) -> Result<SerialNumber> {
        self.db.serials().transfer_ownership_async(input).await
    }

    pub async fn mark_sold(
        &self,
        id: Uuid,
        customer_id: Uuid,
        order_id: Option<Uuid>,
    ) -> Result<SerialNumber> {
        self.db.serials().mark_sold_async(id, customer_id, order_id).await
    }

    pub async fn mark_shipped(&self, id: Uuid, shipment_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_shipped_async(id, shipment_id).await
    }

    pub async fn mark_returned(&self, id: Uuid, return_id: Uuid) -> Result<SerialNumber> {
        self.db.serials().mark_returned_async(id, return_id).await
    }

    pub async fn activate(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().activate_async(id).await
    }

    pub async fn quarantine(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().quarantine_async(id, reason).await
    }

    pub async fn release_quarantine(&self, id: Uuid) -> Result<SerialNumber> {
        self.db.serials().release_quarantine_async(id).await
    }

    pub async fn scrap(&self, id: Uuid, reason: &str) -> Result<SerialNumber> {
        self.db.serials().scrap_async(id, reason).await
    }

    pub async fn get_history(
        &self,
        serial_id: Uuid,
        filter: SerialHistoryFilter,
    ) -> Result<Vec<SerialHistory>> {
        self.db.serials().get_history_async(serial_id, filter).await
    }

    pub async fn lookup(&self, serial: &str) -> Result<Option<SerialLookupResult>> {
        self.db.serials().lookup_async(serial).await
    }

    pub async fn validate(&self, serial: &str) -> Result<SerialValidation> {
        self.db.serials().validate_async(serial).await
    }

    pub async fn get_available_for_sku(&self, sku: &str, limit: u32) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_available_for_sku_async(sku, limit).await
    }

    pub async fn get_for_lot(&self, lot_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_lot_async(lot_id).await
    }

    pub async fn get_for_customer(&self, customer_id: Uuid) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_for_customer_async(customer_id).await
    }

    pub async fn count(&self, filter: SerialFilter) -> Result<u64> {
        self.db.serials().count_async(filter).await
    }

    pub async fn create_batch(
        &self,
        inputs: Vec<CreateSerialNumber>,
    ) -> Result<BatchResult<SerialNumber>> {
        self.db.serials().create_batch_async(inputs).await
    }

    pub async fn get_batch(&self, ids: Vec<Uuid>) -> Result<Vec<SerialNumber>> {
        self.db.serials().get_batch_async(ids).await
    }
}

// ============================================================================
// Async Warehouse
// ============================================================================
