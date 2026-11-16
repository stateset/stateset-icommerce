//! Asset accounting and archived schedule revisions inside the asset transaction.
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, DepreciationEntry, DepreciationEntryStatus, FixedAsset};
use uuid::Uuid;
fn invalid(message: &str) -> rusqlite::Error {
    super::bins::smuggle(CommerceError::ValidationError(message.into()))
}
pub(super) fn acquire(tx: &rusqlite::Transaction<'_>, asset: &FixedAsset) -> rusqlite::Result<()> {
    validate_accounts(tx, asset)?;

    let Some(config) =
        super::general_ledger::SqliteGeneralLedgerRepository::get_auto_posting_config_with_conn(
            tx,
        )?
    else {
        return Ok(());
    };
    let input =
        crate::asset_accounting::acquisition(asset, &config).map_err(super::bins::smuggle)?;
    super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
        tx, &input, "system",
    )?;
    Ok(())
}
pub(super) fn require_acquired(
    tx: &rusqlite::Transaction<'_>,
    asset: &FixedAsset,
) -> rusqlite::Result<()> {
    let original = tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type = 'fixed_asset_acquisition' AND source_document_id = ?1 AND status = 'posted'", rusqlite::params![(asset.id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
    if original.is_none() {
        return Err(invalid(
            "Reconcile the asset acquisition journal before depreciation or disposal",
        ));
    }
    Ok(())
}
pub(super) fn dispose(
    tx: &rusqlite::Transaction<'_>,
    asset: &FixedAsset,
    date: chrono::NaiveDate,
    proceeds: Decimal,
) -> rusqlite::Result<()> {
    validate_accounts(tx, asset)?;

    let Some(config) =
        super::general_ledger::SqliteGeneralLedgerRepository::get_auto_posting_config_with_conn(
            tx,
        )?
    else {
        return Ok(());
    };
    require_acquired(tx, asset)?;
    let periods = tx.query_row("SELECT period FROM fixed_asset_depreciation_entries WHERE asset_id = ?1 AND status = 'posted' ORDER BY period DESC LIMIT 1", rusqlite::params![(asset.id).to_string()], |r| r.get::<_, i32>(0)).optional()?;
    if periods
        .is_some_and(|p| asset.depreciation_period_end(p as u32).is_some_and(|end| end > date))
    {
        return Err(invalid(
            "Reverse depreciation dated after the disposal before disposing the asset",
        ));
    }
    for input in crate::asset_accounting::disposal(asset, &config, date, proceeds)
        .map_err(super::bins::smuggle)?
    {
        super::general_ledger::SqliteGeneralLedgerRepository::create_posted_entry_with_conn(
            tx, &input, "system",
        )?;
    }
    Ok(())
}
pub(super) fn revise(
    tx: &rusqlite::Transaction<'_>,
    before: &FixedAsset,
    after: &FixedAsset,
) -> rusqlite::Result<()> {
    validate_accounts(tx, after)?;

    let recorded = tx.query_row("SELECT id FROM gl_journal_entries WHERE source_document_type IN ('fixed_asset_acquisition','fixed_asset_depreciation') AND source_document_id = ?1 LIMIT 1", rusqlite::params![(before.id).to_string()], |r| super::parse_uuid_row(&r.get::<_, String>(0)?, "stock", "id")).optional()?;
    if recorded.is_some()
        && (before.asset_account_id != after.asset_account_id
            || before.acquisition_offset_account_id != after.acquisition_offset_account_id)
    {
        return Err(invalid("Asset posting accounts are fixed after journal entries exist"));
    }
    let depreciated: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM gl_journal_entries WHERE source_document_type = 'fixed_asset_depreciation' AND source_document_id = ?1)",[before.id.to_string()],|r| r.get(0))?;
    if depreciated
        && (before.accumulated_depreciation_account_id != after.accumulated_depreciation_account_id
            || before.depreciation_expense_account_id != after.depreciation_expense_account_id)
    {
        return Err(invalid(
            "Depreciation accounts are fixed after depreciation journal entries exist",
        ));
    }
    let rows = {
        let mut stmt = tx.prepare("SELECT period,amount,accumulated,book_value,status FROM fixed_asset_depreciation_entries WHERE asset_id = ?1 ORDER BY period")?;
        let rows = stmt.query_map(rusqlite::params![(before.id).to_string()], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                super::parse_decimal_row(&r.get::<_, String>(1)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(2)?, "stock", "quantity")?,
                super::parse_decimal_row(&r.get::<_, String>(3)?, "stock", "quantity")?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let entries: Vec<DepreciationEntry> = rows
        .into_iter()
        .map(|(period, amount, accumulated, book_value, status)| {
            Ok(DepreciationEntry {
                period: u32::try_from(period)
                    .map_err(|_| invalid("Invalid depreciation period"))?,
                amount,
                accumulated,
                book_value,
                status: status.parse().map_err(|_| invalid("Invalid depreciation status"))?,
            })
        })
        .collect::<rusqlite::Result<_>>()?;
    let Some(revised) =
        crate::asset_accounting::replan(before, after, &entries).map_err(super::bins::smuggle)?
    else {
        return Ok(());
    };
    let asset_json =
        serde_json::to_string(before).map_err(|_| invalid("Unable to archive asset"))?;
    let schedule_json =
        serde_json::to_string(&entries).map_err(|_| invalid("Unable to archive depreciation"))?;
    tx.execute("INSERT INTO fixed_asset_schedule_revisions (id,asset_id,previous_asset_json,previous_schedule_json,revised_at) VALUES (?1,?2,?3,?4,?5)", rusqlite::params![(Uuid::new_v4()).to_string(), (before.id).to_string(), &asset_json, &schedule_json, chrono::Utc::now().to_rfc3339()])?;
    tx.execute(
        "DELETE FROM fixed_asset_depreciation_entries WHERE asset_id = ?1 AND status = 'scheduled'",
        rusqlite::params![(before.id).to_string()],
    )?;
    for entry in revised.iter().filter(|e| e.status == DepreciationEntryStatus::Scheduled) {
        tx.execute("INSERT INTO fixed_asset_depreciation_entries (asset_id,period,amount,accumulated,book_value,status) VALUES (?1,?2,?3,?4,?5,'scheduled')", rusqlite::params![(before.id).to_string(), entry.period as i32, (entry.amount).to_string(), (entry.accumulated).to_string(), (entry.book_value).to_string()])?;
    }
    if matches!(
        after.status,
        stateset_core::FixedAssetStatus::InService
            | stateset_core::FixedAssetStatus::FullyDepreciated
    ) {
        let status = if after.book_value() > after.salvage_value {
            "in_service"
        } else {
            "fully_depreciated"
        };
        tx.execute(
            "UPDATE fixed_assets SET status = ?1 WHERE id = ?2",
            rusqlite::params![status, before.id.to_string()],
        )?;
    }
    Ok(())
}

pub(super) fn validate_accounts(
    tx: &rusqlite::Transaction<'_>,
    asset: &FixedAsset,
) -> rusqlite::Result<()> {
    for (id, types) in [
        (asset.asset_account_id, &["asset"][..]),
        (asset.accumulated_depreciation_account_id, &["asset"][..]),
        (asset.depreciation_expense_account_id, &["expense"][..]),
        (asset.acquisition_offset_account_id, &["asset", "liability", "equity"][..]),
        (asset.disposal_gain_loss_account_id, &["expense", "revenue"][..]),
    ] {
        if let Some(id) = id {
            let kind = tx
                .query_row(
                    "SELECT account_type FROM gl_accounts WHERE id = ?1",
                    rusqlite::params![(id).to_string()],
                    |r| r.get::<_, String>(0),
                )
                .optional()?;
            if !kind.is_some_and(|kind| types.contains(&kind.as_str())) {
                return Err(invalid(
                    "Asset posting account has the wrong account type or does not exist",
                ));
            }
        }
    }
    Ok(())
}
