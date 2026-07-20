"""Customer Segments API tests for the stateset_embedded Python bindings."""

import pytest
from stateset_embedded import Commerce, SegmentRuleInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_segments_api_exists(commerce):
    assert commerce.segments is not None
    assert commerce.segments.is_supported() is True


def test_segment_full_lifecycle(commerce):
    s = commerce.segments

    segment = s.create(
        name="VIP",
        description="High spenders",
        segment_type="dynamic",
        rules=[SegmentRuleInput("total_spent", "gte", "1000")],
    )
    assert segment.name == "VIP"
    assert segment.segment_type == "dynamic"
    assert len(segment.rules) == 1
    assert segment.rules[0].field == "total_spent"
    assert segment.rules[0].operator == "gte"
    assert segment.rules[0].value == "1000"
    assert segment.member_count == 0
    assert segment.id

    # get + update round-trip the rules
    assert s.get(segment.id).rules[0].operator == "gte"
    updated = s.update(
        segment.id, name="VIP Renamed", rules=[SegmentRuleInput("orders", "gt", "5")]
    )
    assert updated.name == "VIP Renamed"
    assert updated.rules[0].field == "orders"
    assert updated.rules[0].operator == "gt"

    # list
    assert any(x.id == segment.id for x in s.list())

    # delete
    s.delete(segment.id)
    assert s.get(segment.id) is None


def test_create_rejects_invalid_operator(commerce):
    s = commerce.segments
    with pytest.raises(Exception, match="(?i)invalid segment operator"):
        s.create(name="Bad", rules=[SegmentRuleInput("x", "nonsense", "1")])


def test_member_management(commerce):
    s = commerce.segments
    segment = s.create(name="Manual list")
    customer = commerce.customers.create(
        email="member@example.com", first_name="Mem", last_name="Ber"
    )

    membership = s.add_member(segment.id, customer.id)
    assert membership.segment_id == segment.id
    assert membership.customer_id == customer.id

    assert s.is_member(segment.id, customer.id) is True
    assert any(m.customer_id == customer.id for m in s.list_members(segment.id))

    s.remove_member(segment.id, customer.id)
    assert s.is_member(segment.id, customer.id) is False
