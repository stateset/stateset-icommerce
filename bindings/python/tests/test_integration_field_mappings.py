"""Integration field-mapping API tests for the stateset_embedded Python bindings.

Enums cross as snake_case strings; timestamps as RFC3339 strings.
"""

import pytest
from stateset_embedded import Commerce, NewIntegrationFieldMapping


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_integration_field_mapping_lifecycle(commerce):
    api = commerce.integration_field_mappings
    assert api is not None
    if not api.is_supported():
        pytest.skip("integration field mappings not supported on this engine build")

    mapping = api.create(
        integration_account="acct-1",
        mapping_group="order",
        source_field="order.customer.email",
        destination_field="email",
        transform="lowercase",
        fallback="default@x.test",
    )
    assert mapping.id
    assert mapping.integration_account == "acct-1"
    assert mapping.mapping_group == "order"
    assert mapping.source_field == "order.customer.email"
    assert mapping.destination_field == "email"
    assert mapping.transform == "lowercase"
    assert mapping.fallback == "default@x.test"
    assert mapping.is_active is True
    assert mapping.created_at

    found = api.get(mapping.id)
    assert found is not None
    assert found.id == mapping.id

    listed = api.list(integration_account="acct-1", mapping_group="order")
    assert any(m.id == mapping.id for m in listed)

    updated = api.update(
        mapping.id,
        destination_field="email_address",
        transform="trim",
        is_active=False,
    )
    assert updated.destination_field == "email_address"
    assert updated.transform == "trim"
    assert updated.is_active is False

    created = api.bulk_create(
        [
            NewIntegrationFieldMapping(
                integration_account="acct-1",
                mapping_group="shipment",
                source_field="shipment.tracking",
                destination_field="tracking_number",
            ),
            NewIntegrationFieldMapping(
                integration_account="acct-1",
                mapping_group="shipment",
                source_field="shipment.carrier",
                destination_field="carrier",
                transform="uppercase",
            ),
        ]
    )
    assert created == 2

    groups = api.distinct_groups("acct-1")
    assert "order" in groups
    assert "shipment" in groups

    shipment_ids = [m.id for m in api.list(mapping_group="shipment")]
    assert len(shipment_ids) == 2
    assert api.bulk_delete(shipment_ids) == 2
    assert api.list(mapping_group="shipment") == []

    api.delete(mapping.id)
    assert api.get(mapping.id) is None


def test_invalid_transform_rejected(commerce):
    api = commerce.integration_field_mappings
    if not api.is_supported():
        pytest.skip("integration field mappings not supported on this engine build")
    with pytest.raises(ValueError):
        api.create(
            integration_account="acct-1",
            mapping_group="order",
            source_field="a",
            destination_field="b",
            transform="not_a_transform",
        )
