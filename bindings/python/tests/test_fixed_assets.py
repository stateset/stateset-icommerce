"""Fixed Assets API tests for the stateset_embedded Python bindings.

Money is exchanged as exact decimal strings (no float precision loss);
dates cross as ISO strings; enums as snake_case strings.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_fixed_assets_api_exists(commerce):
    assert commerce.fixed_assets is not None
    assert commerce.fixed_assets.is_supported() is True


def test_fixed_asset_full_lifecycle(commerce):
    fa = commerce.fixed_assets

    asset = fa.create(
        name="Forklift",
        category="machinery",
        acquisition_date="2026-01-01",
        acquisition_cost="10000.00",
        salvage_value="1000.00",
        useful_life_months=36,
        depreciation_method="straight_line",
    )
    assert asset.id
    assert asset.asset_number.startswith("FA-")
    assert asset.status == "draft"
    assert asset.category == "machinery"
    assert asset.acquisition_cost == "10000.00"
    assert asset.salvage_value == "1000.00"
    assert asset.accumulated_depreciation == "0"
    assert asset.book_value == "10000.00"
    assert asset.depreciation_method == "straight_line"

    # get and list find the asset
    found = fa.get(asset.id)
    assert found is not None
    assert found.id == asset.id
    listed = fa.list(category="machinery")
    assert any(a.id == asset.id for a in listed)

    # update changes the name
    updated = fa.update(asset.id, name="Forklift A")
    assert updated.name == "Forklift A"

    # place_in_service transitions draft -> in_service
    in_service = fa.place_in_service(asset.id, "2026-02-01")
    assert in_service.status == "in_service"
    assert in_service.in_service_date == "2026-02-01"

    # generate_schedule produces a straight-line schedule summing to the base
    schedule = fa.generate_schedule(asset.id)
    assert schedule.asset_id == asset.id
    assert schedule.method == "straight_line"
    assert len(schedule.entries) == 36
    assert schedule.total_depreciation == "9000.00"
    assert schedule.entries[0].amount == "250.00"
    assert schedule.entries[0].status == "scheduled"

    persisted = fa.get_schedule(asset.id)
    assert persisted is not None
    assert len(persisted.entries) == 36

    # post_depreciation posts periods and grows accumulated depreciation
    after = fa.post_depreciation(asset.id, 2)
    assert after.accumulated_depreciation == "500.00"
    assert after.book_value == "9500.00"

    schedule = fa.get_schedule(asset.id)
    assert schedule.entries[0].status == "posted"
    assert schedule.entries[1].status == "posted"
    assert schedule.entries[2].status == "scheduled"

    # dispose records proceeds and gain/loss
    disposed = fa.dispose(asset.id, "9800.00", "2026-06-30", "sold")
    assert disposed.status == "disposed"
    assert disposed.disposal is not None
    assert disposed.disposal.proceeds == "9800.00"
    assert disposed.disposal.book_value_at_disposal == "9500.00"
    assert disposed.disposal.gain_loss == "300.00"
    assert disposed.disposal.disposal_date == "2026-06-30"
    assert disposed.disposal.notes == "sold"


def test_write_off_disposes_with_zero_proceeds(commerce):
    fa = commerce.fixed_assets
    other = fa.create(
        name="Old laptop",
        category="computer_hardware",
        acquisition_date="2025-01-01",
        acquisition_cost="1200.00",
        salvage_value="0",
        useful_life_months=24,
        depreciation_method="straight_line",
    )
    fa.place_in_service(other.id, "2025-01-01")
    written = fa.write_off(other.id, "2026-01-01", "damaged")
    assert written.status == "written_off"
    assert written.disposal.proceeds == "0"


def test_invalid_inputs_raise(commerce):
    fa = commerce.fixed_assets
    with pytest.raises(ValueError):
        fa.get("not-a-uuid")
    with pytest.raises(ValueError):
        fa.create(
            name="Bad",
            category="not_a_category",
            acquisition_date="2026-01-01",
            acquisition_cost="1.00",
            salvage_value="0",
            useful_life_months=12,
            depreciation_method="straight_line",
        )
    with pytest.raises(ValueError):
        fa.create(
            name="Bad",
            category="machinery",
            acquisition_date="not-a-date",
            acquisition_cost="1.00",
            salvage_value="0",
            useful_life_months=12,
            depreciation_method="straight_line",
        )
    with pytest.raises(ValueError):
        # declining_balance requires declining_balance_rate
        fa.create(
            name="Bad",
            category="machinery",
            acquisition_date="2026-01-01",
            acquisition_cost="1.00",
            salvage_value="0",
            useful_life_months=12,
            depreciation_method="declining_balance",
        )
