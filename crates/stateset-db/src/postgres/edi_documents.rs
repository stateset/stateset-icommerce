//! PostgreSQL EDI document repository implementation

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateEdiDocument, EdiAggregateSummary, EdiCount, EdiDirection, EdiDocument,
    EdiDocumentFilter, EdiDocumentId, EdiDocumentRepository, EdiStatus, Result,
};
use uuid::Uuid;

/// PostgreSQL implementation of `EdiDocumentRepository`
#[derive(Debug, Clone)]
pub struct PgEdiDocumentRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct EdiDocumentRow {
    id: Uuid,
    document_type: String,
    direction: String,
    status: String,
    partner: Option<String>,
    reference: Option<String>,
    payload: Option<String>,
    error_message: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgEdiDocumentRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_doc(row: EdiDocumentRow) -> Result<EdiDocument> {
        let direction: EdiDirection = row.direction.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid edi_document.direction '{}': {}",
                row.direction, e
            ))
        })?;
        let status: EdiStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid edi_document.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(EdiDocument {
            id: row.id.into(),
            document_type: row.document_type,
            direction,
            status,
            partner: row.partner,
            reference: row.reference,
            payload: row.payload,
            error_message: row.error_message,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    async fn fetch_async(&self, id: Uuid) -> Result<Option<EdiDocument>> {
        let row = sqlx::query_as::<_, EdiDocumentRow>("SELECT * FROM edi_documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_doc).transpose()
    }

    /// Create / ingest an EDI document (async); status starts as `pending`.
    pub async fn create_async(&self, input: CreateEdiDocument) -> Result<EdiDocument> {
        let id = EdiDocumentId::new();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO edi_documents (id, document_type, direction, status, partner, reference, payload, created_at, updated_at)
             VALUES ($1, $2, $3, 'pending', $4, $5, $6, $7, $7)",
        )
        .bind(Uuid::from(id))
        .bind(&input.document_type)
        .bind(input.direction.to_string())
        .bind(&input.partner)
        .bind(&input.reference)
        .bind(&input.payload)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a document by ID (async)
    pub async fn get_async(&self, id: EdiDocumentId) -> Result<Option<EdiDocument>> {
        self.fetch_async(id.into()).await
    }

    /// List documents with filter (async)
    pub async fn list_async(&self, filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>> {
        let limit = super::effective_limit(filter.limit);
        let offset = i64::from(filter.offset.unwrap_or(0));

        let mut query = String::from("SELECT * FROM edi_documents WHERE 1=1");
        let mut param_idx = 1;
        if filter.document_type.is_some() {
            query.push_str(&format!(" AND document_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.direction.is_some() {
            query.push_str(&format!(" AND direction = ${param_idx}"));
            param_idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${param_idx}"));
            param_idx += 1;
        }
        if filter.partner.is_some() {
            query.push_str(&format!(" AND partner = ${param_idx}"));
            param_idx += 1;
        }
        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_idx,
            param_idx + 1
        ));

        let mut q = sqlx::query_as::<_, EdiDocumentRow>(&query);
        if let Some(ref document_type) = filter.document_type {
            q = q.bind(document_type);
        }
        if let Some(direction) = filter.direction {
            q = q.bind(direction.to_string());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(ref partner) = filter.partner {
            q = q.bind(partner);
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_doc).collect()
    }

    /// Update a document's status, optionally recording an error message (async)
    pub async fn set_status_async(
        &self,
        id: EdiDocumentId,
        status: EdiStatus,
        error_message: Option<String>,
    ) -> Result<EdiDocument> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE edi_documents SET status = $1, error_message = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(status.to_string())
        .bind(&error_message)
        .bind(now)
        .bind(Uuid::from(id))
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_async(id.into()).await?.ok_or(CommerceError::NotFound)
    }

    /// Aggregate summary (counts by status and type) across all documents (async)
    pub async fn summary_async(&self) -> Result<EdiAggregateSummary> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM edi_documents")
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;

        let by_status: Vec<(String, i64)> = sqlx::query_as(
            "SELECT status, COUNT(*) FROM edi_documents GROUP BY status ORDER BY status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let by_type: Vec<(String, i64)> = sqlx::query_as(
            "SELECT document_type, COUNT(*) FROM edi_documents GROUP BY document_type ORDER BY document_type",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let to_counts = |rows: Vec<(String, i64)>| -> Vec<EdiCount> {
            rows.into_iter()
                .map(|(key, count)| EdiCount { key, count: count.unsigned_abs() })
                .collect()
        };
        Ok(EdiAggregateSummary {
            total: total.unsigned_abs(),
            by_status: to_counts(by_status),
            by_type: to_counts(by_type),
        })
    }
}

impl EdiDocumentRepository for PgEdiDocumentRepository {
    fn create(&self, input: CreateEdiDocument) -> Result<EdiDocument> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: EdiDocumentId) -> Result<Option<EdiDocument>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>> {
        super::block_on(self.list_async(filter))
    }

    fn set_status(
        &self,
        id: EdiDocumentId,
        status: EdiStatus,
        error_message: Option<String>,
    ) -> Result<EdiDocument> {
        super::block_on(self.set_status_async(id, status, error_message))
    }

    fn summary(&self) -> Result<EdiAggregateSummary> {
        super::block_on(self.summary_async())
    }
}
