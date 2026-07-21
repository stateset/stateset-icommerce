"""Purgatory (order ingestion staging) tests for the stateset_embedded bindings.

Quantities cross as exact decimal strings, metadata as JSON strings,
timestamps as RFC3339 strings.
"""

import json

import pytest
from stateset_embedded import Commerce, IngestLineItemInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_purgatory_full_lifecycle(commerce):
    pg = commerce.purgatory
    if not pg.is_supported():
        pytest.skip("purgatory backend not available on this engine build")

    order = pg.ingest(
        external_order_id="SHOP-1001",
        items=[
            IngestLineItemInput(external_sku="SKU-A", quantity="2"),
            IngestLineItemInput(external_sku="FEE-SHIP", quantity="1"),
        ],
        external_status="paid",
        metadata=json.dumps({"source": "shopify"}),
    )
    assert order.id
    assert order.external_order_id == "SHOP-1001"
    assert order.external_status == "paid"
    assert order.is_posted is False
    assert len(order.items) == 2
    by_sku = {i.external_sku: i for i in order.items}
    assert by_sku["SKU-A"].quantity == "2"
    assert by_sku["SKU-A"].is_resolved is False
    assert order.is_ready_to_post is False
    assert order.unresolved_count == "2"
    assert json.loads(order.metadata)["source"] == "shopify"
    assert order.created_at

    # get and list find the staged order
    found = pg.get(order.id)
    assert found is not None
    assert found.id == order.id
    listed = pg.list(is_posted=False)
    assert any(o.id == order.id for o in listed)

    # resolve both lines: one flagged non-physical, one ignored
    mapped = pg.map_line(order.id, order.items[0].id, non_physical=True)
    assert mapped.items[0].non_physical is True
    assert mapped.items[0].is_resolved is True
    assert mapped.unresolved_count == "1"

    mapped = pg.map_line(order.id, order.items[1].id, ignore_item=True)
    assert mapped.items[1].ignore_item is True
    assert mapped.is_ready_to_post is True
    assert mapped.unresolved_count == "0"

    # post moves the order out of purgatory
    posted = pg.post(order.id)
    assert posted.is_posted is True

    # delete removes it
    pg.delete(order.id)
    assert pg.get(order.id) is None


def test_invalid_inputs_raise(commerce):
    pg = commerce.purgatory
    if not pg.is_supported():
        pytest.skip("purgatory backend not available on this engine build")

    with pytest.raises(ValueError):
        pg.get("not-a-uuid")
    with pytest.raises(ValueError):
        pg.ingest(
            external_order_id="BAD-1",
            items=[IngestLineItemInput(external_sku="SKU-A", quantity="abc")],
        )
    with pytest.raises(ValueError):
        pg.ingest(
            external_order_id="BAD-2",
            items=[IngestLineItemInput(external_sku="SKU-A", quantity="1")],
            metadata="{not json",
        )
