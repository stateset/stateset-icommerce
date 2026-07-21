"""Integration Mappings API tests for the stateset_embedded Python bindings.

Mappings translate an external system value into the canonical internal value
for a given integration, mapping group and field name.
"""

import pytest
from stateset_embedded import Commerce, CreateIntegrationMappingInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_integration_mapping_full_lifecycle(commerce):
    api = commerce.integration_mappings
    if not api.is_supported():
        pytest.skip("integration mappings backend not available on this build")

    mapping = api.create(
        integration="shopify",
        mapping_group="carrier",
        field_name="carrier_code",
        external_value="USPS_PRIORITY",
        internal_value="usps",
    )
    assert mapping.id
    assert mapping.integration == "shopify"
    assert mapping.mapping_group == "carrier"
    assert mapping.field_name == "carrier_code"
    assert mapping.external_value == "USPS_PRIORITY"
    assert mapping.internal_value == "usps"
    assert mapping.is_active is True

    # get and list find the mapping
    found = api.get(mapping.id)
    assert found is not None
    assert found.id == mapping.id
    listed = api.list(integration="shopify", mapping_group="carrier")
    assert any(m.id == mapping.id for m in listed)

    # resolve returns the internal value
    assert (
        api.resolve(
            integration="shopify",
            mapping_group="carrier",
            field_name="carrier_code",
            external_value="USPS_PRIORITY",
        )
        == "usps"
    )
    assert (
        api.resolve(
            integration="shopify",
            mapping_group="carrier",
            field_name="carrier_code",
            external_value="UNKNOWN",
        )
        is None
    )

    # update changes the internal value
    updated = api.update(mapping.id, internal_value="usps_priority")
    assert updated.internal_value == "usps_priority"

    # bulk_upsert reports affected rows as a string
    affected = api.bulk_upsert(
        [
            CreateIntegrationMappingInput(
                "shopify", "carrier", "carrier_code", "UPS_GROUND", "ups"
            ),
            CreateIntegrationMappingInput(
                "shopify", "carrier", "carrier_code", "FEDEX_2DAY", "fedex"
            ),
        ]
    )
    assert isinstance(affected, str)
    assert int(affected) >= 2
    assert (
        api.resolve(
            integration="shopify",
            mapping_group="carrier",
            field_name="carrier_code",
            external_value="UPS_GROUND",
        )
        == "ups"
    )

    # delete removes the mapping
    api.delete(mapping.id)
    assert api.get(mapping.id) is None
