"""Stock snapshot API tests for the stateset_embedded Python bindings.

Quantities are exchanged as exact decimal strings (no float precision loss);
timestamps cross as RFC3339 strings.
"""

import uuid
from decimal import Decimal

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

    commerce.inventory.create_item(sku="SKU-1", name="A", initial_quantity=10)
    commerce.inventory.create_item(sku="SKU-2", name="B", initial_quantity=5)
    snapshot = api.capture(label="EOM June 2026")
    assert snapshot.id
    assert snapshot.label == "EOM June 2026"
    assert snapshot.total_skus == 2
    assert Decimal(snapshot.total_units) == Decimal("15")
    assert snapshot.captured_at
    assert len(snapshot.lines) == 2

    line = next(l for l in snapshot.lines if l.sku == "SKU-1")
    assert line.stock_snapshot_id == snapshot.id
    assert line.product_id is None
    assert Decimal(line.quantity_on_hand) == Decimal("10")
    assert Decimal(line.quantity_available) == Decimal("10")

    other = next(l for l in snapshot.lines if l.sku == "SKU-2")
    assert other.product_id is None

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
