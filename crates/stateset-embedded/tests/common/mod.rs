//! Shared helpers for Postgres parity tests.
//!
//! These test binaries all run against the same `POSTGRES_URL` database in CI,
//! so anything with a global uniqueness constraint must be get-or-create, not
//! create — `gl_periods` has `UNIQUE (fiscal_year, period_number)`, and four
//! binaries wanting a 2026-03 period used to race to create it, leaving the
//! Postgres parity lane red depending on execution order.

#![allow(dead_code)]

use chrono::NaiveDate;
use stateset_core::{CreateGlPeriod, GlPeriod};
use stateset_embedded::AsyncGeneralLedger;

/// Get-or-create a GL period and make sure it is open.
///
/// If another test binary already created the same `(fiscal_year,
/// period_number)`, the unique-constraint conflict is resolved by looking the
/// period up by date instead. `open_period` is a no-op on an already-open
/// period, so calling it unconditionally is safe.
pub async fn ensure_open_period(gl: &AsyncGeneralLedger, input: CreateGlPeriod) -> GlPeriod {
    let probe_date = input.start_date;
    let period = match gl.create_period(input).await {
        Ok(p) => p,
        Err(_) => gl
            .get_period_for_date(probe_date)
            .await
            .expect("look up existing period")
            .unwrap_or_else(|| panic!("period covering {probe_date} exists")),
    };
    gl.open_period(period.id).await.expect("open period")
}

/// Convenience wrapper: build the `CreateGlPeriod` from parts.
pub async fn ensure_open_month(gl: &AsyncGeneralLedger, fiscal_year: i32, month: u32) -> GlPeriod {
    let start = NaiveDate::from_ymd_opt(fiscal_year, month, 1).expect("valid month start");
    let end = if month == 12 {
        NaiveDate::from_ymd_opt(fiscal_year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(fiscal_year, month + 1, 1)
    }
    .expect("valid next month")
    .pred_opt()
    .expect("valid month end");
    ensure_open_period(
        gl,
        CreateGlPeriod {
            period_name: format!("{fiscal_year}-{month:02}"),
            fiscal_year,
            period_number: month as i32,
            start_date: start,
            end_date: end,
        },
    )
    .await
}
