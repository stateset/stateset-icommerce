//! SQLite implementation of the EDI document repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateEdiDocument, EdiAggregateSummary, EdiCount, EdiDirection, EdiDocument,
    EdiDocumentFilter, EdiDocumentId, EdiDocumentRepository, EdiStatus, Result,
};

#[derive(Debug)]
pub struct SqliteEdiDocumentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteEdiDocumentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<EdiDocument> {
        Ok(EdiDocument {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "edi_document", "id")?.into(),
            document_type: row.get("document_type")?,
            direction: parse_enum_row::<EdiDirection>(
                &row.get::<_, String>("direction")?,
                "edi_document",
                "direction",
            )?,
            status: parse_enum_row::<EdiStatus>(
                &row.get::<_, String>("status")?,
                "edi_document",
                "status",
            )?,
            partner: row.get("partner")?,
            reference: row.get("reference")?,
            payload: row.get("payload")?,
            error_message: row.get("error_message")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "edi_document",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "edi_document",
                "updated_at",
            )?,
        })
    }
}

impl EdiDocumentRepository for SqliteEdiDocumentRepository {
    fn create(&self, input: CreateEdiDocument) -> Result<EdiDocument> {
        let id = EdiDocumentId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO edi_documents (id, document_type, direction, status, partner, reference, payload, created_at, updated_at)
                 VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.document_type,
                    input.direction.to_string(),
                    &input.partner,
                    &input.reference,
                    &input.payload,
                    &now,
                    &now,
                ],
            )?;
            tx.query_row("SELECT * FROM edi_documents WHERE id = ?", [&id_str], Self::row_to_doc)
        })
    }

    fn get(&self, id: EdiDocumentId) -> Result<Option<EdiDocument>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM edi_documents WHERE id = ?",
            [id.to_string()],
            Self::row_to_doc,
        ) {
            Ok(d) => Ok(Some(d)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: EdiDocumentFilter) -> Result<Vec<EdiDocument>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM edi_documents WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(ref t) = filter.document_type {
            sql.push_str(" AND document_type = ?");
            params.push(Box::new(t.clone()));
        }
        if let Some(direction) = filter.direction {
            sql.push_str(" AND direction = ?");
            params.push(Box::new(direction.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(ref partner) = filter.partner {
            sql.push_str(" AND partner = ?");
            params.push(Box::new(partner.clone()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_doc)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn set_status(
        &self,
        id: EdiDocumentId,
        status: EdiStatus,
        error_message: Option<String>,
    ) -> Result<EdiDocument> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE edi_documents SET status = ?, error_message = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), &error_message, &now, &id_str],
            )?;
            tx.query_row("SELECT * FROM edi_documents WHERE id = ?", [&id_str], Self::row_to_doc)
        })
    }

    fn summary(&self) -> Result<EdiAggregateSummary> {
        let conn = self.conn()?;
        let total: u64 = conn
            .query_row("SELECT COUNT(*) FROM edi_documents", [], |r| r.get::<_, i64>(0))
            .map_err(map_db_error)? as u64;

        let collect = |sql: &str| -> Result<Vec<EdiCount>> {
            let mut stmt = conn.prepare(sql).map_err(map_db_error)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(EdiCount { key: r.get::<_, String>(0)?, count: r.get::<_, i64>(1)? as u64 })
                })
                .map_err(map_db_error)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(map_db_error)?;
            Ok(rows)
        };

        let by_status =
            collect("SELECT status, COUNT(*) FROM edi_documents GROUP BY status ORDER BY status")?;
        let by_type = collect(
            "SELECT document_type, COUNT(*) FROM edi_documents GROUP BY document_type ORDER BY document_type",
        )?;
        Ok(EdiAggregateSummary { total, by_status, by_type })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqliteEdiDocumentRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteEdiDocumentRepository::new(db.pool().clone())
    }

    fn create(
        repo: &SqliteEdiDocumentRepository,
        doc_type: &str,
        dir: EdiDirection,
    ) -> EdiDocument {
        repo.create(CreateEdiDocument {
            document_type: doc_type.into(),
            direction: dir,
            partner: Some("ACME-EDI".into()),
            reference: Some("PO-1001".into()),
            payload: Some("ISA*00*...".into()),
        })
        .expect("create")
    }

    #[test]
    fn create_get_and_default_status() {
        let repo = test_repo();
        let d = create(&repo, "850", EdiDirection::Inbound);
        assert_eq!(d.status, EdiStatus::Pending);
        let fetched = repo.get(d.id).expect("get").expect("found");
        assert_eq!(fetched.document_type, "850");
    }

    #[test]
    fn set_status_records_error() {
        let repo = test_repo();
        let d = create(&repo, "810", EdiDirection::Outbound);
        let errored =
            repo.set_status(d.id, EdiStatus::Error, Some("malformed segment".into())).expect("set");
        assert_eq!(errored.status, EdiStatus::Error);
        assert_eq!(errored.error_message.as_deref(), Some("malformed segment"));
    }

    #[test]
    fn list_filters() {
        let repo = test_repo();
        create(&repo, "850", EdiDirection::Inbound);
        create(&repo, "856", EdiDirection::Outbound);
        let inbound = repo
            .list(EdiDocumentFilter {
                direction: Some(EdiDirection::Inbound),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(inbound.len(), 1);
        let by_type = repo
            .list(EdiDocumentFilter { document_type: Some("856".into()), ..Default::default() })
            .expect("list");
        assert_eq!(by_type.len(), 1);
    }

    #[test]
    fn summary_groups_counts() {
        let repo = test_repo();
        create(&repo, "850", EdiDirection::Inbound);
        create(&repo, "850", EdiDirection::Inbound);
        let d = create(&repo, "810", EdiDirection::Outbound);
        repo.set_status(d.id, EdiStatus::Error, None).expect("set");

        let summary = repo.summary().expect("summary");
        assert_eq!(summary.total, 3);
        // by_type: 810 -> 1, 850 -> 2
        let t850 = summary.by_type.iter().find(|c| c.key == "850").unwrap();
        assert_eq!(t850.count, 2);
        // by_status: error -> 1, pending -> 2
        let err = summary.by_status.iter().find(|c| c.key == "error").unwrap();
        assert_eq!(err.count, 1);
    }
}
