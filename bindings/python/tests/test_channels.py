"""Channels API tests for the stateset_embedded Python bindings.

Enums cross as snake_case strings, metadata as JSON strings, timestamps as
RFC3339 strings.
"""

import json

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_channel_full_lifecycle(commerce):
    ch = commerce.channels
    if not ch.is_supported():
        pytest.skip("channels backend not supported on this engine build")

    channel = ch.create(
        name="Shopify US",
        channel_type="sales_channel",
        integration="shopify",
        tags=["retail"],
        metadata=json.dumps({"region": "us"}),
    )
    assert channel.id
    assert channel.name == "Shopify US"
    assert channel.channel_type == "sales_channel"
    assert channel.integration == "shopify"
    assert channel.status == "active"
    assert channel.api_locked is False
    assert channel.tags == ["retail"]
    assert json.loads(channel.metadata)["region"] == "us"

    # get and list find the channel
    found = ch.get(channel.id)
    assert found is not None
    assert found.id == channel.id
    listed = ch.list(channel_type="sales_channel", status="active")
    assert any(c.id == channel.id for c in listed)

    # update applies PATCH semantics
    updated = ch.update(channel.id, name="Shopify NA", status="paused")
    assert updated.name == "Shopify NA"
    assert updated.status == "paused"
    assert updated.integration == "shopify"

    # lock / unlock
    locked = ch.set_lock(channel.id, True)
    assert locked.api_locked is True
    unlocked = ch.set_lock(channel.id, False)
    assert unlocked.api_locked is False

    # a fresh channel has no product mappings
    assert ch.list_product_mappings(channel.id) == []

    # soft-delete
    ch.delete(channel.id)
    deleted = ch.get(channel.id)
    assert deleted is None or deleted.status == "deleted"


def test_channel_get_missing_returns_none(commerce):
    ch = commerce.channels
    if not ch.is_supported():
        pytest.skip("channels backend not supported on this engine build")
    assert ch.get("00000000-0000-0000-0000-000000000000") is None


def test_channel_invalid_enum_raises(commerce):
    ch = commerce.channels
    if not ch.is_supported():
        pytest.skip("channels backend not supported on this engine build")
    with pytest.raises(ValueError):
        ch.create(name="Bad", channel_type="not_a_channel_type")
