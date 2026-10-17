//! Operator receipt inspection and audited completion. See the operations runbook.
#![cfg(feature = "sqlite")]

use std::error::Error;
use std::io::Read;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};
use stateset_db::reconciliation::{
    HttpReceiptResolution, pending_http_receipts, reconcile_http_receipt,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: http_receipts DB list TENANT | resolve JSON [--apply --worker-stopped]")?;
    let operation = args.next().ok_or("expected list or resolve")?;
    match operation.as_str() {
        "list" => {
            let tenant =
                args.next().ok_or("supply an explicit tenant (empty string for single tenant)")?;
            if args.next().is_some() {
                return Err("unexpected list argument".into());
            }
            let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            let pending = pending_http_receipts(&conn, &tenant, Utc::now(), 1000)?;
            println!("{}", serde_json::to_string_pretty(&pending)?);
            if pending.len() == 1000 {
                eprintln!(
                    "Result capped at 1000; inspect remaining receipts after reconciliation."
                );
            }
        }
        "resolve" => {
            let file = args.next().ok_or("expected resolution JSON file")?;
            let mut apply = false;
            let mut worker_stopped = false;
            for arg in args {
                match arg.as_str() {
                    "--apply" if !apply => apply = true,
                    "--worker-stopped" if !worker_stopped => worker_stopped = true,
                    _ => return Err("unexpected or repeated resolve argument".into()),
                }
            }
            // JSON byte arrays can be several times larger than their decoded payload.
            let mut json = String::new();
            std::fs::File::open(file)?.take(8 * 1024 * 1024 + 1).read_to_string(&mut json)?;
            if json.len() > 8 * 1024 * 1024 {
                return Err("resolution file exceeds 8 MiB".into());
            }
            let resolution: HttpReceiptResolution = serde_json::from_str(&json)?;
            resolution.validate()?;
            if !apply {
                let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let count: u64 = conn.query_row(
                    "SELECT COUNT(*) FROM http_idempotency_keys WHERE tenant = ?1
                     AND idempotency_key = ?2 AND response_status = 0",
                    (&resolution.tenant, &resolution.idempotency_key),
                    |row| row.get(0),
                )?;
                println!(
                    "Preview: {} pending receipt(s) for the explicit tenant/key. Completion requires --apply --worker-stopped, matching fingerprint/time, and a successful audit insert.",
                    count
                );
                return Ok(());
            }
            if !worker_stopped {
                return Err("stop the original worker, verify its business/provider outcome, then pass --worker-stopped".into());
            }
            // Do not create a missing database or run implicit migrations.
            let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            conn.busy_timeout(std::time::Duration::from_secs(5))?;
            conn.pragma_update(None, "synchronous", "FULL")?;
            let receipt = reconcile_http_receipt(&mut conn, &resolution)?;
            println!(
                "Reconciled receipt with status {}; audit entry committed.",
                receipt.response_status
            );
        }
        _ => return Err("expected list or resolve".into()),
    }
    Ok(())
}
