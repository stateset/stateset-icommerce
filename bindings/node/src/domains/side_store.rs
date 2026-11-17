//! Side stores: a synchronous SQLite handle on the engine's own database file,
//! for auxiliary tables the engine does not own (the sync outbox, an import
//! id map, a marketplace bridge inbox, ...).
//!
//! # Why this exists
//!
//! Host code used to open the engine's store file with `better-sqlite3`, which
//! bundles its own copy of SQLite. The engine links another. POSIX advisory
//! locks belong to the process, while each SQLite copy keeps its own lock
//! bookkeeping, so when the second copy closed its connection it checkpointed,
//! unlinked `-wal`/`-shm` and released locks the engine still believed it
//! held. Later engine commits became invisible to new connections and the file
//! eventually reported "database disk image is malformed".
//!
//! A side store is opened by the engine's own SQLite
//! (`Commerce::open_auxiliary_sqlite_connection`), so one library copy owns
//! the file and closing a side store can never drop the engine's locks.
//!
//! # Shape
//!
//! `SideStore` and `SideStoreStatement` expose the subset of the
//! `better-sqlite3` API the CLI's side stores use: `prepare` → `run` / `get` /
//! `all`, `exec`, `pragma`, `transaction` and `close`. The variadic,
//! better-sqlite3-style argument handling and `transaction()` live in
//! `index.js` on top of the internal `__exec` / `__pragma` / `__run` /
//! `__get` / `__all` methods here. Values bind and read back exactly as
//! better-sqlite3 binds them by default: a JavaScript number binds as a REAL,
//! a bigint as an INTEGER, a string as TEXT, a Buffer as a BLOB and
//! `null`/`undefined` as NULL; an INTEGER reads back as a number (lossy past
//! 2^53, as in better-sqlite3 without `safeIntegers`).
//!
//! # Lifetime
//!
//! Each side store owns one connection, outside the engine's pool, so holding
//! it costs the engine no pooled capacity. `Commerce.close()` closes every
//! side store opened from it before it resolves; afterwards every call on the
//! side store or its statements fails with `PRECONDITION_FAILED`.

#![allow(clippy::wildcard_imports)]

use super::*;

use napi::{JsObject, JsUnknown, Status, ValueType};
use rusqlite::types::{Value, ValueRef};
use stateset_db::sqlite::rusqlite;
use std::sync::{Mutex, PoisonError, Weak};

/// Prepared statements each side-store connection keeps compiled. Statements
/// are re-prepared through this cache on every call (a napi object cannot
/// hold a borrowing `rusqlite::Statement`), and the sync outbox alone uses
/// several dozen distinct statements.
const STATEMENT_CACHE_CAPACITY: usize = 256;

/// One side-store connection, shared by the `SideStore` and every statement
/// prepared from it. `None` once closed.
pub(crate) struct SideConnection {
    conn: Mutex<Option<rusqlite::Connection>>,
}

impl SideConnection {
    fn close(&self) {
        // Dropping the connection rolls back an open transaction and closes
        // the file through the engine's SQLite, which never releases locks
        // another connection of the same library still holds.
        drop(self.conn.lock().unwrap_or_else(PoisonError::into_inner).take());
    }

    fn is_open(&self) -> bool {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }
}

/// Every open side store, by the engine handle it came from, so
/// `Commerce.close()` can close them before the engine releases its files.
type Registry = Vec<(Weak<EngineHandle>, Weak<SideConnection>)>;
static REGISTRY: Mutex<Registry> = Mutex::new(Vec::new());

/// Record a new side store, unless its engine has already closed.
///
/// `Commerce.close()` empties the engine slot *before* it sweeps the registry,
/// and both this check and the sweep run under the registry lock, so a side
/// store opened concurrently with `close()` is either swept or refused here;
/// it can never outlive the engine unnoticed.
fn register(engine: &Handle, connection: &Arc<SideConnection>) -> Result<()> {
    let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
    if engine.is_closed() {
        connection.close();
        return Err(coded(ErrCode::PreconditionFailed, "Commerce instance is closed"));
    }
    registry.retain(|(engine, side)| engine.strong_count() > 0 && side.strong_count() > 0);
    registry.push((Arc::downgrade(engine), Arc::downgrade(connection)));
    Ok(())
}

/// Close every side store opened from `engine`. Called by `Commerce.close()`
/// after it has emptied the engine slot and before it waits for the engine to
/// drop, so the last connection to close (and so the one that checkpoints and
/// removes the WAL) is closed by the time `close()` resolves.
pub(crate) fn close_side_stores(engine: &Handle) {
    let doomed: Vec<Arc<SideConnection>> = {
        let mut registry = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
        let mut doomed = Vec::new();
        registry.retain(|(owner, side)| {
            let Some(side) = side.upgrade() else { return false };
            if std::ptr::eq(owner.as_ptr(), Arc::as_ptr(engine)) {
                doomed.push(side);
                return false;
            }
            owner.strong_count() > 0
        });
        doomed
    };
    for side in doomed {
        side.close();
    }
}

/// The error for a call on a closed side store, naming which side closed.
fn closed_error(engine: &Handle) -> Error {
    if engine.is_closed() {
        coded(ErrCode::PreconditionFailed, "Commerce instance is closed")
    } else {
        coded(ErrCode::PreconditionFailed, "The side store is closed")
    }
}

/// Run `operation` on the open connection, or fail with [`closed_error`].
fn with_connection<T>(
    engine: &Handle,
    connection: &SideConnection,
    operation: impl FnOnce(&rusqlite::Connection) -> Result<T>,
) -> Result<T> {
    let guard = connection.conn.lock().unwrap_or_else(PoisonError::into_inner);
    match guard.as_ref() {
        Some(conn) => operation(conn),
        None => Err(closed_error(engine)),
    }
}

// ----------------------------------------------------------------------------
// SQLite errors
// ----------------------------------------------------------------------------

/// The `SQLITE_*` name of a (possibly extended) result code, as better-sqlite3
/// reports it in `err.code`.
fn sqlite_code_name(code: i32) -> String {
    let primary = code & 0xff;
    let extended = code >> 8;
    let name = |base: &str, variants: &[&str]| -> String {
        match usize::try_from(extended) {
            Ok(0) => format!("SQLITE_{base}"),
            Ok(index) => variants.get(index - 1).filter(|variant| !variant.is_empty()).map_or_else(
                || format!("SQLITE_{base}"),
                |variant| format!("SQLITE_{base}_{variant}"),
            ),
            Err(_) => format!("SQLITE_{base}"),
        }
    };
    match primary {
        0 => name("OK", &["LOAD_PERMANENTLY", "SYMLINK"]),
        1 => {
            name("ERROR", &["MISSING_COLLSEQ", "RETRY", "SNAPSHOT", "RESERVESIZE", "KEY", "UNABLE"])
        }
        2 => name("INTERNAL", &[]),
        3 => name("PERM", &[]),
        // `SQLITE_ABORT_ROLLBACK` is extended code 2; 1 is unassigned.
        4 => name("ABORT", &["", "ROLLBACK"]),
        5 => name("BUSY", &["RECOVERY", "SNAPSHOT", "TIMEOUT"]),
        6 => name("LOCKED", &["SHAREDCACHE", "VTAB"]),
        7 => name("NOMEM", &[]),
        8 => name(
            "READONLY",
            &["RECOVERY", "CANTLOCK", "ROLLBACK", "DBMOVED", "CANTINIT", "DIRECTORY"],
        ),
        9 => name("INTERRUPT", &[]),
        10 => name(
            "IOERR",
            &[
                "READ",
                "SHORT_READ",
                "WRITE",
                "FSYNC",
                "DIR_FSYNC",
                "TRUNCATE",
                "FSTAT",
                "UNLOCK",
                "RDLOCK",
                "DELETE",
                "BLOCKED",
                "NOMEM",
                "ACCESS",
                "CHECKRESERVEDLOCK",
                "LOCK",
                "CLOSE",
                "DIR_CLOSE",
                "SHMOPEN",
                "SHMSIZE",
                "SHMLOCK",
                "SHMMAP",
                "SEEK",
                "DELETE_NOENT",
                "MMAP",
                "GETTEMPPATH",
                "CONVPATH",
                "VNODE",
                "AUTH",
                "BEGIN_ATOMIC",
                "COMMIT_ATOMIC",
                "ROLLBACK_ATOMIC",
                "DATA",
                "CORRUPTFS",
                "IN_PAGE",
            ],
        ),
        11 => name("CORRUPT", &["VTAB", "SEQUENCE", "INDEX"]),
        12 => name("NOTFOUND", &[]),
        13 => name("FULL", &[]),
        14 => {
            name("CANTOPEN", &["NOTEMPDIR", "ISDIR", "FULLPATH", "CONVPATH", "DIRTYWAL", "SYMLINK"])
        }
        15 => name("PROTOCOL", &[]),
        16 => name("EMPTY", &[]),
        17 => name("SCHEMA", &[]),
        18 => name("TOOBIG", &[]),
        19 => name(
            "CONSTRAINT",
            &[
                "CHECK",
                "COMMITHOOK",
                "FOREIGNKEY",
                "FUNCTION",
                "NOTNULL",
                "PRIMARYKEY",
                "TRIGGER",
                "UNIQUE",
                "VTAB",
                "ROWID",
                "PINNED",
                "DATATYPE",
            ],
        ),
        20 => name("MISMATCH", &[]),
        21 => name("MISUSE", &[]),
        22 => name("NOLFS", &[]),
        23 => name("AUTH", &["USER"]),
        24 => name("FORMAT", &[]),
        25 => name("RANGE", &[]),
        26 => name("NOTADB", &[]),
        27 => name("NOTICE", &["RECOVER_WAL", "RECOVER_ROLLBACK", "RBU"]),
        28 => name("WARNING", &["AUTOINDEX"]),
        100 => "SQLITE_ROW".to_owned(),
        101 => "SQLITE_DONE".to_owned(),
        _ => format!("UNKNOWN_SQLITE_ERROR_{code}"),
    }
}

/// A SQLite failure as a JavaScript error: `err.code` is the extended result
/// code name (`SQLITE_CONSTRAINT_UNIQUE`, ...) exactly as better-sqlite3
/// reports it, `err.message` is SQLite's own message, and `err.details`
/// carries the numeric code.
fn sqlite_error(error: rusqlite::Error) -> Error {
    let (code, message) = match &error {
        rusqlite::Error::SqliteFailure(failure, message) => {
            (failure.extended_code, message.clone().unwrap_or_else(|| failure.to_string()))
        }
        rusqlite::Error::SqlInputError { error: failure, msg, .. } => {
            (failure.extended_code, msg.clone())
        }
        _ => return coded(ErrCode::Database, error),
    };
    let payload = serde_json::json!({
        "code": sqlite_code_name(code),
        "message": message,
        "details": { "sqliteCode": code, "httpStatus": 500 },
    });
    Error::new(Status::GenericFailure, payload.to_string())
}

// ----------------------------------------------------------------------------
// Values
// ----------------------------------------------------------------------------

/// Bind one JavaScript value the way better-sqlite3 does.
fn to_sql_value(value: JsUnknown) -> Result<Value> {
    match value.get_type()? {
        ValueType::Undefined | ValueType::Null => Ok(Value::Null),
        ValueType::Number => Ok(Value::Real(f64::from_unknown(value)?)),
        ValueType::String => Ok(Value::Text(String::from_unknown(value)?)),
        ValueType::BigInt => match BigInt::from_unknown(value)?.get_i64() {
            (integer, true) => Ok(Value::Integer(integer)),
            (_, false) => Err(coded(
                ErrCode::Validation,
                "The bound bigint does not fit in a 64-bit signed integer",
            )),
        },
        ValueType::Object if value.is_buffer()? => {
            Ok(Value::Blob(Buffer::from_unknown(value)?.to_vec()))
        }
        _ => Err(coded(
            ErrCode::Validation,
            "SQLite3 can only bind numbers, strings, bigints, buffers, and null",
        )),
    }
}

/// An owned copy of one column value. TEXT that is not valid UTF-8 is read
/// with replacement characters, as V8 does for better-sqlite3.
fn owned_value(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(integer) => Value::Integer(integer),
        ValueRef::Real(real) => Value::Real(real),
        ValueRef::Text(text) => Value::Text(String::from_utf8_lossy(text).into_owned()),
        ValueRef::Blob(blob) => Value::Blob(blob.to_vec()),
    }
}

/// Read a column value back the way better-sqlite3 does by default.
fn to_js_value(env: &Env, value: Value) -> Result<JsUnknown> {
    Ok(match value {
        Value::Null => env.get_null()?.into_unknown(),
        // better-sqlite3 without `safeIntegers` hands back a double.
        #[allow(clippy::cast_precision_loss)]
        Value::Integer(integer) => env.create_double(integer as f64)?.into_unknown(),
        Value::Real(real) => env.create_double(real)?.into_unknown(),
        Value::Text(text) => env.create_string_from_std(text)?.into_unknown(),
        Value::Blob(blob) => env.create_buffer_with_data(blob)?.into_raw().into_unknown(),
    })
}

/// One result row as an object keyed by column name. A repeated name keeps
/// its last value, as in better-sqlite3.
fn to_js_row(env: &Env, columns: &[String], row: Vec<Value>) -> Result<JsObject> {
    let mut object = env.create_object()?;
    for (column, value) in columns.iter().zip(row) {
        object.set_named_property(column, to_js_value(env, value)?)?;
    }
    Ok(object)
}

fn read_row(row: &rusqlite::Row<'_>, width: usize) -> rusqlite::Result<Vec<Value>> {
    (0..width).map(|index| row.get_ref(index).map(owned_value)).collect()
}

fn column_names(statement: &rusqlite::Statement<'_>) -> Vec<String> {
    statement.column_names().into_iter().map(str::to_owned).collect()
}

/// Rows a statement produced, owned, so no JavaScript runs while the
/// connection lock is held.
struct Rows {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

impl Rows {
    fn into_js_array(self, env: &Env) -> Result<JsObject> {
        let mut array = env.create_array_with_length(self.rows.len())?;
        for (index, row) in self.rows.into_iter().enumerate() {
            let index = u32::try_from(index)
                .map_err(|_| coded(ErrCode::Validation, "Too many rows for one array"))?;
            array.set_element(index, to_js_row(env, &self.columns, row)?)?;
        }
        Ok(array)
    }
}

// ----------------------------------------------------------------------------
// Commerce entry point
// ----------------------------------------------------------------------------

#[napi]
impl Commerce {
    /// Open a synchronous SQLite handle on this engine's own database file,
    /// for auxiliary tables the engine does not own (a sync outbox, an import
    /// id map, a bridge inbox). It is not a way to read or write commerce
    /// data: writing engine tables through it bypasses every engine
    /// invariant.
    ///
    /// It speaks the subset of the `better-sqlite3` `Database` API side stores
    /// use (`prepare` → `run`/`get`/`all`, `exec`, `pragma`, `transaction`,
    /// `close`), so a store written against better-sqlite3 can take it in
    /// place of a `Database`. Use it instead of opening the store file with
    /// `better-sqlite3` (or any other SQLite library) in a process that also
    /// has the engine open: a second SQLite copy drops the engine's file locks
    /// when it closes and can corrupt the database.
    ///
    /// Works for `:memory:` (it reaches the engine's private temp file). Each
    /// call opens a new connection outside the engine's pool. `close()` on the
    /// handle closes only that connection; `Commerce.close()` closes every
    /// side store opened from it. Not available on a PostgreSQL backend.
    #[napi]
    pub fn open_side_store(&self) -> Result<SideStore> {
        guard(|| {
            let engine = self.inner.get()?;
            let conn = engine
                .open_auxiliary_sqlite_connection()
                .map_err(|error| wrap(ErrCode::Database, "Failed to open side store", error))?;
            drop(engine);
            conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE_CAPACITY);
            let connection = Arc::new(SideConnection { conn: Mutex::new(Some(conn)) });
            register(&self.inner, &connection)?;
            Ok(SideStore { engine: self.inner.clone(), connection })
        })
    }
}

// ----------------------------------------------------------------------------
// SideStore
// ----------------------------------------------------------------------------

/// A synchronous SQLite handle on the engine's own database file, for
/// auxiliary tables. Opened by `Commerce.openSideStore()`; see there.
#[napi]
pub struct SideStore {
    engine: Handle,
    connection: Arc<SideConnection>,
}

#[napi]
impl SideStore {
    /// Compile one SQL statement. Throws on a syntax error, on SQL that holds
    /// no statement, and on SQL that holds more than one (use `exec` for a
    /// script).
    #[napi]
    pub fn prepare(&self, sql: String) -> Result<SideStoreStatement> {
        guard(|| {
            let (parameters, reader) = with_connection(&self.engine, &self.connection, |conn| {
                let mut batch = rusqlite::Batch::new(conn, &sql);
                let first = batch.next().map_err(sqlite_error)?.ok_or_else(|| {
                    coded(ErrCode::Validation, "The supplied SQL string contains no statements")
                })?;
                let reader = first.column_count() > 0;
                let parameters = (1..=first.parameter_count())
                    .map(|index| match first.parameter_name(index) {
                        Some(name) if !name.starts_with('?') => {
                            Parameter::Named(name.get(1..).unwrap_or_default().to_owned())
                        }
                        _ => Parameter::Anonymous,
                    })
                    .collect::<Vec<_>>();
                drop(first);
                // A tail that is only whitespace, `;` or comments prepares to
                // nothing; anything else (even SQL that fails to compile) is a
                // second statement.
                if !matches!(batch.next(), Ok(None)) {
                    return Err(coded(
                        ErrCode::Validation,
                        "The supplied SQL string contains more than one statement",
                    ));
                }
                Ok((parameters, reader))
            })?;
            Ok(SideStoreStatement {
                engine: self.engine.clone(),
                connection: self.connection.clone(),
                sql,
                parameters,
                reader,
            })
        })
    }

    /// Run a script of zero or more statements. Internal: `exec()` in
    /// `index.js` calls this and returns the side store, as better-sqlite3
    /// does.
    #[napi(js_name = "__exec")]
    pub fn exec(&self, sql: String) -> Result<()> {
        guard(|| {
            with_connection(&self.engine, &self.connection, |conn| {
                conn.execute_batch(&sql).map_err(sqlite_error)
            })
        })
    }

    /// Run `PRAGMA <source>`. With `simple`, the first column of the first
    /// row (or `undefined`); otherwise every row. Internal: `pragma()` in
    /// `index.js` validates the arguments.
    #[napi(js_name = "__pragma", ts_return_type = "unknown")]
    pub fn pragma(&self, env: Env, source: String, simple: bool) -> Result<JsUnknown> {
        guard(|| {
            let rows = with_connection(&self.engine, &self.connection, |conn| {
                let collect = || -> rusqlite::Result<Rows> {
                    let mut statement = conn.prepare(&format!("PRAGMA {source}"))?;
                    let columns = column_names(&statement);
                    let mut rows = Vec::new();
                    let mut cursor = statement.raw_query();
                    while let Some(row) = cursor.next()? {
                        rows.push(read_row(row, columns.len())?);
                        if simple {
                            break;
                        }
                    }
                    Ok(Rows { columns, rows })
                };
                collect().map_err(sqlite_error)
            })?;
            if simple {
                return match rows.rows.into_iter().next().and_then(|row| row.into_iter().next()) {
                    Some(value) => to_js_value(&env, value),
                    None => Ok(env.get_undefined()?.into_unknown()),
                };
            }
            Ok(rows.into_js_array(&env)?.into_unknown())
        })
    }

    /// Close this side store's connection, rolling back any open transaction.
    /// The engine and other side stores are unaffected, and so are the
    /// engine's file locks and WAL. Idempotent.
    #[napi]
    pub fn close(&self) {
        self.connection.close();
    }

    /// Whether the side store can still run statements: `false` after its own
    /// `close()` or its engine's `Commerce.close()`.
    #[napi(getter)]
    pub fn open(&self) -> bool {
        self.connection.is_open() && !self.engine.is_closed()
    }

    /// Whether a transaction is open on this side store's connection.
    #[napi(getter)]
    pub fn in_transaction(&self) -> bool {
        self.connection
            .conn
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|conn| !conn.is_autocommit())
    }
}

// ----------------------------------------------------------------------------
// SideStoreStatement
// ----------------------------------------------------------------------------

/// How one SQL parameter is bound: by position (`?`, `?NNN`) or by name
/// (`:name`, `@name`, `$name`, looked up without the prefix).
enum Parameter {
    Anonymous,
    Named(String),
}

/// What `run()` reports, as better-sqlite3 does.
#[napi(object)]
pub struct SideStoreRunResult {
    /// Rows inserted, updated or deleted by this statement (0 for a statement
    /// that changed nothing, even right after one that did).
    pub changes: i64,
    /// The connection's last inserted rowid.
    pub last_insert_rowid: i64,
}

/// A statement compiled by `SideStore.prepare()`. Usable until its side store
/// or engine closes.
#[napi]
pub struct SideStoreStatement {
    engine: Handle,
    connection: Arc<SideConnection>,
    sql: String,
    parameters: Vec<Parameter>,
    reader: bool,
}

impl SideStoreStatement {
    /// Resolve the JavaScript arguments against this statement's parameters,
    /// with better-sqlite3's rules and messages. Runs before the connection
    /// is locked: reading a named parameter can run a JavaScript getter.
    fn bind_values(
        &self,
        positional: Vec<JsUnknown>,
        named: Option<JsObject>,
    ) -> Result<Vec<Value>> {
        let anonymous =
            self.parameters.iter().filter(|parameter| matches!(parameter, Parameter::Anonymous));
        let anonymous = anonymous.count();
        if positional.len() > anonymous {
            return Err(coded(ErrCode::Validation, "Too many parameter values were provided"));
        }
        let has_named = anonymous < self.parameters.len();
        if positional.len() < anonymous {
            return Err(coded(
                ErrCode::Validation,
                if has_named && named.is_none() {
                    "Missing named parameters"
                } else {
                    "Too few parameter values were provided"
                },
            ));
        }
        if has_named && named.is_none() {
            return Err(coded(ErrCode::Validation, "Missing named parameters"));
        }
        let mut positional = positional.into_iter();
        let mut values = Vec::with_capacity(self.parameters.len());
        for parameter in &self.parameters {
            let value = match parameter {
                Parameter::Anonymous => positional.next().ok_or_else(|| {
                    coded(ErrCode::Validation, "Too few parameter values were provided")
                })?,
                Parameter::Named(name) => {
                    let object = named
                        .as_ref()
                        .ok_or_else(|| coded(ErrCode::Validation, "Missing named parameters"))?;
                    if !object.has_own_property(name)? {
                        return Err(coded(
                            ErrCode::Validation,
                            format!("Missing named parameter \"{name}\""),
                        ));
                    }
                    object.get_named_property::<JsUnknown>(name)?
                }
            };
            values.push(to_sql_value(value)?);
        }
        Ok(values)
    }

    /// Bind `values` and hand the statement to `operation` under the
    /// connection lock.
    fn execute<T>(
        &self,
        values: &[Value],
        operation: impl FnOnce(
            &rusqlite::Connection,
            &mut rusqlite::CachedStatement<'_>,
        ) -> rusqlite::Result<T>,
    ) -> Result<T> {
        with_connection(&self.engine, &self.connection, |conn| {
            let run = || -> rusqlite::Result<T> {
                let mut statement = conn.prepare_cached(&self.sql)?;
                for (index, value) in values.iter().enumerate() {
                    statement.raw_bind_parameter(index + 1, value)?;
                }
                operation(conn, &mut statement)
            };
            run().map_err(sqlite_error)
        })
    }

    fn require_reader(&self) -> Result<()> {
        if self.reader {
            Ok(())
        } else {
            Err(coded(
                ErrCode::Validation,
                "This statement does not return data. Use run() instead",
            ))
        }
    }
}

#[napi]
impl SideStoreStatement {
    /// The SQL this statement was prepared from.
    #[napi(getter)]
    pub fn source(&self) -> String {
        self.sql.clone()
    }

    /// Whether the statement returns rows (`get`/`all` need one that does).
    #[napi(getter)]
    pub fn reader(&self) -> bool {
        self.reader
    }

    /// Execute once. Internal: `run(...params)` in `index.js` sorts the
    /// arguments into positional and named values.
    #[napi(
        js_name = "__run",
        ts_args_type = "positional: Array<unknown>, named?: Record<string, unknown> | null"
    )]
    pub fn run(
        &self,
        positional: Vec<JsUnknown>,
        named: Option<JsObject>,
    ) -> Result<SideStoreRunResult> {
        guard(|| {
            let values = self.bind_values(positional, named)?;
            let (changes, last_insert_rowid) = self.execute(&values, |conn, statement| {
                let before = conn.total_changes();
                // One step, as better-sqlite3's `run()` takes: enough to
                // finish a write (and an `INSERT ... RETURNING`), harmless on
                // a read.
                statement.raw_query().next()?;
                let changes = if conn.total_changes() == before { 0 } else { conn.changes() };
                Ok((changes, conn.last_insert_rowid()))
            })?;
            Ok(SideStoreRunResult {
                changes: i64::try_from(changes).unwrap_or(i64::MAX),
                last_insert_rowid,
            })
        })
    }

    /// The first row, or `undefined`. Internal: see `__run`.
    #[napi(
        js_name = "__get",
        ts_args_type = "positional: Array<unknown>, named?: Record<string, unknown> | null",
        ts_return_type = "Record<string, unknown> | undefined"
    )]
    pub fn get(
        &self,
        env: Env,
        positional: Vec<JsUnknown>,
        named: Option<JsObject>,
    ) -> Result<JsUnknown> {
        guard(|| {
            self.require_reader()?;
            let values = self.bind_values(positional, named)?;
            let row = self.execute(&values, |_, statement| {
                let columns = column_names(statement);
                let width = columns.len();
                let row = match statement.raw_query().next()? {
                    Some(row) => Some(read_row(row, width)?),
                    None => None,
                };
                Ok(row.map(|row| (columns, row)))
            })?;
            match row {
                Some((columns, row)) => Ok(to_js_row(&env, &columns, row)?.into_unknown()),
                None => Ok(env.get_undefined()?.into_unknown()),
            }
        })
    }

    /// Every row. Internal: see `__run`.
    #[napi(
        js_name = "__all",
        ts_args_type = "positional: Array<unknown>, named?: Record<string, unknown> | null",
        ts_return_type = "Array<Record<string, unknown>>"
    )]
    pub fn all(
        &self,
        env: Env,
        positional: Vec<JsUnknown>,
        named: Option<JsObject>,
    ) -> Result<JsObject> {
        guard(|| {
            self.require_reader()?;
            let values = self.bind_values(positional, named)?;
            let rows = self.execute(&values, |_, statement| {
                let columns = column_names(statement);
                let width = columns.len();
                let mut rows = Vec::new();
                let mut cursor = statement.raw_query();
                while let Some(row) = cursor.next()? {
                    rows.push(read_row(row, width)?);
                }
                Ok(Rows { columns, rows })
            })?;
            rows.into_js_array(&env)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::sqlite_code_name;

    #[test]
    fn extended_codes_are_named_as_better_sqlite3_names_them() {
        assert_eq!(sqlite_code_name(19), "SQLITE_CONSTRAINT");
        assert_eq!(sqlite_code_name(19 | (8 << 8)), "SQLITE_CONSTRAINT_UNIQUE");
        assert_eq!(sqlite_code_name(19 | (6 << 8)), "SQLITE_CONSTRAINT_PRIMARYKEY");
        assert_eq!(sqlite_code_name(19 | (3 << 8)), "SQLITE_CONSTRAINT_FOREIGNKEY");
        assert_eq!(sqlite_code_name(5 | (3 << 8)), "SQLITE_BUSY_TIMEOUT");
        assert_eq!(sqlite_code_name(4 | (2 << 8)), "SQLITE_ABORT_ROLLBACK");
        assert_eq!(sqlite_code_name(10 | (34 << 8)), "SQLITE_IOERR_IN_PAGE");
        assert_eq!(sqlite_code_name(1), "SQLITE_ERROR");
        assert_eq!(sqlite_code_name(11), "SQLITE_CORRUPT");
        assert_eq!(sqlite_code_name(19 | (99 << 8)), "SQLITE_CONSTRAINT");
        assert_eq!(sqlite_code_name(77), "UNKNOWN_SQLITE_ERROR_77");
    }
}
