"""Month-end close orchestration tests for the Python bindings.

general_ledger.close_month(period_id, ...) runs depreciation, revenue
recognition, FX revaluation, and the period close in order; `dry_run=True`
computes candidates without writing anything.
"""

import pytest
from stateset_embedded import Commerce


def setup_close(commerce):
    gl = commerce.general_ledger
    gl.initialize_chart_of_accounts()

    # Wide open period covering today: GL auto-posting stamps entries with
    # today's date, which must fall inside an open period.
    period = gl.create_period(
        period_name="FY-wide",
        fiscal_year=2026,
        period_number=1,
        start_date="2020-01-01",
        end_date="2030-12-31",
    )
    assert period.status == "future"
    opened = gl.open_period(period.id)
    assert opened.status == "open"

    # Asset: $1200 over 12 months straight-line; all periods due by period end.
    asset = commerce.fixed_assets.create(
        name="Espresso machine",
        category="machinery",
        acquisition_date="2026-01-01",
        acquisition_cost="1200.00",
        salvage_value="0",
        useful_life_months=12,
        depreciation_method="straight_line",
    )
    commerce.fixed_assets.place_in_service(asset.id, "2026-01-01")
    commerce.fixed_assets.generate_schedule(asset.id)
    return period, asset


def test_dry_run_reports_candidates_without_writing():
    commerce = Commerce(":memory:")
    period, asset = setup_close(commerce)

    report = commerce.general_ledger.close_month(period.id, dry_run=True)
    assert report.dry_run is True
    assert report.period_id == period.id
    assert report.period_name == "FY-wide"
    assert report.depreciation.status == "dry_run"
    assert report.depreciation.entry_count == 12
    assert report.depreciation.total_amount == "1200.00"
    assert report.revenue_recognition.status == "dry_run"
    assert report.fx_revaluation.status == "skipped"
    assert report.period_close.status == "dry_run"
    assert report.closing_entry is None
    assert report.period_status == "open"

    # Nothing was written.
    after = commerce.fixed_assets.get(asset.id)
    assert after.accumulated_depreciation == "0"


def test_skip_flags_mark_steps_skipped():
    commerce = Commerce(":memory:")
    period, _asset = setup_close(commerce)

    report = commerce.general_ledger.close_month(
        period.id,
        skip_depreciation=True,
        skip_revenue_recognition=True,
        skip_fx_revaluation=True,
        skip_period_close=True,
    )
    assert report.depreciation.status == "skipped"
    assert report.revenue_recognition.status == "skipped"
    assert report.fx_revaluation.status == "skipped"
    assert report.period_close.status == "skipped"
    assert report.period_status == "open"


def test_real_run_posts_depreciation_with_period_close_skipped():
    commerce = Commerce(":memory:")
    period, asset = setup_close(commerce)

    # Without GL auto-posting there is no P&L activity, so skip the final
    # period close (which requires net income) and exercise the wet path
    # for the depreciation step.
    report = commerce.general_ledger.close_month(
        period.id, closed_by="binding-test", skip_period_close=True
    )
    assert report.dry_run is False
    assert report.depreciation.status == "executed"
    assert report.depreciation.entry_count == 12
    assert report.depreciation.total_amount == "1200.00"
    assert report.depreciation.warnings == []
    assert report.fx_revaluation.status == "skipped"
    assert report.period_close.status == "skipped"
    assert report.period_status == "open"

    after = commerce.fixed_assets.get(asset.id)
    assert after.accumulated_depreciation == "1200.00"
    assert after.status == "fully_depreciated"


def test_rejects_invalid_period_id():
    commerce = Commerce(":memory:")
    with pytest.raises(ValueError, match="Invalid UUID"):
        commerce.general_ledger.close_month("not-a-uuid")
