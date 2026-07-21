"""Shipping Zones API tests for the stateset_embedded Python bindings.

Money and weights are exchanged as exact decimal strings (no float precision
loss); timestamps cross as RFC 3339 strings; enums as snake_case strings.
"""

import pytest
from stateset_embedded import Commerce, ShippingCondition


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_shipping_zones_full_lifecycle(commerce):
    sz = commerce.shipping_zones
    if not sz.is_supported():
        pytest.skip("shipping zones not supported by this engine build")

    zone = sz.create(
        name="US Domestic",
        countries=["US"],
        regions=["CA"],
        postal_codes=["90*"],
        priority=1,
    )
    assert zone.id
    assert zone.name == "US Domestic"
    assert zone.countries == ["US"]
    assert zone.priority == 1
    assert zone.is_active is True

    found = sz.get(zone.id)
    assert found is not None and found.id == zone.id

    listed = sz.list(country="US")
    assert any(z.id == zone.id for z in listed)

    updated = sz.update(zone.id, name="US Domestic (West)")
    assert updated.name == "US Domestic (West)"

    matching = sz.find_matching_zones("US", "CA", "90210")
    assert any(z.id == zone.id for z in matching)

    method = sz.create_method(
        zone_id=zone.id,
        name="Standard",
        method_type="weight_based",
        base_rate="5.00",
        currency="USD",
        carrier="UPS",
        min_delivery_days=2,
        max_delivery_days=5,
        conditions=[ShippingCondition(rate="9.00", min_weight="1000")],
    )
    assert method.id
    assert method.zone_id == zone.id
    assert method.method_type == "weight_based"
    assert method.base_rate == "5.00"
    assert method.currency == "USD"
    assert len(method.conditions) == 1
    assert method.conditions[0].rate == "9.00"

    got_method = sz.get_method(method.id)
    assert got_method is not None and got_method.id == method.id

    methods = sz.list_methods(zone_id=zone.id)
    assert any(m.id == method.id for m in methods)

    rates = sz.calculate_rates(
        country="US",
        currency="USD",
        region="CA",
        postal_code="90210",
        weight="500",
        order_total="100.00",
    )
    assert any(r.method_id == method.id for r in rates)

    sz.delete_method(method.id)
    assert sz.get_method(method.id) is None

    sz.delete(zone.id)
    assert sz.get(zone.id) is None
