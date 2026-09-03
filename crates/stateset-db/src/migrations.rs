//! Database migrations
//!
//! Embedded SQL migrations that run automatically on database initialization.

use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during database migrations
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum MigrationError {
    #[error("Migration failed: {0}")]
    Failed(String),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

/// Run all migrations on the database
pub fn run_migrations(conn: &mut Connection) -> Result<(), MigrationError> {
    // Create migrations table if not exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            checksum TEXT,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;
    ensure_migration_checksum_column(conn)?;

    let migrations = get_migrations();

    for (name, sql) in migrations {
        if name == "027_vector_search" && !cfg!(feature = "vector") {
            continue;
        }
        if name == "028_bm25_search" && !fts5_available(conn) {
            continue;
        }
        let checksum = compute_migration_checksum(sql);
        let existing_checksum: Option<String> = conn
            .query_row("SELECT checksum FROM _migrations WHERE name = ?", [name], |row| row.get(0))
            .optional()?;

        if let Some(existing) = existing_checksum {
            if !existing.is_empty() && existing != checksum {
                return Err(MigrationError::Failed(format!(
                    "migration checksum mismatch for {name}: expected {checksum}, found {existing}",
                )));
            }
            if existing.is_empty() {
                conn.execute(
                    "UPDATE _migrations SET checksum = ? WHERE name = ?",
                    rusqlite::params![checksum, name],
                )?;
            }
        } else {
            // Run migration atomically.
            let tx = conn.transaction()?;
            let migrate_legacy_disputes =
                name == "073_a2a_dispute_kernel" && prepare_legacy_a2a_disputes(&tx)?;
            tx.execute_batch(sql)?;
            if migrate_legacy_disputes {
                migrate_legacy_a2a_disputes(&tx)?;
            }
            tx.execute(
                "INSERT INTO _migrations (name, checksum) VALUES (?, ?)",
                rusqlite::params![name, checksum],
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}

fn prepare_legacy_a2a_disputes(tx: &rusqlite::Transaction<'_>) -> Result<bool, MigrationError> {
    let exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'a2a_disputes')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(false);
    }
    let has_claimant = tx
        .prepare("PRAGMA table_info(a2a_disputes)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|column| column == "claimant_address");
    if has_claimant {
        return Ok(false);
    }
    let evidence_exists: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'a2a_dispute_evidence')",
        [],
        |row| row.get(0),
    )?;
    if evidence_exists {
        tx.execute("ALTER TABLE a2a_dispute_evidence RENAME TO a2a_dispute_evidence_legacy", [])?;
    } else {
        // Some early CLI databases created disputes lazily and may not have an
        // evidence table at all.  Give the copy step an empty, schema-compatible
        // source so migration 073 remains atomic for those databases too.
        tx.execute_batch(
            "CREATE TABLE a2a_dispute_evidence_legacy (
                id TEXT PRIMARY KEY,
                dispute_id TEXT NOT NULL,
                submitted_by TEXT NOT NULL,
                evidence_type TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                content TEXT,
                content_hash TEXT,
                created_at TEXT NOT NULL
             );",
        )?;
    }
    tx.execute("ALTER TABLE a2a_disputes RENAME TO a2a_disputes_legacy", [])?;
    Ok(true)
}

fn migrate_legacy_a2a_disputes(tx: &rusqlite::Transaction<'_>) -> Result<(), MigrationError> {
    tx.execute_batch(
        // The legacy CLI did not enforce the dispute -> escrow foreign key.
        // Preserve any orphaned dispute as a quarantined legacy escrow instead
        // of making the database permanently unmigratable.
        "INSERT OR IGNORE INTO a2a_escrows (
            id, status, quote_id, buyer_address, seller_address, amount,
            amount_decimal, asset, network, release_conditions, disputed_at,
            dispute_id, expires_at, metadata, created_at, updated_at,
            tenant_id, store_id
         )
         SELECT escrow_id,
                CASE WHEN status = 'resolved' THEN 'resolved' ELSE 'disputed' END,
                quote_id, filed_by, filed_against, amount_disputed,
                CAST(amount_decimal AS TEXT), asset, 'legacy', '[]',
                updated_at, id, COALESCE(review_deadline, updated_at, created_at),
                metadata, created_at, updated_at, 'legacy', 'legacy'
         FROM a2a_disputes_legacy;

         INSERT INTO a2a_disputes (
            id, tenant_id, store_id, status, escrow_id, quote_id,
            claimant_address, respondent_address, reason, category,
            amount_decimal, asset, resolution_type, buyer_amount_decimal,
            resolution_note, resolved_by, evidence_deadline, review_deadline,
            metadata, created_at, updated_at, resolved_at
         )
         SELECT id, 'legacy', 'legacy', status, escrow_id, quote_id,
                filed_by, filed_against, reason, category,
                CAST(amount_decimal AS TEXT), asset, resolution_type,
                CASE WHEN resolution_amount IS NULL THEN NULL
                     ELSE CAST(resolution_amount AS TEXT) END,
                resolution_note, resolved_by,
                COALESCE(evidence_deadline, created_at),
                COALESCE(review_deadline, updated_at, created_at),
                metadata, created_at, updated_at, resolved_at
         FROM a2a_disputes_legacy;

         INSERT INTO a2a_dispute_evidence (
            id, tenant_id, store_id, dispute_id, submitted_by, evidence_type,
            title, description, content, content_hash, created_at
         )
         SELECT id, 'legacy', 'legacy', dispute_id, submitted_by, evidence_type,
                title, description, COALESCE(content, ''), COALESCE(content_hash, ''), created_at
         FROM a2a_dispute_evidence_legacy;

         DROP TABLE a2a_dispute_evidence_legacy;
         DROP TABLE a2a_disputes_legacy;",
    )?;
    Ok(())
}

fn fts5_available(conn: &Connection) -> bool {
    conn.query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |row| {
        row.get::<_, i32>(0)
    })
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
        == 1
}

fn ensure_migration_checksum_column(conn: &Connection) -> Result<(), MigrationError> {
    let has_checksum = conn
        .prepare("PRAGMA table_info(_migrations)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|column| column == "checksum");
    if !has_checksum {
        conn.execute("ALTER TABLE _migrations ADD COLUMN checksum TEXT", [])?;
    }
    Ok(())
}

fn compute_migration_checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Names of every migration this binary knows about, in application order.
///
/// Used by backup/restore tooling to determine the schema version a binary
/// supports, so that restoring a backup taken by a *newer* binary can be
/// refused rather than silently corrupting data.
#[must_use]
pub fn known_migration_names() -> Vec<&'static str> {
    get_migrations().into_iter().map(|(name, _)| name).collect()
}

/// The highest (last) migration name known to this binary.
#[must_use]
pub fn latest_known_migration() -> &'static str {
    get_migrations().last().map_or("", |(name, _)| *name)
}

/// Get list of migrations in order
fn get_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("001_initial_schema", include_str!("../migrations/001_initial_schema.sql")),
        ("002_inventory", include_str!("../migrations/002_inventory.sql")),
        ("003_returns", include_str!("../migrations/003_returns.sql")),
        ("004_manufacturing", include_str!("../migrations/004_manufacturing.sql")),
        ("005_shipments", include_str!("../migrations/005_shipments.sql")),
        (
            "006_payments_warranties_po_invoices",
            include_str!("../migrations/006_payments_warranties_po_invoices.sql"),
        ),
        ("007_carts", include_str!("../migrations/007_carts.sql")),
        ("008_multi_currency", include_str!("../migrations/008_multi_currency.sql")),
        ("009_tax", include_str!("../migrations/009_tax.sql")),
        ("010_promotions", include_str!("../migrations/010_promotions.sql")),
        ("011_subscriptions", include_str!("../migrations/011_subscriptions.sql")),
        ("012_versioning", include_str!("../migrations/012_versioning.sql")),
        ("013_quality", include_str!("../migrations/013_quality.sql")),
        ("014_lots", include_str!("../migrations/014_lots.sql")),
        ("015_serials", include_str!("../migrations/015_serials.sql")),
        ("016_warehouse", include_str!("../migrations/016_warehouse.sql")),
        ("017_receiving", include_str!("../migrations/017_receiving.sql")),
        ("018_fulfillment", include_str!("../migrations/018_fulfillment.sql")),
        ("019_accounts_payable", include_str!("../migrations/019_accounts_payable.sql")),
        ("020_cost_accounting", include_str!("../migrations/020_cost_accounting.sql")),
        ("021_credit", include_str!("../migrations/021_credit.sql")),
        ("022_backorder", include_str!("../migrations/022_backorder.sql")),
        ("023_accounts_receivable", include_str!("../migrations/023_accounts_receivable.sql")),
        ("024_general_ledger", include_str!("../migrations/024_general_ledger.sql")),
        ("025_performance_indexes", include_str!("../migrations/025_performance_indexes.sql")),
        ("026_idempotency_keys", include_str!("../migrations/026_idempotency_keys.sql")),
        // Vector search migration (requires sqlite-vec extension to be loaded first)
        ("027_vector_search", include_str!("../migrations/027_vector_search.sql")),
        // Full-text search migration (requires FTS5)
        ("028_bm25_search", include_str!("../migrations/028_bm25_search.sql")),
        // x402 Payment Intents and Agent Cards for A2A Commerce
        ("029_x402_a2a", include_str!("../migrations/029_x402_a2a.sql")),
        // x402 Credit Ledger for metered billing
        ("030_x402_credits", include_str!("../migrations/030_x402_credits.sql")),
        // ERC-8004 Trustless Agents registries
        ("031_erc8004", include_str!("../migrations/031_erc8004.sql")),
        // Custom objects (custom states / metaobjects)
        ("032_custom_objects", include_str!("../migrations/032_custom_objects.sql")),
        // Orders <-> carts linkage for checkout idempotency (retry safety)
        ("033_orders_cart_id", include_str!("../migrations/033_orders_cart_id.sql")),
        // x402 nonce and idempotency integrity hardening
        ("034_x402_nonce_integrity", include_str!("../migrations/034_x402_nonce_integrity.sql")),
        // x402 PQC signature metadata
        ("035_x402_pqc", include_str!("../migrations/035_x402_pqc.sql")),
        // Fix updated_at triggers to write RFC3339 (was 'YYYY-MM-DD HH:MM:SS' which
        // failed to parse on subsequent reads)
        (
            "036_fix_updated_at_triggers",
            include_str!("../migrations/036_fix_updated_at_triggers.sql"),
        ),
        // Commerce entity tables that previously existed only in #[cfg(test)]
        // blocks and the PostgreSQL backend: shipping zones, gift cards, reviews,
        // segments, store credits, wishlists, loyalty. Their mounted REST
        // endpoints returned HTTP 500 'no such table' until provisioned here.
        ("037_commerce_entities", include_str!("../migrations/037_commerce_entities.sql")),
        (
            "038_fix_location_inventory_trigger",
            include_str!("../migrations/038_fix_location_inventory_trigger.sql"),
        ),
        // B2B / ERP-ops entities: channels, companies, transfer orders, units
        // of measure, production batches.
        ("039_b2b_erp_entities", include_str!("../migrations/039_b2b_erp_entities.sql")),
        // Supplier SKUs: per-supplier SKU / unit-cost overrides.
        ("040_supplier_skus", include_str!("../migrations/040_supplier_skus.sql")),
        // Vendor returns (return-to-supplier).
        ("041_vendor_returns", include_str!("../migrations/041_vendor_returns.sql")),
        // Vendor credits + applications.
        ("042_vendor_credits", include_str!("../migrations/042_vendor_credits.sql")),
        // Payment obligations (scheduled AP payments).
        ("043_payment_obligations", include_str!("../migrations/043_payment_obligations.sql")),
        // Price levels (B2B pricing tiers) + per-product entries.
        ("044_price_levels", include_str!("../migrations/044_price_levels.sql")),
        // Prepayments + applications.
        ("045_prepayments", include_str!("../migrations/045_prepayments.sql")),
        // Price schedules (time-bounded pricing) + entries.
        ("046_price_schedules", include_str!("../migrations/046_price_schedules.sql")),
        // Activity logs (append-only subject history).
        ("047_activity_logs", include_str!("../migrations/047_activity_logs.sql")),
        // Integration mappings (external↔internal value translation).
        ("048_integration_mappings", include_str!("../migrations/048_integration_mappings.sql")),
        // Inbound shipments (ASNs) + line items.
        ("049_inbound_shipments", include_str!("../migrations/049_inbound_shipments.sql")),
        // Purgatory (order ingestion staging) + line items.
        ("050_purgatory", include_str!("../migrations/050_purgatory.sql")),
        // Print stations + print job queue.
        ("051_print_stations", include_str!("../migrations/051_print_stations.sql")),
        // EDI documents + aggregate reporting.
        ("052_edi_documents", include_str!("../migrations/052_edi_documents.sql")),
        // Integration field mappings (field-path mappings).
        (
            "053_integration_field_mappings",
            include_str!("../migrations/053_integration_field_mappings.sql"),
        ),
        // Customer operational topology snapshots.
        ("054_topology_snapshots", include_str!("../migrations/054_topology_snapshots.sql")),
        // Stock snapshots (point-in-time inventory) + lines.
        ("055_stock_snapshots", include_str!("../migrations/055_stock_snapshots.sql")),
        ("056_rewards", include_str!("../migrations/056_rewards.sql")),
        ("057_loyalty_tiers", include_str!("../migrations/057_loyalty_tiers.sql")),
        ("058_wishlist_item_fields", include_str!("../migrations/058_wishlist_item_fields.sql")),
        ("059_fraud", include_str!("../migrations/059_fraud.sql")),
        // Fixed asset register + revenue recognition (ASC 606).
        ("060_fixed_assets_revrec", include_str!("../migrations/060_fixed_assets_revrec.sql")),
        // Auto-posting flags for depreciation and revenue recognition.
        ("061_gl_auto_posting_flags", include_str!("../migrations/061_gl_auto_posting_flags.sql")),
        // Cycle counts + lines.
        ("062_cycle_counts", include_str!("../migrations/062_cycle_counts.sql")),
        // FX gain/loss account for GL revaluation.
        ("063_gl_fx_revaluation", include_str!("../migrations/063_gl_fx_revaluation.sql")),
        // Durable HTTP idempotency response store.
        ("064_http_idempotency", include_str!("../migrations/064_http_idempotency.sql")),
        ("065_zone_shipping_methods", include_str!("../migrations/065_zone_shipping_methods.sql")),
        // Search configuration profiles (fields, facets, synonyms, boosts).
        ("066_search_configs", include_str!("../migrations/066_search_configs.sql")),
        // Per-line shipped quantities for partial shipments (+ backfill).
        (
            "067_order_item_shipped_quantity",
            include_str!("../migrations/067_order_item_shipped_quantity.sql"),
        ),
        // Warehouse bins + return item disposition.
        (
            "068_warehouse_bins_return_disposition",
            include_str!("../migrations/068_warehouse_bins_return_disposition.sql"),
        ),
        // Durable command receipts and transactional domain-event outbox.
        ("069_kernel_outbox", include_str!("../migrations/069_kernel_outbox.sql")),
        (
            "070_kernel_outbox_delivery",
            include_str!("../migrations/070_kernel_outbox_delivery.sql"),
        ),
        (
            "071_kernel_receipt_audit_chain",
            include_str!("../migrations/071_kernel_receipt_audit_chain.sql"),
        ),
        ("072_a2a_escrow_kernel", include_str!("../migrations/072_a2a_escrow_kernel.sql")),
        ("073_a2a_dispute_kernel", include_str!("../migrations/073_a2a_dispute_kernel.sql")),
        // Bookkeeping column so direct payments (record_payment) survive the
        // AR recalculation instead of being replaced by application sums.
        (
            "074_invoice_direct_amount_paid",
            include_str!("../migrations/074_invoice_direct_amount_paid.sql"),
        ),
        // Database-enforced auto-post idempotency: unique key per source
        // document for the single-entry journal families.
        (
            "075_gl_source_document_key",
            include_str!("../migrations/075_gl_source_document_key.sql"),
        ),
        // Idempotency ledger so a retried direct invoice payment (a caller
        // supplying `RecordInvoicePayment.payment_id`) applies exactly once.
        (
            "076_invoice_payment_idempotency",
            include_str!("../migrations/076_invoice_payment_idempotency.sql"),
        ),
        // Legacy-safe uniqueness for subscription billing cycles: a nullable
        // `cycle_key` column (subscription_id:cycle_number) + unique index, so a
        // duplicate cycle cannot be created and pre-guard duplicates stay NULL.
        (
            "077_billing_cycle_uniqueness",
            include_str!("../migrations/077_billing_cycle_uniqueness.sql"),
        ),
        // Order-level tax/shipping/discount so checkout can carry what the
        // customer is actually charged (see the migration for the failure).
        ("078_order_money_breakdown", include_str!("../migrations/078_order_money_breakdown.sql")),
        // Legacy-safe "one open reservation per serial": a nullable
        // `active_key` (= serial_id while open) + unique index, the DB backstop
        // behind the locked, status-conditional `reserve`.
        (
            "079_serial_reservation_uniqueness",
            include_str!("../migrations/079_serial_reservation_uniqueness.sql"),
        ),
        // Nullable `order_item_id` on inventory_reservations so a removed order
        // line releases ITS reservation, not the oldest one for the same SKU.
        (
            "080_reservation_order_line",
            include_str!("../migrations/080_reservation_order_line.sql"),
        ),
        // Legacy-safe uniqueness for x402 settlement tx hashes.
        // Lot/serial traceability columns on return_items.
        (
            "081_return_idempotency_and_traceability",
            include_str!("../migrations/081_return_idempotency_and_traceability.sql"),
        ),
        (
            "082_x402_tx_hash_uniqueness",
            include_str!("../migrations/082_x402_tx_hash_uniqueness.sql"),
        ),
        // Non-negative triggers on inventory balances (legacy-safe) and
        // `reservation_id` on backorder allocations.
        (
            "083_inventory_balance_guards",
            include_str!("../migrations/083_inventory_balance_guards.sql"),
        ),
        // Nullable billing-worker lease columns on subscriptions so due
        // subscriptions are claimed atomically before they are billed.
        ("084_billing_claim_lease", include_str!("../migrations/084_billing_claim_lease.sql")),
        // Legacy-safe, case-insensitive e-mail uniqueness for live customers
        // (keyed column; deleted accounts release their address).
        ("085_customer_email_key", include_str!("../migrations/085_customer_email_key.sql")),
        // Durable, tenant-scoped A2A credit terms and agent messaging
        // (previously process-local state in the HTTP routes).
        (
            "086_a2a_credit_terms_and_messaging",
            include_str!("../migrations/086_a2a_credit_terms_and_messaging.sql"),
        ),
        // Key the legacy case-duplicate customers 085 had to leave NULL, so
        // every live account is reachable and re-registration is defined.
        (
            "091_customer_email_key_backfill",
            include_str!("../migrations/091_customer_email_key_backfill.sql"),
        ),
        // The inventory balance identity (available == on_hand - allocated),
        // enforced by legacy-safe triggers. 083 only guaranteed
        // non-negativity.
        (
            "092_inventory_balance_identity",
            include_str!("../migrations/092_inventory_balance_identity.sql"),
        ),
        // Lot parent/child linkage, so a merged lot can be traced back to
        // every source lot (and its supplier or work order).
        ("093_lot_genealogy", include_str!("../migrations/093_lot_genealogy.sql")),
        // One claiming x402 intent per cart / per order, enforced by keyed
        // columns. The accessor's read-then-create check was a TOCTOU that
        // let two concurrent creates double-charge one cart.
        ("094_x402_cart_order_claim", include_str!("../migrations/094_x402_cart_order_claim.sql")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn install_legacy_dispute_schema(conn: &Connection, include_evidence: bool) {
        conn.execute_batch(
            "DROP INDEX idx_a2a_escrows_scope;
             ALTER TABLE a2a_escrows DROP COLUMN tenant_id;
             ALTER TABLE a2a_escrows DROP COLUMN store_id;
             DROP TABLE a2a_dispute_evidence;
             DROP TABLE a2a_disputes;
             DELETE FROM _migrations WHERE name = '073_a2a_dispute_kernel';
             CREATE TABLE a2a_disputes (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL DEFAULT 'filed',
                escrow_id TEXT NOT NULL,
                quote_id TEXT,
                filed_by TEXT NOT NULL,
                filed_against TEXT NOT NULL,
                reason TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'non_delivery',
                amount_disputed INTEGER NOT NULL,
                amount_decimal REAL NOT NULL,
                asset TEXT NOT NULL,
                resolution_type TEXT,
                resolution_amount INTEGER,
                resolution_note TEXT,
                resolved_by TEXT,
                evidence_deadline TEXT,
                review_deadline TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                resolved_at TEXT
             );
             INSERT INTO a2a_disputes (
                id, escrow_id, filed_by, filed_against, reason,
                amount_disputed, amount_decimal, asset, resolution_amount,
                created_at, updated_at
             ) VALUES (
                'dispute-legacy', 'escrow-legacy', 'buyer', 'seller', 'not delivered',
                1234, 12.34, 'USD', 7,
                '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
             );",
        )
        .expect("install legacy dispute schema");

        if include_evidence {
            conn.execute_batch(
                "CREATE TABLE a2a_dispute_evidence (
                    id TEXT PRIMARY KEY,
                    dispute_id TEXT NOT NULL,
                    submitted_by TEXT NOT NULL,
                    evidence_type TEXT NOT NULL,
                    title TEXT NOT NULL,
                    description TEXT,
                    content TEXT,
                    content_hash TEXT,
                    created_at TEXT NOT NULL
                 );
                 INSERT INTO a2a_dispute_evidence (
                    id, dispute_id, submitted_by, evidence_type, title,
                    description, content, content_hash, created_at
                 ) VALUES (
                    'evidence-legacy', 'dispute-legacy', 'buyer', 'document', 'Receipt',
                    'proof', 'payload', 'sha256:legacy', '2026-01-01T01:00:00Z'
                 );",
            )
            .expect("install legacy evidence schema");
        }
    }

    #[test]
    fn migrates_legacy_a2a_disputes_without_losing_exact_values_or_evidence() {
        let mut conn = Connection::open_in_memory().expect("open database");
        run_migrations(&mut conn).expect("initialize database");
        install_legacy_dispute_schema(&conn, true);

        run_migrations(&mut conn).expect("migrate legacy disputes");

        let dispute: (String, String, String, String, String, Option<String>) = conn
            .query_row(
                "SELECT tenant_id, store_id, claimant_address, respondent_address,
                        amount_decimal, buyer_amount_decimal
                 FROM a2a_disputes WHERE id = 'dispute-legacy'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read migrated dispute");
        assert_eq!(
            dispute,
            (
                "legacy".into(),
                "legacy".into(),
                "buyer".into(),
                "seller".into(),
                "12.34".into(),
                Some("7".into()),
            )
        );

        let evidence: (String, String, String, String) = conn
            .query_row(
                "SELECT tenant_id, store_id, content, content_hash
                 FROM a2a_dispute_evidence WHERE id = 'evidence-legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read migrated evidence");
        assert_eq!(
            evidence,
            ("legacy".into(), "legacy".into(), "payload".into(), "sha256:legacy".into())
        );
    }

    #[test]
    fn migrates_legacy_a2a_disputes_when_evidence_table_is_absent() {
        let mut conn = Connection::open_in_memory().expect("open database");
        run_migrations(&mut conn).expect("initialize database");
        install_legacy_dispute_schema(&conn, false);

        run_migrations(&mut conn).expect("migrate without legacy evidence table");

        let disputes: i64 = conn
            .query_row("SELECT COUNT(*) FROM a2a_disputes", [], |row| row.get(0))
            .expect("count disputes");
        let evidence: i64 = conn
            .query_row("SELECT COUNT(*) FROM a2a_dispute_evidence", [], |row| row.get(0))
            .expect("count evidence");
        assert_eq!(disputes, 1);
        assert_eq!(evidence, 0);
    }
}
