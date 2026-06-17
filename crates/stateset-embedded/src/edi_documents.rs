//! EDI document operations (trading-partner document tracking).

use stateset_core::{
    CreateEdiDocument, EdiAggregateSummary, EdiDocument, EdiDocumentFilter, EdiDocumentId,
    EdiStatus, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// EDI document operations.
pub struct EdiDocuments {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for EdiDocuments {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdiDocuments").finish_non_exhaustive()
    }
}

impl EdiDocuments {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether EDI documents are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::EdiDocuments)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::EdiDocuments)
    }

    /// Create / ingest an EDI document.
    pub fn create(&self, input: CreateEdiDocument) -> Result<EdiDocument> {
        self.ensure()?;
        self.db.edi_documents().create(input)
    }

    /// Get a document by ID.
    pub fn get(&self, id: EdiDocumentId) -> Result<Option<EdiDocument>> {
        self.ensure()?;
        self.db.edi_documents().get(id)
    }

    /// List documents with optional filtering.
    pub fn list(&self, filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>> {
        self.ensure()?;
        self.db.edi_documents().list(filter)
    }

    /// Update a document's status.
    pub fn set_status(
        &self,
        id: EdiDocumentId,
        status: EdiStatus,
        error_message: Option<String>,
    ) -> Result<EdiDocument> {
        self.ensure()?;
        self.db.edi_documents().set_status(id, status, error_message)
    }

    /// Aggregate summary across all documents.
    pub fn summary(&self) -> Result<EdiAggregateSummary> {
        self.ensure()?;
        self.db.edi_documents().summary()
    }
}
