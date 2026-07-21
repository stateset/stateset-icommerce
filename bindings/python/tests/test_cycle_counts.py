"""Cycle Counts API tests for the stateset_embedded Python bindings.

Quantities are exchanged as exact decimal strings; enums as snake_case
strings; timestamps as RFC 3339 strings.
"""

import pytest
from stateset_embedded import (
    Commerce,
    CycleCountLineInput,
    RecordCycleCountLineInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_cycle_counts_api_exists(commerce):
    assert commerce.cycle_counts is not None


def test_cycle_count_full_lifecycle(commerce):
    cc = commerce.cycle_counts
    warehouse = commerce.warehouse.create_warehouse(
        code="WH-CC", name="Cycle Count Warehouse"
    )
    location = commerce.warehouse.create_location(
        warehouse_id=warehouse.id, code="A-01", location_type="bulk"
    )

    count = cc.create(
        warehouse_id=int(warehouse.id),
        location_id=int(location.id),
        counted_by="counter@example.com",
        lines=[
            CycleCountLineInput(sku="CC-SKU-1", expected_quantity="100"),
            CycleCountLineInput(sku="CC-SKU-2", expected_quantity="25.5"),
        ],
    )
    assert count.id
    assert count.status == "draft"
    assert count.warehouse_id == int(warehouse.id)
    assert count.counted_by == "counter@example.com"
    assert len(count.lines) == 2
    assert count.lines[0].expected_quantity == "100"
    assert count.lines[1].expected_quantity == "25.5"
    assert count.lines[0].counted_quantity is None

    # get and list find the count
    found = cc.get(count.id)
    assert found is not None
    assert found.id == count.id
    listed = cc.list(warehouse_id=int(warehouse.id), status="draft")
    assert any(c.id == count.id for c in listed)

    # start transitions draft -> in_progress
    started = cc.start(count.id)
    assert started.status == "in_progress"

    # record_counts records physical counts with variances
    recorded = cc.record_counts(
        count.id,
        [
            RecordCycleCountLineInput(sku="CC-SKU-1", counted_quantity="103"),
            RecordCycleCountLineInput(sku="CC-SKU-2", counted_quantity="25.5"),
        ],
    )
    line1 = next(l for l in recorded.lines if l.sku == "CC-SKU-1")
    line2 = next(l for l in recorded.lines if l.sku == "CC-SKU-2")
    assert line1.counted_quantity == "103"
    assert line1.variance == "3"
    assert line2.variance == "0.0"

    # complete applies variances and finishes the count
    completed = cc.complete(count.id)
    assert completed.status == "completed"
    assert completed.completed_at is not None


def test_cancel_abandons_a_draft_count(commerce):
    cc = commerce.cycle_counts
    warehouse = commerce.warehouse.create_warehouse(
        code="WH-CC2", name="Cancel Warehouse"
    )
    other = cc.create(
        warehouse_id=int(warehouse.id),
        lines=[CycleCountLineInput(sku="CC-SKU-3", expected_quantity="10")],
    )
    cancelled = cc.cancel(other.id)
    assert cancelled.status == "cancelled"


def test_invalid_inputs_raise(commerce):
    cc = commerce.cycle_counts
    with pytest.raises(ValueError):
        cc.get("not-a-uuid")
    warehouse = commerce.warehouse.create_warehouse(code="WH-CC3", name="Bad Input")
    with pytest.raises(ValueError):
        cc.create(
            warehouse_id=int(warehouse.id),
            lines=[CycleCountLineInput(sku="X", expected_quantity="not-a-number")],
        )
