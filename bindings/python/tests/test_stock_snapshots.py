"""Stock snapshot API tests for the stateset_embedded Python bindings.

Quantities are exchanged as exact decimal strings (no float precision loss);
timestamps cross as RFC3339 strings.
"""

import uuid

import pytest
from stateset_embedded import CaptureStockLineInput, Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_stock_snapshots_api_exists(commerce):
    assert commerce.stock_snapshots is not None


def test_stock_snapshot_full_lifecycle(commerce):
    api = commerce.stock_snapshots
    if not api.is_supported():
        pytest.skip("stock snapshots backend not available on this build")

    product_a = str(uuid.uuid4())
    product_b = str(uuid.uuid4())

    snapshot = api.capture(
        lines=[
            CaptureStockLineInput(product_a, "SKU-1", "10.00", "8.00", "MAIN"),
            CaptureStockLineInput(product_b, "SKU-2", "5.00", "5.00"),
        ],
        label="EOM June 2026",
    )
    assert snapshot.id
    assert snapshot.label == "EOM June 2026"
    assert snapshot.total_skus == 2
    assert snapshot.total_units == "15.00"
    assert snapshot.captured_at
    assert len(snapshot.lines) == 2

    line = next(l for l in snapshot.lines if l.sku == "SKU-1")
    assert line.stock_snapshot_id == snapshot.id
    assert line.product_id == product_a
    assert line.quantity_on_hand == "10.00"
    assert line.quantity_available == "8.00"
    assert line.location == "MAIN"

    other = next(l for l in snapshot.lines if l.sku == "SKU-2")
    assert other.location is None

    # get finds the snapshot
    found = api.get(snapshot.id)
    assert found is not None
    assert found.id == snapshot.id
    assert len(found.lines) == 2

    # latest returns the most recent snapshot
    latest = api.latest()
    assert latest is not None
    assert latest.id == snapshot.id

    # list finds it, and honours pagination
    listed = api.list()
    assert any(s.id == snapshot.id for s in listed)
    assert len(api.list(limit=1)) == 1

    # delete removes it
    api.delete(snapshot.id)
    assert api.get(snapshot.id) is None


def test_invalid_inputs_raise(commerce):
    api = commerce.stock_snapshots
    if not api.is_supported():
        pytest.skip("stock snapshots backend not available on this build")

    with pytest.raises(ValueError):
        api.get("not-a-uuid")
    with pytest.raises(ValueError):
        api.delete("not-a-uuid")
    with pytest.raises(ValueError):
        api.capture(lines=[CaptureStockLineInput("not-a-uuid", "SKU-1", "1", "1")])
    with pytest.raises(ValueError):
        api.capture(
            lines=[CaptureStockLineInput(str(uuid.uuid4()), "SKU-1", "abc", "1")]
        )
