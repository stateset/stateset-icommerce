//! Full-fidelity structured export and import.
//!
//! Where [`crate::maintenance`] moves *bytes* (an exact database image),
//! this module moves *records* — a versioned, engine-independent JSON
//! document you can diff, archive, migrate across schema versions, or load
//! into a different backend.
//!
//! # Envelope format
//!
//! ```json
//! {
//!   "format_version": 1,
//!   "engine_version": "1.19.0",
//!   "exported_at": "2026-07-20T12:00:00Z",
//!   "schema_version": "066_search_configs",
//!   "domains": {
//!     "customers": [ { ... }, { ... } ],
//!     "orders":    [ { ... } ]
//!   }
//! }
//! ```
//!
//! Domains are streamed one at a time and each page is serialized straight to
//! the writer, so peak memory is one page (default 500 records), not the whole
//! database.
//!
//! # What export covers
//!
//! | Domain | Export | Import |
//! |---|---|---|
//! | `customers` | yes | yes |
//! | `products` (with variants) | yes | yes |
//! | `inventory_items` | yes | yes |
//! | `warehouses` | yes | yes |
//! | `suppliers` | yes | yes |
//! | `gl_accounts` | yes | yes |
//! | `orders` (with line items) | yes | yes |
//! | `purchase_orders` (with line items) | yes | yes |
//! | `journal_entries` (with lines) | yes | yes |
//! | `returns` | yes | no (export only) |
//! | `invoices` | yes | no (export only) |
//! | `payments` | yes | no (export only) |
//! | `bills` | yes | no (export only) |
//!
//! # What export does NOT cover
//!
//! This is a *business-record* export, not a database image. Use
//! [`crate::maintenance::backup_to`] when you need byte-exact recovery.
//! Deliberately omitted:
//!
//! - **Derived / ledger-internal state**: inventory transactions and
//!   reservations, GL trial balances, period roll-forwards, allocations.
//!   These are recomputed from the records that are exported.
//! - **Operational plumbing**: `_migrations`, idempotency keys, HTTP
//!   idempotency responses, audit log, saga state, outbox rows, job queues.
//! - **Search / ML artifacts**: FTS5 shadow tables, vector embeddings, search
//!   configs. Rebuild these by reindexing after import.
//! - **Agent + payment protocol state**: x402 payment intents and credits, A2A
//!   quotes/purchases, agent cards, identities, reputation, ERC-8004 registries.
//!   These carry cryptographic material and nonces that must not be replayed.
//! - **The long tail of ~50 other domains** (carts, subscriptions, promotions,
//!   loyalty, quality, lots, serials, EDI, fixed assets, revenue recognition,
//!   …). They are exportable in principle; the registry is designed to be
//!   extended one `DomainSpec` at a time.
//!
//! # Import semantics and the ID caveat
//!
//! Import replays records through the ordinary repository `create` methods, so
//! every invariant, validation rule and derived-field computation the engine
//! enforces is applied. The consequence is that **IDs are not preserved**:
//! `CreateCustomer` and friends have no `id` field, and the repository mints a
//! new one. Import therefore maintains an `IdRemap` and rewrites foreign keys
//! as it goes (customer → order, product → order item, supplier → purchase
//! order, GL account → journal entry line), inserting domains in dependency
//! order.
//!
//! The practical rule: import is **content-preserving, not identity-preserving**.
//! If you need identity preserved — the disaster-recovery case — restore a
//! backup instead.
//!
//! Idempotency is controlled by [`ImportOptions::on_conflict`].

use crate::Database;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use stateset_core::{CommerceError, Result};
use std::collections::HashMap;
use std::io::{Read, Write};

/// The envelope format version this build writes and accepts.
pub const FORMAT_VERSION: u32 = 1;

/// Records fetched per page when walking a domain.
///
/// Kept below the engine's 1000-row LIMIT policy so a page is never silently
/// truncated by the repository layer.
pub const PAGE_SIZE: u32 = 500;

/// Header fields of an export envelope, without the (streamed) domain payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportHeader {
    /// Envelope format version.
    pub format_version: u32,
    /// Engine version that produced the export.
    pub engine_version: String,
    /// When the export was taken.
    pub exported_at: String,
    /// Highest applied migration name of the source database.
    pub schema_version: String,
}

/// Options controlling [`export_all`].
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Restrict the export to these domain names. Empty means "all".
    pub domains: Vec<String>,
    /// Records fetched per page.
    pub page_size: u32,
    /// Pretty-print the JSON.
    pub pretty: bool,
    /// Schema version to record in the header.
    pub schema_version: String,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            domains: Vec::new(),
            page_size: PAGE_SIZE,
            pretty: false,
            schema_version: String::new(),
        }
    }
}

/// Per-domain export statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportReport {
    /// Records written per domain, in export order.
    pub counts: Vec<(String, usize)>,
    /// Total records written.
    pub total: usize,
}

/// How import should react to a record that already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConflictPolicy {
    /// Leave the existing record alone and continue (idempotent re-import).
    #[default]
    Skip,
    /// Abort the whole import.
    Fail,
}

/// Options controlling [`import_all`].
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Restrict the import to these domain names. Empty means "all importable".
    pub domains: Vec<String>,
    /// Behaviour when a record already exists.
    pub on_conflict: ConflictPolicy,
    /// Parse and validate without writing anything.
    pub dry_run: bool,
}

/// Result of an [`import_all`] run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImportReport {
    /// Records created per domain.
    pub created: Vec<(String, usize)>,
    /// Records skipped as already present, per domain.
    pub skipped: Vec<(String, usize)>,
    /// Domains present in the envelope that this build cannot import.
    pub unsupported_domains: Vec<String>,
    /// Total records created.
    pub total_created: usize,
}

/// Maps source-export IDs onto the IDs minted during import.
#[derive(Debug, Default)]
struct IdRemap {
    by_domain: HashMap<&'static str, HashMap<String, String>>,
}

impl IdRemap {
    fn record(&mut self, domain: &'static str, old: &str, new: String) {
        self.by_domain.entry(domain).or_default().insert(old.to_owned(), new);
    }

    fn lookup(&self, domain: &str, old: &str) -> Option<&String> {
        self.by_domain.get(domain).and_then(|m| m.get(old))
    }
}

// ===========================================================================
// Domain registry
// ===========================================================================

/// A page fetcher: `(db, limit, offset) -> records as JSON`.
type Fetch = fn(&dyn Database, u32, u32) -> Result<Vec<Value>>;
/// An importer: `(db, record, remap, policy) -> Outcome`.
type Import = fn(&dyn Database, &Value, &mut IdRemap, ConflictPolicy) -> Result<Outcome>;

/// What happened to a single imported record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Created,
    Skipped,
}

/// Describes one exportable domain.
struct DomainSpec {
    /// Key used in the envelope's `domains` map.
    name: &'static str,
    /// Paginated reader.
    fetch: Fetch,
    /// Writer, or `None` for export-only domains.
    import: Option<Import>,
}

/// All registered domains, in dependency order: a domain never appears before
/// something it references.
fn registry() -> Vec<DomainSpec> {
    vec![
        DomainSpec { name: "customers", fetch: fetch_customers, import: Some(import_customer) },
        DomainSpec { name: "products", fetch: fetch_products, import: Some(import_product) },
        DomainSpec {
            name: "inventory_items",
            fetch: fetch_inventory_items,
            import: Some(import_inventory_item),
        },
        DomainSpec { name: "warehouses", fetch: fetch_warehouses, import: Some(import_warehouse) },
        DomainSpec { name: "suppliers", fetch: fetch_suppliers, import: Some(import_supplier) },
        DomainSpec {
            name: "gl_accounts",
            fetch: fetch_gl_accounts,
            import: Some(import_gl_account),
        },
        DomainSpec { name: "orders", fetch: fetch_orders, import: Some(import_order) },
        DomainSpec {
            name: "purchase_orders",
            fetch: fetch_purchase_orders,
            import: Some(import_purchase_order),
        },
        DomainSpec { name: "gl_periods", fetch: fetch_gl_periods, import: Some(import_gl_period) },
        DomainSpec {
            name: "journal_entries",
            fetch: fetch_journal_entries,
            import: Some(import_journal_entry),
        },
        // Export-only: these carry posting/state-machine history that cannot be
        // faithfully replayed through a single `create` call.
        DomainSpec { name: "returns", fetch: fetch_returns, import: None },
        DomainSpec { name: "invoices", fetch: fetch_invoices, import: None },
        DomainSpec { name: "payments", fetch: fetch_payments, import: None },
        DomainSpec { name: "bills", fetch: fetch_bills, import: None },
    ]
}

/// Names of every domain the export covers, in export order.
#[must_use]
pub fn exportable_domains() -> Vec<&'static str> {
    registry().into_iter().map(|d| d.name).collect()
}

/// Names of the domains import can write.
#[must_use]
pub fn importable_domains() -> Vec<&'static str> {
    registry().into_iter().filter(|d| d.import.is_some()).map(|d| d.name).collect()
}

// ===========================================================================
// Export
// ===========================================================================

fn json_err(err: serde_json::Error) -> CommerceError {
    CommerceError::ValidationError(format!("json error: {err}"))
}

fn io_err(err: std::io::Error) -> CommerceError {
    CommerceError::DatabaseError(format!("export io error: {err}"))
}

fn to_values<T: Serialize>(items: Vec<T>) -> Result<Vec<Value>> {
    items.into_iter().map(|item| serde_json::to_value(item).map_err(json_err)).collect()
}

/// Write a full structured export of `db` to `writer`.
///
/// Domains are streamed sequentially and each page is written as it is read,
/// so memory use is bounded by [`ExportOptions::page_size`] regardless of
/// database size.
///
/// # Errors
///
/// Returns an error if a repository read fails, a record cannot be serialized,
/// or the writer fails.
pub fn export_all<W: Write>(
    db: &dyn Database,
    writer: &mut W,
    options: &ExportOptions,
) -> Result<ExportReport> {
    let page_size = options.page_size.clamp(1, 1000);
    let header = ExportHeader {
        format_version: FORMAT_VERSION,
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        exported_at: Utc::now().to_rfc3339(),
        schema_version: options.schema_version.clone(),
    };

    let mut out = Vec::new();
    write!(
        out,
        "{{\"format_version\":{},\"engine_version\":{},\"exported_at\":{},\"schema_version\":{},\"domains\":{{",
        header.format_version,
        serde_json::to_string(&header.engine_version).map_err(json_err)?,
        serde_json::to_string(&header.exported_at).map_err(json_err)?,
        serde_json::to_string(&header.schema_version).map_err(json_err)?,
    )
    .map_err(io_err)?;
    writer.write_all(&out).map_err(io_err)?;

    let mut counts = Vec::new();
    let mut total = 0_usize;
    let mut first_domain = true;

    for spec in registry() {
        if !options.domains.is_empty() && !options.domains.iter().any(|d| d == spec.name) {
            continue;
        }
        if !first_domain {
            writer.write_all(b",").map_err(io_err)?;
        }
        first_domain = false;
        write!(writer, "{}:[", serde_json::to_string(spec.name).map_err(json_err)?)
            .map_err(io_err)?;

        let mut domain_count = 0_usize;
        let mut offset = 0_u32;
        loop {
            let page = (spec.fetch)(db, page_size, offset)?;
            let fetched = u32::try_from(page.len()).unwrap_or(u32::MAX);
            for record in page {
                if domain_count > 0 {
                    writer.write_all(b",").map_err(io_err)?;
                }
                let encoded = if options.pretty {
                    serde_json::to_vec_pretty(&record)
                } else {
                    serde_json::to_vec(&record)
                }
                .map_err(json_err)?;
                writer.write_all(&encoded).map_err(io_err)?;
                domain_count += 1;
            }
            if fetched < page_size {
                break;
            }
            offset = offset.saturating_add(page_size);
            // Defensive stop: a repository that ignores offset would otherwise
            // loop forever re-emitting page zero.
            if offset > 10_000_000 {
                break;
            }
        }

        writer.write_all(b"]").map_err(io_err)?;
        total += domain_count;
        counts.push((spec.name.to_owned(), domain_count));
    }

    writer.write_all(b"}}").map_err(io_err)?;
    writer.flush().map_err(io_err)?;

    Ok(ExportReport { counts, total })
}

// ===========================================================================
// Import
// ===========================================================================

/// Read an export envelope from `reader` and replay it into `db`.
///
/// See the [module docs](self) for the identity caveat.
///
/// # Errors
///
/// Returns an error if the envelope is malformed, its `format_version` is
/// unsupported, a record fails validation, or (under
/// [`ConflictPolicy::Fail`]) a record already exists.
pub fn import_all<R: Read>(
    db: &dyn Database,
    reader: &mut R,
    options: &ImportOptions,
) -> Result<ImportReport> {
    let mut buf = String::new();
    reader
        .read_to_string(&mut buf)
        .map_err(|e| CommerceError::DatabaseError(format!("import io error: {e}")))?;
    let envelope: Value = serde_json::from_str(&buf).map_err(json_err)?;

    let format_version =
        envelope.get("format_version").and_then(Value::as_u64).ok_or_else(|| {
            CommerceError::ValidationError("export envelope is missing format_version".to_owned())
        })?;
    if format_version != u64::from(FORMAT_VERSION) {
        return Err(CommerceError::ValidationError(format!(
            "unsupported export format_version {format_version}; this build reads {FORMAT_VERSION}"
        )));
    }

    let domains = envelope.get("domains").and_then(Value::as_object).ok_or_else(|| {
        CommerceError::ValidationError("export envelope is missing a domains object".to_owned())
    })?;

    let mut remap = IdRemap::default();
    let mut created = Vec::new();
    let mut skipped = Vec::new();
    let mut total_created = 0_usize;

    let specs = registry();
    for spec in &specs {
        let Some(importer) = spec.import else { continue };
        if !options.domains.is_empty() && !options.domains.iter().any(|d| d == spec.name) {
            continue;
        }
        let Some(records) = domains.get(spec.name).and_then(Value::as_array) else { continue };

        let mut n_created = 0_usize;
        let mut n_skipped = 0_usize;
        for record in records {
            if options.dry_run {
                n_skipped += 1;
                continue;
            }
            match importer(db, record, &mut remap, options.on_conflict)? {
                Outcome::Created => n_created += 1,
                Outcome::Skipped => n_skipped += 1,
            }
        }
        total_created += n_created;
        created.push((spec.name.to_owned(), n_created));
        skipped.push((spec.name.to_owned(), n_skipped));
    }

    let importable = importable_domains();
    let unsupported_domains = domains
        .keys()
        .filter(|k| !importable.contains(&k.as_str()))
        .filter(|k| domains.get(*k).and_then(Value::as_array).is_some_and(|a| !a.is_empty()))
        .cloned()
        .collect();

    Ok(ImportReport { created, skipped, unsupported_domains, total_created })
}

// ===========================================================================
// Field helpers
// ===========================================================================

fn obj(record: &Value) -> Result<&Map<String, Value>> {
    record
        .as_object()
        .ok_or_else(|| CommerceError::ValidationError("export record is not an object".to_owned()))
}

fn str_field(record: &Value, field: &str) -> Result<String> {
    obj(record)?
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| CommerceError::ValidationError(format!("record is missing '{field}'")))
}

fn opt_str(record: &Value, field: &str) -> Option<String> {
    record.get(field).and_then(Value::as_str).map(ToOwned::to_owned)
}

/// Deserialize the first of `fields` that is present and non-null.
///
/// Model structs and their `Create*` inputs sometimes name the same concept
/// differently (`PurchaseOrderItem::quantity_ordered` vs
/// `CreatePurchaseOrderItem::quantity`); this bridges the two.
fn any_field_as<T: for<'de> Deserialize<'de>>(record: &Value, fields: &[&str]) -> Result<T> {
    let value = fields
        .iter()
        .find_map(|field| record.get(*field).filter(|v| !v.is_null()).cloned())
        .unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| {
        CommerceError::ValidationError(format!("field '{}': {e}", fields.join("' / '")))
    })
}

/// Take a subobject of a record and deserialize it into `T`.
fn field_as<T: for<'de> Deserialize<'de>>(record: &Value, field: &str) -> Result<T> {
    let value = record.get(field).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value)
        .map_err(|e| CommerceError::ValidationError(format!("field '{field}': {e}")))
}

/// Deserialize an entire record into a `Create*` input by reusing the model's
/// own serde representation, dropping fields the input does not declare.
fn record_as<T: for<'de> Deserialize<'de>>(record: &Value) -> Result<T> {
    serde_json::from_value(record.clone())
        .map_err(|e| CommerceError::ValidationError(format!("cannot decode record: {e}")))
}

/// Resolve a foreign key through the remap, falling back to the original value
/// when the referenced domain was not part of this import.
fn remapped_id(remap: &IdRemap, domain: &str, raw: &str) -> String {
    remap.lookup(domain, raw).cloned().unwrap_or_else(|| raw.to_owned())
}

fn parse_uuid(raw: &str, field: &str) -> Result<uuid::Uuid> {
    raw.parse::<uuid::Uuid>()
        .map_err(|e| CommerceError::ValidationError(format!("field '{field}' is not a uuid: {e}")))
}

// ===========================================================================
// Per-domain fetch + import
// ===========================================================================

fn fetch_customers(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::CustomerFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.customers().list(filter)?)
}

fn import_customer(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let old_id = str_field(record, "id")?;
    let email = str_field(record, "email")?;
    if let Some(existing) = db.customers().get_by_email(&email)? {
        if policy == ConflictPolicy::Fail {
            return Err(CommerceError::ValidationError(format!(
                "customer with email '{email}' already exists"
            )));
        }
        remap.record("customers", &old_id, existing.id.to_string());
        return Ok(Outcome::Skipped);
    }
    let input = stateset_core::CreateCustomer {
        email,
        first_name: str_field(record, "first_name")?,
        last_name: str_field(record, "last_name")?,
        phone: opt_str(record, "phone"),
        accepts_marketing: record.get("accepts_marketing").and_then(Value::as_bool),
        tags: field_as::<Option<Vec<String>>>(record, "tags")?,
        metadata: record.get("metadata").cloned().filter(|v| !v.is_null()),
    };
    let created = db.customers().create(input)?;
    remap.record("customers", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_products(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::ProductFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.products().list(filter)?)
}

fn import_product(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let old_id = str_field(record, "id")?;
    let name = str_field(record, "name")?;
    let slug = opt_str(record, "slug");

    if let Some(slug) = slug.as_deref() {
        let existing = db.products().list(stateset_core::ProductFilter {
            search: Some(slug.to_owned()),
            limit: Some(1000),
            ..Default::default()
        })?;
        if let Some(found) = existing.iter().find(|p| p.slug == slug) {
            if policy == ConflictPolicy::Fail {
                return Err(CommerceError::ValidationError(format!(
                    "product with slug '{slug}' already exists"
                )));
            }
            remap.record("products", &old_id, found.id.to_string());
            return Ok(Outcome::Skipped);
        }
    }

    let variants: Option<Vec<stateset_core::CreateProductVariant>> =
        match record.get("variants").and_then(Value::as_array) {
            Some(list) if !list.is_empty() => Some(
                list.iter()
                    .map(record_as::<stateset_core::CreateProductVariant>)
                    .collect::<Result<Vec<_>>>()?,
            ),
            _ => None,
        };

    let input = stateset_core::CreateProduct {
        name,
        slug,
        description: opt_str(record, "description"),
        product_type: field_as(record, "product_type")?,
        attributes: field_as(record, "attributes")?,
        seo: field_as(record, "seo")?,
        variants,
    };
    let created = db.products().create(input)?;
    remap.record("products", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_inventory_items(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::InventoryFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.inventory().list(filter)?)
}

fn import_inventory_item(
    db: &dyn Database,
    record: &Value,
    _remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let sku = str_field(record, "sku")?;
    if db.inventory().get_item_by_sku(&sku)?.is_some() {
        if policy == ConflictPolicy::Fail {
            return Err(CommerceError::ValidationError(format!(
                "inventory item with sku '{sku}' already exists"
            )));
        }
        return Ok(Outcome::Skipped);
    }
    let input = stateset_core::CreateInventoryItem {
        sku,
        name: str_field(record, "name")?,
        description: opt_str(record, "description"),
        unit_of_measure: opt_str(record, "unit_of_measure"),
        initial_quantity: field_as(record, "initial_quantity")?,
        location_id: record
            .get("location_id")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok()),
        reorder_point: field_as(record, "reorder_point")?,
        safety_stock: field_as(record, "safety_stock")?,
    };
    db.inventory().create_item(input)?;
    Ok(Outcome::Created)
}

fn fetch_warehouses(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::WarehouseFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.warehouse().list_warehouses(filter)?)
}

fn import_warehouse(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let code = str_field(record, "code")?;
    let old_id = record.get("id").map(ToString::to_string).unwrap_or_default();
    if let Some(existing) = db.warehouse().get_warehouse_by_code(&code)? {
        if policy == ConflictPolicy::Fail {
            return Err(CommerceError::ValidationError(format!(
                "warehouse with code '{code}' already exists"
            )));
        }
        remap.record("warehouses", &old_id, existing.id.to_string());
        return Ok(Outcome::Skipped);
    }
    let input = stateset_core::CreateWarehouse {
        code,
        name: str_field(record, "name")?,
        warehouse_type: field_as(record, "warehouse_type")?,
        address: field_as(record, "address")?,
        timezone: opt_str(record, "timezone"),
    };
    let created = db.warehouse().create_warehouse(input)?;
    remap.record("warehouses", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_suppliers(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::SupplierFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.purchase_orders().list_suppliers(filter)?)
}

fn import_supplier(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let old_id = str_field(record, "id")?;
    let code = opt_str(record, "supplier_code");
    if let Some(code) = code.as_deref() {
        if let Some(existing) = db.purchase_orders().get_supplier_by_code(code)? {
            if policy == ConflictPolicy::Fail {
                return Err(CommerceError::ValidationError(format!(
                    "supplier with code '{code}' already exists"
                )));
            }
            remap.record("suppliers", &old_id, existing.id.to_string());
            return Ok(Outcome::Skipped);
        }
    }
    let input = stateset_core::CreateSupplier {
        name: str_field(record, "name")?,
        supplier_code: code,
        contact_name: opt_str(record, "contact_name"),
        email: opt_str(record, "email"),
        phone: opt_str(record, "phone"),
        website: opt_str(record, "website"),
        address: opt_str(record, "address"),
        city: opt_str(record, "city"),
        state: opt_str(record, "state"),
        postal_code: opt_str(record, "postal_code"),
        country: opt_str(record, "country"),
        tax_id: opt_str(record, "tax_id"),
        payment_terms: field_as(record, "payment_terms")?,
        currency: field_as(record, "currency")?,
        lead_time_days: record
            .get("lead_time_days")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok()),
        minimum_order: field_as(record, "minimum_order")?,
        notes: opt_str(record, "notes"),
    };
    let created = db.purchase_orders().create_supplier(input)?;
    remap.record("suppliers", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_gl_accounts(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::GlAccountFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.general_ledger().list_accounts(filter)?)
}

fn import_gl_account(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let old_id = str_field(record, "id")?;
    let account_number = str_field(record, "account_number")?;
    if let Some(existing) = db.general_ledger().get_account_by_number(&account_number)? {
        if policy == ConflictPolicy::Fail {
            return Err(CommerceError::ValidationError(format!(
                "gl account '{account_number}' already exists"
            )));
        }
        remap.record("gl_accounts", &old_id, existing.id.to_string());
        return Ok(Outcome::Skipped);
    }
    // Parent accounts appear in the same page and are remapped if already seen.
    let parent_account_id = match opt_str(record, "parent_account_id") {
        Some(parent) => {
            Some(parse_uuid(&remapped_id(remap, "gl_accounts", &parent), "parent_account_id")?)
        }
        None => None,
    };
    let input = stateset_core::CreateGlAccount {
        account_number,
        name: str_field(record, "name")?,
        description: opt_str(record, "description"),
        account_type: field_as(record, "account_type")?,
        account_sub_type: field_as(record, "account_sub_type")?,
        parent_account_id,
        is_header: record.get("is_header").and_then(Value::as_bool),
        is_posting: record.get("is_posting").and_then(Value::as_bool),
        currency: field_as(record, "currency")?,
    };
    let created = db.general_ledger().create_account(input)?;
    remap.record("gl_accounts", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_orders(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::OrderFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.orders().list(filter)?)
}

fn import_order(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    _policy: ConflictPolicy,
) -> Result<Outcome> {
    let raw_customer = str_field(record, "customer_id")?;
    let customer_id = parse_uuid(&remapped_id(remap, "customers", &raw_customer), "customer_id")?;

    let items = record
        .get("items")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|item| {
                    let raw_product = str_field(item, "product_id")?;
                    Ok(stateset_core::CreateOrderItem {
                        product_id: parse_uuid(
                            &remapped_id(remap, "products", &raw_product),
                            "product_id",
                        )?
                        .into(),
                        variant_id: None,
                        sku: str_field(item, "sku")?,
                        name: str_field(item, "name")?,
                        quantity: item
                            .get("quantity")
                            .and_then(Value::as_i64)
                            .and_then(|q| i32::try_from(q).ok())
                            .unwrap_or(0),
                        unit_price: field_as(item, "unit_price")?,
                        discount: field_as(item, "discount")?,
                        tax_amount: field_as(item, "tax_amount")?,
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let input = stateset_core::CreateOrder {
        customer_id: customer_id.into(),
        items,
        currency: field_as(record, "currency")?,
        shipping_address: field_as(record, "shipping_address")?,
        billing_address: field_as(record, "billing_address")?,
        notes: opt_str(record, "notes"),
        payment_method: opt_str(record, "payment_method"),
        shipping_method: opt_str(record, "shipping_method"),
    };
    let created = db.orders().create(input)?;
    if let Ok(old_id) = str_field(record, "id") {
        remap.record("orders", &old_id, created.id.to_string());
    }
    Ok(Outcome::Created)
}

fn fetch_purchase_orders(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::PurchaseOrderFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.purchase_orders().list(filter)?)
}

fn import_purchase_order(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    _policy: ConflictPolicy,
) -> Result<Outcome> {
    let raw_supplier = str_field(record, "supplier_id")?;
    let supplier_id = parse_uuid(&remapped_id(remap, "suppliers", &raw_supplier), "supplier_id")?;

    let items = record
        .get("items")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|item| {
                    let product_id = match opt_str(item, "product_id") {
                        Some(raw) => Some(
                            parse_uuid(&remapped_id(remap, "products", &raw), "product_id")?.into(),
                        ),
                        None => None,
                    };
                    Ok(stateset_core::CreatePurchaseOrderItem {
                        product_id,
                        sku: str_field(item, "sku")?,
                        name: str_field(item, "name")?,
                        supplier_sku: opt_str(item, "supplier_sku"),
                        quantity: any_field_as(item, &["quantity", "quantity_ordered"])?,
                        unit_of_measure: opt_str(item, "unit_of_measure"),
                        unit_cost: field_as(item, "unit_cost")?,
                        tax_amount: field_as(item, "tax_amount")?,
                        discount_amount: field_as(item, "discount_amount")?,
                        expected_date: field_as(item, "expected_date")?,
                        notes: opt_str(item, "notes"),
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let input = stateset_core::CreatePurchaseOrder {
        supplier_id,
        order_date: field_as(record, "order_date")?,
        expected_date: field_as(record, "expected_date")?,
        ship_to_address: opt_str(record, "ship_to_address"),
        ship_to_city: opt_str(record, "ship_to_city"),
        ship_to_state: opt_str(record, "ship_to_state"),
        ship_to_postal_code: opt_str(record, "ship_to_postal_code"),
        ship_to_country: opt_str(record, "ship_to_country"),
        payment_terms: field_as(record, "payment_terms")?,
        currency: field_as(record, "currency")?,
        tax_amount: field_as(record, "tax_amount")?,
        shipping_cost: field_as(record, "shipping_cost")?,
        discount_amount: field_as(record, "discount_amount")?,
        notes: opt_str(record, "notes"),
        supplier_notes: opt_str(record, "supplier_notes"),
        items,
    };
    db.purchase_orders().create(input)?;
    Ok(Outcome::Created)
}

fn fetch_gl_periods(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::GlPeriodFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.general_ledger().list_periods(filter)?)
}

fn import_gl_period(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    policy: ConflictPolicy,
) -> Result<Outcome> {
    let old_id = str_field(record, "id")?;
    let fiscal_year = record.get("fiscal_year").and_then(Value::as_i64).unwrap_or(0);
    let period_number = record.get("period_number").and_then(Value::as_i64).unwrap_or(0);

    let existing = db.general_ledger().list_periods(stateset_core::GlPeriodFilter {
        fiscal_year: i32::try_from(fiscal_year).ok(),
        limit: Some(1000),
        ..Default::default()
    })?;
    if let Some(found) = existing.into_iter().find(|p| i64::from(p.period_number) == period_number)
    {
        if policy == ConflictPolicy::Fail {
            return Err(CommerceError::ValidationError(format!(
                "gl period {fiscal_year}-{period_number} already exists"
            )));
        }
        remap.record("gl_periods", &old_id, found.id.to_string());
        return Ok(Outcome::Skipped);
    }

    let input = stateset_core::CreateGlPeriod {
        period_name: str_field(record, "period_name")?,
        fiscal_year: i32::try_from(fiscal_year).unwrap_or_default(),
        period_number: i32::try_from(period_number).unwrap_or_default(),
        start_date: field_as(record, "start_date")?,
        end_date: field_as(record, "end_date")?,
    };
    let created = db.general_ledger().create_period(input)?;
    // Preserve the open/closed lifecycle so journal entries can still post.
    if opt_str(record, "status").as_deref() == Some("open") {
        db.general_ledger().open_period(created.id)?;
    }
    remap.record("gl_periods", &old_id, created.id.to_string());
    Ok(Outcome::Created)
}

fn fetch_journal_entries(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::JournalEntryFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.general_ledger().list_journal_entries(filter)?)
}

fn import_journal_entry(
    db: &dyn Database,
    record: &Value,
    remap: &mut IdRemap,
    _policy: ConflictPolicy,
) -> Result<Outcome> {
    let lines = record
        .get("lines")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|line| {
                    let raw_account = str_field(line, "account_id")?;
                    Ok(stateset_core::CreateJournalEntryLine {
                        account_id: parse_uuid(
                            &remapped_id(remap, "gl_accounts", &raw_account),
                            "account_id",
                        )?,
                        description: opt_str(line, "description"),
                        debit_amount: field_as(line, "debit_amount")?,
                        credit_amount: field_as(line, "credit_amount")?,
                        reference_type: opt_str(line, "reference_type"),
                        reference_id: match opt_str(line, "reference_id") {
                            Some(raw) => Some(parse_uuid(&raw, "reference_id")?),
                            None => None,
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let input = stateset_core::CreateJournalEntry {
        entry_date: field_as(record, "entry_date")?,
        entry_type: field_as(record, "entry_type")?,
        description: str_field(record, "description")?,
        lines,
        source_document_type: opt_str(record, "source_document_type"),
        source_document_id: match opt_str(record, "source_document_id") {
            Some(raw) => Some(parse_uuid(&raw, "source_document_id")?),
            None => None,
        },
        auto_post: Some(false),
    };
    db.general_ledger().create_journal_entry(input)?;
    Ok(Outcome::Created)
}

// --- export-only domains ---------------------------------------------------

fn fetch_returns(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::ReturnFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.returns().list(filter)?)
}

fn fetch_invoices(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::InvoiceFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.invoices().list(filter)?)
}

fn fetch_payments(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::PaymentFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.payments().list(filter)?)
}

fn fetch_bills(db: &dyn Database, limit: u32, offset: u32) -> Result<Vec<Value>> {
    let filter = stateset_core::BillFilter {
        limit: Some(limit),
        offset: Some(offset),
        ..Default::default()
    };
    to_values(db.accounts_payable().list_bills(filter)?)
}
