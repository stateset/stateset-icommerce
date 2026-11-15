//! Asset journals and prospective schedule revisions shared by both databases.
use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{
    AutoPostingConfig, CommerceError, CreateJournalEntry, CreateJournalEntryLine,
    DepreciationEntry, DepreciationEntryStatus, FixedAsset, JournalEntryType, Result,
    generate_depreciation_schedule,
};

fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}

fn entry(
    asset: &FixedAsset,
    kind: &str,
    date: NaiveDate,
    lines: Vec<CreateJournalEntryLine>,
) -> CreateJournalEntry {
    CreateJournalEntry {
        entry_date: date,
        entry_type: Some(JournalEntryType::Standard),
        description: format!("{kind} {}", asset.asset_number),
        lines,
        source_document_type: Some(kind.into()),
        source_document_id: Some(asset.id),
        auto_post: Some(true),
        fx: None,
    }
}

pub(crate) fn acquisition(
    asset: &FixedAsset,
    config: &AutoPostingConfig,
) -> Result<CreateJournalEntry> {
    let capital = asset.asset_account_id.ok_or_else(|| {
        invalid("Set the asset's capitalization account before acquisition posting")
    })?;
    let offset = asset.acquisition_offset_account_id.unwrap_or(config.cash_account_id);
    if capital == offset {
        return Err(invalid("Acquisition asset and offset accounts must differ"));
    }
    Ok(entry(
        asset,
        "fixed_asset_acquisition",
        asset.acquisition_date,
        vec![
            CreateJournalEntryLine::debit(
                capital,
                asset.acquisition_cost,
                Some("Asset acquisition".into()),
            ),
            CreateJournalEntryLine::credit(
                offset,
                asset.acquisition_cost,
                Some("Acquisition settlement".into()),
            ),
        ],
    ))
}

pub(crate) fn disposal(
    asset: &FixedAsset,
    config: &AutoPostingConfig,
    date: NaiveDate,
    proceeds: Decimal,
) -> Result<Vec<CreateJournalEntry>> {
    if date < asset.acquisition_date || asset.in_service_date.is_some_and(|start| date < start) {
        return Err(invalid("Asset disposal cannot precede acquisition or service"));
    }
    let capital = asset
        .asset_account_id
        .ok_or_else(|| invalid("Set the asset's capitalization account before disposal"))?;
    let gain_loss = asset.disposal_gain_loss_account_id.ok_or_else(|| {
        invalid("Set the asset's disposal gain/loss account before disposal or write-off")
    })?;
    if capital == gain_loss {
        return Err(invalid("Disposal capital and gain/loss accounts must differ"));
    }
    let book = asset
        .acquisition_cost
        .checked_sub(asset.accumulated_depreciation)
        .ok_or_else(|| invalid("Asset carrying amount overflow"))?;
    if book < Decimal::ZERO || proceeds < Decimal::ZERO {
        return Err(invalid("Invalid asset carrying amount or proceeds"));
    }
    let mut lines = vec![CreateJournalEntryLine::credit(
        capital,
        asset.acquisition_cost,
        Some("Remove asset cost".into()),
    )];
    if asset.accumulated_depreciation > Decimal::ZERO {
        let accumulated = asset
            .accumulated_depreciation_account_id
            .ok_or_else(|| invalid("Set the accumulated depreciation account before disposal"))?;
        lines.push(CreateJournalEntryLine::debit(
            accumulated,
            asset.accumulated_depreciation,
            Some("Remove accumulated depreciation".into()),
        ));
    }
    if book > Decimal::ZERO {
        lines.push(CreateJournalEntryLine::debit(
            gain_loss,
            book,
            Some("Asset carrying value on disposal".into()),
        ));
    }
    // Removal retains the acquisition rate; cash proceeds use the disposal date's rate.
    let mut entries = vec![entry(asset, "fixed_asset_disposal", date, lines)];
    if proceeds > Decimal::ZERO {
        entries.push(entry(
            asset,
            "fixed_asset_proceeds",
            date,
            vec![
                CreateJournalEntryLine::debit(
                    config.cash_account_id,
                    proceeds,
                    Some("Disposal proceeds".into()),
                ),
                CreateJournalEntryLine::credit(
                    gain_loss,
                    proceeds,
                    Some("Asset disposal recovery".into()),
                ),
            ],
        ));
    }
    Ok(entries)
}

pub(crate) fn replan(
    before: &FixedAsset,
    after: &FixedAsset,
    entries: &[DepreciationEntry],
) -> Result<Option<Vec<DepreciationEntry>>> {
    if after.salvage_value > after.acquisition_cost {
        return Err(invalid("Salvage value cannot exceed acquisition cost"));
    }
    if after.in_service_date.is_some_and(|date| date < after.acquisition_date) {
        return Err(invalid("Service date cannot precede acquisition"));
    }
    if before.salvage_value == after.salvage_value
        && before.useful_life_months == after.useful_life_months
        && before.in_service_date == after.in_service_date
    {
        return Ok(None);
    }
    // An asset without a schedule remains explicitly unscheduled. Revisions
    // replace existing future entries; schedule creation is a separate action.
    if entries.is_empty() && before.accumulated_depreciation.is_zero() {
        return Ok(None);
    }
    let posted: Vec<_> =
        entries.iter().filter(|e| e.status == DepreciationEntryStatus::Posted).cloned().collect();
    if !posted.is_empty() && before.in_service_date != after.in_service_date {
        return Err(invalid("Reverse posted depreciation before changing the service date"));
    }
    let mut accumulated = Decimal::ZERO;
    for (index, e) in posted.iter().enumerate() {
        accumulated = accumulated
            .checked_add(e.amount)
            .ok_or_else(|| invalid("Depreciation amount overflow"))?;
        if e.period != index as u32 + 1 || e.accumulated != accumulated {
            return Err(invalid("Reconcile posted depreciation before revising its schedule"));
        }
    }
    if accumulated != before.accumulated_depreciation {
        return Err(invalid("Posted depreciation does not match the asset register"));
    }
    let remaining_book = after
        .acquisition_cost
        .checked_sub(accumulated)
        .ok_or_else(|| invalid("Asset carrying value overflow"))?;
    if after.salvage_value > remaining_book {
        return Err(invalid("Revised salvage value cannot exceed the remaining book value"));
    }
    let periods = after
        .useful_life_months
        .checked_sub(posted.len() as u32)
        .ok_or_else(|| invalid("Revised life cannot precede posted depreciation periods"))?;
    if periods == 0 && remaining_book > after.salvage_value {
        return Err(invalid("Remaining depreciable value needs at least one future period"));
    }
    let mut future = generate_depreciation_schedule(
        after.depreciation_method,
        remaining_book,
        after.salvage_value,
        periods,
    );
    if future.is_empty() && remaining_book > after.salvage_value {
        return Err(invalid("Cannot generate a time-based schedule for this depreciation method"));
    }
    for e in &mut future {
        e.period += posted.len() as u32;
        e.accumulated += accumulated;
        e.book_value = after.acquisition_cost - e.accumulated;
    }
    let mut revised = posted;
    revised.extend(future);
    Ok(Some(revised))
}
