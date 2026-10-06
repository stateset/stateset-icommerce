//! Timing harness for `:memory:` store construction under concurrency.
//!
//! Spawns `THREADS` threads that each open `ITERS` in-memory stores and run a
//! first query, then prints construction-latency percentiles. The first store
//! of the process also builds the migration template; it is timed separately.
//!
//! ```text
//! cargo run -p stateset-db --example memdb_construction -- 32 2
//! STATESET_SQLITE_NO_TEMPLATE=1 cargo run -p stateset-db --example memdb_construction -- 32 2
//! ```
//!
//! Wrap it in `taskset -c 0,1` (CPU starvation) or run a bulk writer such as
//! `dd if=/dev/zero of=/tmp/load bs=1M count=2000` next to it (fsync stalls)
//! to reproduce a loaded CI machine.

use stateset_db::SqliteDatabase;
use std::time::Instant;

fn open_and_query() -> f64 {
    let start = Instant::now();
    let db = SqliteDatabase::in_memory().expect("in-memory store opens");
    let applied: i64 = db
        .conn()
        .expect("connection")
        .query_row("SELECT count(*) FROM _migrations", [], |row| row.get(0))
        .expect("first query");
    assert!(applied > 0);
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let mut args = std::env::args().skip(1);
    let threads: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(8);
    let iters: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);

    let first_ms = open_and_query();
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            std::thread::spawn(move || (0..iters).map(|_| open_and_query()).collect::<Vec<_>>())
        })
        .collect();
    let mut all: Vec<f64> =
        handles.into_iter().flat_map(|h| h.join().expect("worker thread")).collect();
    all.sort_by(f64::total_cmp);
    let pct = |q: f64| all[((all.len() - 1) as f64 * q).round() as usize];
    println!(
        "first store {first_ms:.1} ms; {threads} threads x {iters}: construct+first query \
         p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms; wall {:.2} s",
        pct(0.5),
        pct(0.95),
        pct(1.0),
        start.elapsed().as_secs_f64()
    );
}
