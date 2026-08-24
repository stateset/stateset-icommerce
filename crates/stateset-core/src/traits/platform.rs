//! Platform repositories: custom objects, activity log, integration mappings, purgatory, EDI documents, and topology snapshots.

use super::*;

// ============================================================================
// Custom Objects Repository
// ============================================================================

/// Custom Objects repository trait (custom states / metaobjects).
///
/// Provides a schema-driven custom data system:
/// - Define types (schemas) with typed fields
/// - Create records (instances) that validate against the schema
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CustomObjectRepository: Send + Sync {
    // ------------------------------------------------------------------------
    // Type (schema) operations
    // ------------------------------------------------------------------------

    fn create_type(&self, input: CreateCustomObjectType) -> Result<CustomObjectType>;

    fn get_type(&self, id: Uuid) -> Result<Option<CustomObjectType>>;

    fn get_type_by_handle(&self, handle: &str) -> Result<Option<CustomObjectType>>;

    fn update_type(&self, id: Uuid, input: UpdateCustomObjectType) -> Result<CustomObjectType>;

    fn list_types(&self, filter: CustomObjectTypeFilter) -> Result<Vec<CustomObjectType>>;

    fn delete_type(&self, id: Uuid) -> Result<()>;

    // ------------------------------------------------------------------------
    // Record operations
    // ------------------------------------------------------------------------

    fn create_object(&self, input: CreateCustomObject) -> Result<CustomObject>;

    fn get_object(&self, id: Uuid) -> Result<Option<CustomObject>>;

    fn get_object_by_handle(
        &self,
        type_handle: &str,
        object_handle: &str,
    ) -> Result<Option<CustomObject>>;

    fn update_object(&self, id: Uuid, input: UpdateCustomObject) -> Result<CustomObject>;

    fn list_objects(&self, filter: CustomObjectFilter) -> Result<Vec<CustomObject>>;

    fn delete_object(&self, id: Uuid) -> Result<()>;
}

/// Activity log (append-only subject history) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ActivityLogRepository: Send + Sync {
    /// Record a new activity log entry.
    fn record(&self, input: RecordActivity) -> Result<ActivityLogEntry>;

    /// Get an entry by ID.
    fn get(&self, id: ActivityLogId) -> Result<Option<ActivityLogEntry>>;

    /// List entries with filter (most recent first).
    fn list(&self, filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>>;

    /// List the full (unpaginated) history for a single subject, most recent
    /// first. Used for timelines and AI-over-history summaries.
    fn history_for_subject(
        &self,
        subject_type: &str,
        subject_id: uuid::Uuid,
    ) -> Result<Vec<ActivityLogEntry>>;
}

/// Integration mapping repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait IntegrationMappingRepository: Send + Sync {
    /// Create a new integration mapping.
    fn create(&self, input: CreateIntegrationMapping) -> Result<IntegrationMapping>;

    /// Get a mapping by ID.
    fn get(&self, id: IntegrationMappingId) -> Result<Option<IntegrationMapping>>;

    /// Update a mapping (partial).
    fn update(
        &self,
        id: IntegrationMappingId,
        input: UpdateIntegrationMapping,
    ) -> Result<IntegrationMapping>;

    /// List mappings with filter.
    fn list(&self, filter: IntegrationMappingFilter) -> Result<Vec<IntegrationMapping>>;

    /// Delete a mapping.
    fn delete(&self, id: IntegrationMappingId) -> Result<()>;

    /// Bulk upsert mappings. Returns the number of rows affected.
    fn bulk_upsert(&self, items: Vec<CreateIntegrationMapping>) -> Result<u64>;

    /// Resolve the internal value for an external value, or `None` if unmapped
    /// (or the mapping is inactive).
    fn resolve(&self, lookup: &MappingLookup) -> Result<Option<String>>;
}

/// Integration field-mapping repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait IntegrationFieldMappingRepository: Send + Sync {
    /// Create a new field mapping.
    fn create(&self, input: CreateIntegrationFieldMapping) -> Result<IntegrationFieldMapping>;

    /// Get a field mapping by ID.
    fn get(&self, id: IntegrationFieldMappingId) -> Result<Option<IntegrationFieldMapping>>;

    /// Update a field mapping (partial).
    fn update(
        &self,
        id: IntegrationFieldMappingId,
        input: UpdateIntegrationFieldMapping,
    ) -> Result<IntegrationFieldMapping>;

    /// List field mappings with filter.
    fn list(&self, filter: IntegrationFieldMappingFilter) -> Result<Vec<IntegrationFieldMapping>>;

    /// Delete a field mapping.
    fn delete(&self, id: IntegrationFieldMappingId) -> Result<()>;

    /// Bulk create field mappings. Returns the number created.
    fn bulk_create(&self, items: Vec<CreateIntegrationFieldMapping>) -> Result<u64>;

    /// Bulk delete field mappings by ID. Returns the number deleted.
    fn bulk_delete(&self, ids: Vec<IntegrationFieldMappingId>) -> Result<u64>;

    /// List the distinct mapping groups for an integration account.
    fn distinct_groups(&self, integration_account: &str) -> Result<Vec<String>>;
}

/// Purgatory (order ingestion staging) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PurgatoryRepository: Send + Sync {
    /// Ingest an order into purgatory (non-posted).
    fn ingest(&self, input: IngestOrder) -> Result<PurgatoryOrder>;

    /// Get a purgatory order by ID (with line items).
    fn get(&self, id: PurgatoryOrderId) -> Result<Option<PurgatoryOrder>>;

    /// List purgatory orders with filter (defaults to non-posted).
    fn list(&self, filter: PurgatoryFilter) -> Result<Vec<PurgatoryOrder>>;

    /// Map a line to a product and/or toggle its ignore / non-physical flags.
    fn map_line(
        &self,
        id: PurgatoryOrderId,
        line_id: PurgatoryLineItemId,
        input: MapPurgatoryLine,
    ) -> Result<PurgatoryOrder>;

    /// Post the order, committing it out of purgatory. Errors if any line is
    /// still unresolved.
    fn post(&self, id: PurgatoryOrderId) -> Result<PurgatoryOrder>;

    /// Delete a purgatory order.
    fn delete(&self, id: PurgatoryOrderId) -> Result<()>;
}

/// EDI document repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait EdiDocumentRepository: Send + Sync {
    /// Create / ingest an EDI document.
    fn create(&self, input: CreateEdiDocument) -> Result<EdiDocument>;

    /// Get a document by ID.
    fn get(&self, id: EdiDocumentId) -> Result<Option<EdiDocument>>;

    /// List documents with filter.
    fn list(&self, filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>>;

    /// Update a document's status, optionally recording an error message.
    fn set_status(
        &self,
        id: EdiDocumentId,
        status: EdiStatus,
        error_message: Option<String>,
    ) -> Result<EdiDocument>;

    /// Aggregate summary (counts by status and type) across all documents.
    fn summary(&self) -> Result<EdiAggregateSummary>;
}

/// Customer operational topology snapshot repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait TopologySnapshotRepository: Send + Sync {
    /// Capture a new snapshot (health is derived from the metrics).
    fn capture(&self, input: CaptureTopologySnapshot) -> Result<TopologySnapshot>;

    /// Get a snapshot by ID.
    fn get(&self, id: TopologySnapshotId) -> Result<Option<TopologySnapshot>>;

    /// Get the most recent snapshot, if any.
    fn latest(&self) -> Result<Option<TopologySnapshot>>;

    /// List snapshots (most recent first).
    fn list(&self, filter: TopologySnapshotFilter) -> Result<Vec<TopologySnapshot>>;

    /// Delete a snapshot.
    fn delete(&self, id: TopologySnapshotId) -> Result<()>;
}
