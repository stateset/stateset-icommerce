//! EDI Documents API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// EDI Documents API
// ============================================================================

pub(crate) fn parse_edi_direction(s: &str) -> Result<stateset_core::EdiDirection> {
    s.parse::<stateset_core::EdiDirection>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid EDI direction: {s}")))
}

pub(crate) fn parse_edi_status(s: &str) -> Result<stateset_core::EdiStatus> {
    s.parse::<stateset_core::EdiStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid EDI status: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiDocumentOutput {
    pub id: String,
    /// EDI document type (e.g. `850`, `855`, `856`, `810`)
    pub document_type: String,
    /// One of `inbound`, `outbound`
    #[napi(ts_type = "EdiDirection")]
    pub direction: String,
    /// One of `pending`, `sent`, `acknowledged`, `processed`, `error`
    #[napi(ts_type = "EdiStatus")]
    pub status: String,
    /// Trading partner name / id
    pub partner: Option<String>,
    /// Related business reference (PO number, order number, etc.)
    pub reference: Option<String>,
    /// Raw EDI payload
    pub payload: Option<String>,
    /// Error detail when `status = error`
    pub error_message: Option<String>,
    /// RFC 3339 timestamp
    pub created_at: String,
    /// RFC 3339 timestamp
    pub updated_at: String,
}

impl From<stateset_core::EdiDocument> for EdiDocumentOutput {
    fn from(d: stateset_core::EdiDocument) -> Self {
        Self {
            id: d.id.to_string(),
            document_type: d.document_type,
            direction: d.direction.to_string(),
            status: d.status.to_string(),
            partner: d.partner,
            reference: d.reference,
            payload: d.payload,
            error_message: d.error_message,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateEdiDocumentInput {
    /// EDI document type (e.g. `850`, `855`, `856`, `810`)
    pub document_type: String,
    /// One of `inbound`, `outbound` (defaults to `inbound`)
    #[napi(ts_type = "EdiDirection")]
    pub direction: Option<String>,
    /// Trading partner name / id
    pub partner: Option<String>,
    /// Related business reference (PO number, order number, etc.)
    pub reference: Option<String>,
    /// Raw EDI payload
    pub payload: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EdiDocumentFilterInput {
    /// Filter by document type (e.g. `850`)
    pub document_type: Option<String>,
    /// Filter by direction: `inbound` or `outbound`
    #[napi(ts_type = "EdiDirection")]
    pub direction: Option<String>,
    /// Filter by status: `pending`, `sent`, `acknowledged`, `processed`, `error`
    #[napi(ts_type = "EdiStatus")]
    pub status: Option<String>,
    /// Filter by trading partner
    pub partner: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiCountOutput {
    /// The group key (status or document type)
    pub key: String,
    /// Number of documents in the group
    pub count: i64,
}

impl From<stateset_core::EdiCount> for EdiCountOutput {
    fn from(c: stateset_core::EdiCount) -> Self {
        Self { key: c.key, count: i64::try_from(c.count).unwrap_or(i64::MAX) }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EdiSummaryOutput {
    /// Total document count
    pub total: i64,
    /// Counts grouped by status
    pub by_status: Vec<EdiCountOutput>,
    /// Counts grouped by document type
    pub by_type: Vec<EdiCountOutput>,
}

impl From<stateset_core::EdiAggregateSummary> for EdiSummaryOutput {
    fn from(s: stateset_core::EdiAggregateSummary) -> Self {
        Self {
            total: i64::try_from(s.total).unwrap_or(i64::MAX),
            by_status: s.by_status.into_iter().map(Into::into).collect(),
            by_type: s.by_type.into_iter().map(Into::into).collect(),
        }
    }
}

#[napi]
pub struct EdiDocuments {
    pub(crate) commerce: Handle,
}

#[napi]
impl EdiDocuments {
    /// Create / ingest an EDI document.
    #[napi]
    pub async fn create(&self, input: CreateEdiDocumentInput) -> Result<EdiDocumentOutput> {
        let commerce = self.commerce.get()?;
        let direction =
            input.direction.as_deref().map(parse_edi_direction).transpose()?.unwrap_or_default();
        let doc = commerce
            .edi_documents()
            .create(stateset_core::CreateEdiDocument {
                document_type: input.document_type,
                direction,
                partner: input.partner,
                reference: input.reference,
                payload: input.payload,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create EDI document", e))?;
        Ok(doc.into())
    }

    /// Get an EDI document by ID.
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<EdiDocumentOutput>> {
        let commerce = self.commerce.get()?;
        let doc_id = id
            .parse::<stateset_core::EdiDocumentId>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let doc = commerce
            .edi_documents()
            .get(doc_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get EDI document", e))?;
        Ok(doc.map(Into::into))
    }

    /// List EDI documents with optional filtering.
    #[napi]
    pub async fn list(
        &self,
        filter: Option<EdiDocumentFilterInput>,
    ) -> Result<Vec<EdiDocumentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let docs = commerce
            .edi_documents()
            .list(stateset_core::EdiDocumentFilter {
                document_type: filter.document_type,
                direction: filter.direction.as_deref().map(parse_edi_direction).transpose()?,
                status: filter.status.as_deref().map(parse_edi_status).transpose()?,
                partner: filter.partner,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list EDI documents", e))?;
        Ok(docs.into_iter().map(Into::into).collect())
    }

    /// Update an EDI document's status.
    ///
    /// `status` is one of `pending`, `sent`, `acknowledged`, `processed`, `error`;
    /// `error_message` records failure detail when the status is `error`.
    #[napi]
    pub async fn set_status(
        &self,
        id: String,
        #[napi(ts_arg_type = "EdiStatus")] status: String,
        error_message: Option<String>,
    ) -> Result<EdiDocumentOutput> {
        let commerce = self.commerce.get()?;
        let doc_id = id
            .parse::<stateset_core::EdiDocumentId>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let status = parse_edi_status(&status)?;
        let doc = commerce
            .edi_documents()
            .set_status(doc_id, status, error_message)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set EDI document status", e))?;
        Ok(doc.into())
    }

    /// Aggregate summary across all EDI documents (counts by status and type).
    #[napi]
    pub async fn summary(&self) -> Result<EdiSummaryOutput> {
        let commerce = self.commerce.get()?;
        let summary = commerce
            .edi_documents()
            .summary()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get EDI summary", e))?;
        Ok(summary.into())
    }
}
