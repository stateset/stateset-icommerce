//! Error conversion helpers for database backends
//!
//! This module provides utilities for converting database-specific errors
//! to the unified DbError type from stateset-core.

use stateset_core::{CommerceError, DbError};

// ============================================================================
// SQLite Error Conversion
// ============================================================================

#[cfg(feature = "sqlite")]
pub mod sqlite {
    use super::*;
    use rusqlite::Error as SqliteError;

    /// Convert a rusqlite error to CommerceError with context
    pub fn map_error(table: &'static str, operation: &'static str, err: SqliteError) -> CommerceError {
        match &err {
            SqliteError::SqliteFailure(ffi_err, msg) => {
                // Check for constraint violations
                match ffi_err.code {
                    rusqlite::ErrorCode::ConstraintViolation => {
                        let constraint = msg.as_ref().map(|s| s.as_str()).unwrap_or("unknown");
                        CommerceError::Database(DbError::ConstraintViolation {
                            table,
                            constraint: extract_constraint_name(constraint).to_string(),
                            message: err.to_string(),
                        })
                    }
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                        CommerceError::Database(DbError::TransactionFailed {
                            message: format!("Database busy/locked during {}: {}", operation, err),
                        })
                    }
                    rusqlite::ErrorCode::CannotOpen => {
                        CommerceError::Database(DbError::ConnectionFailed {
                            url: "sqlite".to_string(),
                            message: err.to_string(),
                        })
                    }
                    _ => CommerceError::Database(DbError::QueryFailed {
                        table,
                        operation,
                        message: err.to_string(),
                    }),
                }
            }
            SqliteError::QueryReturnedNoRows => {
                // This is often not an error - let the caller handle it
                CommerceError::NotFound
            }
            SqliteError::InvalidColumnType(col, col_name, expected) => {
                CommerceError::Database(DbError::SerializationError {
                    field: col_name.clone(),
                    message: format!(
                        "Column {} has invalid type at index {}, expected {:?}",
                        col_name, col, expected
                    ),
                })
            }
            SqliteError::InvalidColumnName(name) => {
                CommerceError::Database(DbError::QueryFailed {
                    table,
                    operation,
                    message: format!("Invalid column name: {}", name),
                })
            }
            SqliteError::FromSqlConversionFailure(col, _, err) => {
                CommerceError::Database(DbError::SerializationError {
                    field: format!("column_{}", col),
                    message: err.to_string(),
                })
            }
            _ => CommerceError::Database(DbError::QueryFailed {
                table,
                operation,
                message: err.to_string(),
            }),
        }
    }

    /// Convert an r2d2 pool error to CommerceError
    pub fn map_pool_error(err: r2d2::Error) -> CommerceError {
        CommerceError::Database(DbError::PoolExhausted {
            timeout_ms: 30000, // Default timeout
        })
    }

    /// Convert a connection get error to CommerceError
    pub fn map_connection_error<E: std::fmt::Display>(err: E) -> CommerceError {
        CommerceError::Database(DbError::ConnectionFailed {
            url: "sqlite-pool".to_string(),
            message: err.to_string(),
        })
    }

    /// Extract constraint name from SQLite error message
    fn extract_constraint_name(msg: &str) -> &str {
        // SQLite constraint errors often look like "UNIQUE constraint failed: table.column"
        if let Some(idx) = msg.find(':') {
            msg[..idx].trim()
        } else {
            msg
        }
    }

    /// Helper macro for mapping SQLite errors with context
    #[macro_export]
    macro_rules! map_sqlite_err {
        ($table:expr, $op:expr, $result:expr) => {
            $result.map_err(|e| $crate::error_helpers::sqlite::map_error($table, $op, e))
        };
    }
}

// ============================================================================
// PostgreSQL Error Conversion
// ============================================================================

#[cfg(feature = "postgres")]
pub mod postgres {
    use super::*;
    use sqlx::Error as PgError;

    /// Convert a sqlx error to CommerceError with context
    pub fn map_error(table: &'static str, operation: &'static str, err: PgError) -> CommerceError {
        match &err {
            PgError::Database(db_err) => {
                // Check for specific PostgreSQL error codes
                if let Some(code) = db_err.code() {
                    let code_str = code.code();

                    // Class 23 - Integrity Constraint Violation
                    if code_str.starts_with("23") {
                        let constraint = db_err.constraint().unwrap_or("unknown").to_string();
                        return CommerceError::Database(DbError::ConstraintViolation {
                            table,
                            constraint,
                            message: db_err.message().to_string(),
                        });
                    }

                    // Class 08 - Connection Exception
                    if code_str.starts_with("08") {
                        return CommerceError::Database(DbError::ConnectionFailed {
                            url: "postgres".to_string(),
                            message: db_err.message().to_string(),
                        });
                    }

                    // Class 40 - Transaction Rollback
                    if code_str.starts_with("40") {
                        return CommerceError::Database(DbError::TransactionFailed {
                            message: db_err.message().to_string(),
                        });
                    }
                }

                CommerceError::Database(DbError::QueryFailed {
                    table,
                    operation,
                    message: db_err.message().to_string(),
                })
            }
            PgError::RowNotFound => CommerceError::NotFound,
            PgError::PoolTimedOut => {
                CommerceError::Database(DbError::PoolExhausted {
                    timeout_ms: 30000, // Default timeout
                })
            }
            PgError::PoolClosed => {
                CommerceError::Database(DbError::ConnectionFailed {
                    url: "postgres-pool".to_string(),
                    message: "Connection pool is closed".to_string(),
                })
            }
            PgError::Io(io_err) => {
                CommerceError::Database(DbError::ConnectionFailed {
                    url: "postgres".to_string(),
                    message: io_err.to_string(),
                })
            }
            PgError::Decode(decode_err) => {
                CommerceError::Database(DbError::SerializationError {
                    field: "unknown".to_string(),
                    message: decode_err.to_string(),
                })
            }
            _ => CommerceError::Database(DbError::QueryFailed {
                table,
                operation,
                message: err.to_string(),
            }),
        }
    }

    /// Convert a migration error to CommerceError
    pub fn map_migration_error(version: i32, err: impl std::fmt::Display) -> CommerceError {
        CommerceError::Database(DbError::MigrationFailed {
            version,
            message: err.to_string(),
        })
    }

    /// Helper macro for mapping PostgreSQL errors with context
    #[macro_export]
    macro_rules! map_pg_err {
        ($table:expr, $op:expr, $result:expr) => {
            $result.map_err(|e| $crate::error_helpers::postgres::map_error($table, $op, e))
        };
    }
}

// ============================================================================
// Common Error Helpers
// ============================================================================

/// Map a JSON serialization error
pub fn map_json_error(field: &str, err: impl std::fmt::Display) -> CommerceError {
    CommerceError::Database(DbError::SerializationError {
        field: field.to_string(),
        message: err.to_string(),
    })
}

/// Map a generic database error
pub fn map_db_error(msg: impl Into<String>) -> CommerceError {
    CommerceError::Database(DbError::Other(msg.into()))
}

/// Create a migration failed error
pub fn migration_failed(version: i32, msg: impl Into<String>) -> CommerceError {
    CommerceError::Database(DbError::MigrationFailed {
        version,
        message: msg.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "sqlite")]
    fn test_map_sqlite_not_found() {
        let err = rusqlite::Error::QueryReturnedNoRows;
        let result = sqlite::map_error("orders", "get", err);
        assert!(result.is_not_found());
    }

    #[test]
    fn test_map_json_error() {
        let err = map_json_error("metadata", "invalid JSON");
        assert!(err.is_database());
    }
}
