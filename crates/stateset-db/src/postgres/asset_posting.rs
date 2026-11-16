//! Asset accounting and archived schedule revisions inside the asset transaction.
use super::map_db_error;
use rust_decimal::Decimal;
use stateset_core::{CommerceError, DepreciationEntry, DepreciationEntryStatus, FixedAsset};
use uuid::Uuid;
fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}
pub(super) async fn acquire(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &FixedAsset,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
) -> stateset_core::Result<()> {
    validate_accounts(tx, asset).await?;

    let Some(config) =
        super::general_ledger::PgGeneralLedgerRepository::get_auto_posting_config_tx(tx).await?
    else {
        return Ok(());
    };
    let input = crate::asset_accounting::acquisition(asset, &config)?;
    repo.create_posted_entry_tx(tx, &input, "system").await?;
    Ok(())
}
pub(super) async fn require_acquired(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &FixedAsset,
) -> stateset_core::Result<()> {
    let original = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type = 'fixed_asset_acquisition' AND source_document_id = $1 AND status = 'posted'").bind(asset.id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    if original.is_none() {
        return Err(invalid(
            "Reconcile the asset acquisition journal before depreciation or disposal",
        ));
    }
    Ok(())
}
pub(super) async fn dispose(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &FixedAsset,
    date: chrono::NaiveDate,
    proceeds: Decimal,
    repo: &super::general_ledger::PgGeneralLedgerRepository,
) -> stateset_core::Result<()> {
    validate_accounts(tx, asset).await?;

    let Some(config) =
        super::general_ledger::PgGeneralLedgerRepository::get_auto_posting_config_tx(tx).await?
    else {
        return Ok(());
    };
    require_acquired(tx, asset).await?;
    let periods = sqlx::query_scalar::<_, i32>("SELECT period FROM fixed_asset_depreciation_entries WHERE asset_id = $1 AND status = 'posted' ORDER BY period DESC LIMIT 1").bind(asset.id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    if periods
        .is_some_and(|p| asset.depreciation_period_end(p as u32).is_some_and(|end| end > date))
    {
        return Err(invalid(
            "Reverse depreciation dated after the disposal before disposing the asset",
        ));
    }
    for input in crate::asset_accounting::disposal(asset, &config, date, proceeds)? {
        repo.create_posted_entry_tx(tx, &input, "system").await?;
    }
    Ok(())
}
pub(super) async fn revise(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    before: &FixedAsset,
    after: &FixedAsset,
) -> stateset_core::Result<()> {
    validate_accounts(tx, after).await?;

    let recorded = sqlx::query_scalar::<_, Uuid>("SELECT id FROM gl_journal_entries WHERE source_document_type IN ('fixed_asset_acquisition','fixed_asset_depreciation') AND source_document_id = $1 LIMIT 1").bind(before.id).fetch_optional(tx.as_mut()).await.map_err(map_db_error)?;
    if recorded.is_some()
        && (before.asset_account_id != after.asset_account_id
            || before.acquisition_offset_account_id != after.acquisition_offset_account_id)
    {
        return Err(invalid("Asset posting accounts are fixed after journal entries exist"));
    }
    let depreciated: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM gl_journal_entries WHERE source_document_type = 'fixed_asset_depreciation' AND source_document_id = $1)").bind(before.id).fetch_one(tx.as_mut()).await.map_err(map_db_error)?;
    if depreciated
        && (before.accumulated_depreciation_account_id != after.accumulated_depreciation_account_id
            || before.depreciation_expense_account_id != after.depreciation_expense_account_id)
    {
        return Err(invalid(
            "Depreciation accounts are fixed after depreciation journal entries exist",
        ));
    }
    let rows = sqlx::query_as::<_, (i32, Decimal, Decimal, Decimal, String)>("SELECT period,amount,accumulated,book_value,status FROM fixed_asset_depreciation_entries WHERE asset_id = $1 ORDER BY period").bind(before.id).fetch_all(tx.as_mut()).await.map_err(map_db_error)?;
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
        .collect::<stateset_core::Result<_>>()?;
    let Some(revised) = crate::asset_accounting::replan(before, after, &entries)? else {
        return Ok(());
    };
    let asset_json =
        serde_json::to_string(before).map_err(|_| invalid("Unable to archive asset"))?;
    let schedule_json =
        serde_json::to_string(&entries).map_err(|_| invalid("Unable to archive depreciation"))?;
    sqlx::query("INSERT INTO fixed_asset_schedule_revisions (id,asset_id,previous_asset_json,previous_schedule_json,revised_at) VALUES ($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(before.id).bind(&asset_json).bind(&schedule_json).bind(chrono::Utc::now().to_rfc3339()).execute(tx.as_mut()).await.map_err(map_db_error)?;
    sqlx::query(
        "DELETE FROM fixed_asset_depreciation_entries WHERE asset_id = $1 AND status = 'scheduled'",
    )
    .bind(before.id)
    .execute(tx.as_mut())
    .await
    .map_err(map_db_error)?;
    for entry in revised.iter().filter(|e| e.status == DepreciationEntryStatus::Scheduled) {
        sqlx::query("INSERT INTO fixed_asset_depreciation_entries (asset_id,period,amount,accumulated,book_value,status) VALUES ($1,$2,$3,$4,$5,'scheduled')").bind(before.id).bind(entry.period as i32).bind(entry.amount).bind(entry.accumulated).bind(entry.book_value).execute(tx.as_mut()).await.map_err(map_db_error)?;
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
        sqlx::query("UPDATE fixed_assets SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(before.id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
    }
    Ok(())
}

pub(super) async fn validate_accounts(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    asset: &FixedAsset,
) -> stateset_core::Result<()> {
    for (id, types) in [
        (asset.asset_account_id, &["asset"][..]),
        (asset.accumulated_depreciation_account_id, &["asset"][..]),
        (asset.depreciation_expense_account_id, &["expense"][..]),
        (asset.acquisition_offset_account_id, &["asset", "liability", "equity"][..]),
        (asset.disposal_gain_loss_account_id, &["expense", "revenue"][..]),
    ] {
        if let Some(id) = id {
            let kind = sqlx::query_scalar::<_, String>(
                "SELECT account_type FROM gl_accounts WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(map_db_error)?;
            if !kind.is_some_and(|kind| types.contains(&kind.as_str())) {
                return Err(invalid(
                    "Asset posting account has the wrong account type or does not exist",
                ));
            }
        }
    }
    Ok(())
}
