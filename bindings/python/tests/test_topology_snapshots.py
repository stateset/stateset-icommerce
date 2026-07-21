"""Topology Snapshots API tests for the stateset_embedded Python bindings.

Counts cross as integers, signals as a JSON string, enums as snake_case
strings, timestamps as RFC3339 strings.
"""

import json

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_topology_snapshots_api_exists(commerce):
    assert commerce.topology_snapshots is not None


def test_topology_snapshot_full_lifecycle(commerce):
    ts = commerce.topology_snapshots
    if not ts.is_supported():
        pytest.skip("topology snapshots not supported by this engine build")

    snapshot = ts.capture(
        channels_total=2,
        channels_active=1,
        warehouses_total=3,
        products_total=100,
        open_orders=7,
        signals=json.dumps({"note": "nightly"}),
    )
    assert snapshot.id
    assert snapshot.channels_total == 2
    assert snapshot.channels_active == 1
    assert snapshot.warehouses_total == 3
    assert snapshot.products_total == 100
    assert snapshot.open_orders == 7
    assert snapshot.health == "healthy"
    assert json.loads(snapshot.signals) == {"note": "nightly"}
    assert snapshot.captured_at

    # get and latest find the snapshot
    found = ts.get(snapshot.id)
    assert found is not None
    assert found.id == snapshot.id
    latest = ts.latest()
    assert latest is not None
    assert latest.id == snapshot.id

    # health is derived, not supplied: no active channel is critical
    critical = ts.capture(
        channels_total=2,
        channels_active=0,
        warehouses_total=3,
        products_total=100,
        open_orders=0,
    )
    assert critical.health == "critical"
    assert critical.signals == "null"

    # list filters by health grade
    healthy = ts.list(health="healthy")
    assert any(s.id == snapshot.id for s in healthy)
    assert all(s.health == "healthy" for s in healthy)
    assert not any(s.id == critical.id for s in healthy)
    assert len(ts.list(limit=1)) == 1

    # bad health grade rejected
    with pytest.raises(ValueError):
        ts.list(health="not_a_grade")

    # delete removes it
    ts.delete(critical.id)
    assert ts.get(critical.id) is None
