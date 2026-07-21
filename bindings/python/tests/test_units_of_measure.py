"""Units of Measure API tests for the stateset_embedded Python bindings.

Factors cross as exact decimal strings; rule types as SYSTEM/SKU strings.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_units_of_measure_full_lifecycle(commerce):
    uom_api = commerce.units_of_measure
    if not uom_api.is_supported():
        pytest.skip("units-of-measure backend not available on this engine build")

    # Create a unit class
    unit_class = uom_api.create_class(name="Weight", description="Mass units")
    assert unit_class.id
    assert unit_class.name == "Weight"
    assert unit_class.description == "Mass units"
    assert any(c.id == unit_class.id for c in uom_api.list_classes())

    # Create two units within the class
    gram = uom_api.create_uom(
        unit_class_id=unit_class.id,
        name="Gram",
        abbreviation="g",
        factor="1",
    )
    kilogram = uom_api.create_uom(
        unit_class_id=unit_class.id,
        name="Kilogram",
        abbreviation="kg",
        factor="1000",
    )
    assert gram.unit_class_id == unit_class.id
    assert kilogram.factor == "1000"

    listed = uom_api.list_uoms(class_id=unit_class.id)
    ids = {u.id for u in listed}
    assert {gram.id, kilogram.id} <= ids

    # Mark the gram as the class base unit
    base = uom_api.set_base_uom(gram.id)
    assert base.id == gram.id
    assert base.is_base is True

    # Create a system conversion rule g -> kg
    rule = uom_api.create_rule(
        rule_type="SYSTEM",
        from_uom_id=gram.id,
        to_uom_id=kilogram.id,
        factor="0.001",
    )
    assert rule.rule_type == "SYSTEM"
    assert rule.product_id is None
    assert rule.factor == "0.001"
    assert any(r.id == rule.id for r in uom_api.list_rules())

    # Cleanup: rule, uom, class
    uom_api.delete_rule(rule.id)
    assert all(r.id != rule.id for r in uom_api.list_rules())

    uom_api.delete_uom(kilogram.id)
    assert all(u.id != kilogram.id for u in uom_api.list_uoms(class_id=unit_class.id))

    uom_api.delete_uom(gram.id)
    uom_api.delete_class(unit_class.id)
    assert all(c.id != unit_class.id for c in uom_api.list_classes())


def test_invalid_uuid_raises(commerce):
    uom_api = commerce.units_of_measure
    if not uom_api.is_supported():
        pytest.skip("units-of-measure backend not available on this engine build")
    with pytest.raises(ValueError):
        uom_api.delete_class("not-a-uuid")
