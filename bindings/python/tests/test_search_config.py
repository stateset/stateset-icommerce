"""Search configuration API tests for the stateset_embedded Python bindings.

Enums cross as snake_case strings; timestamps as RFC3339 strings.
"""

import pytest
from stateset_embedded import (
    BoostRuleInput,
    Commerce,
    FacetConfigInput,
    SearchFieldInput,
    SynonymGroupInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_search_config_api_exists(commerce):
    assert commerce.search_config is not None


def test_search_config_full_lifecycle(commerce):
    sc = commerce.search_config
    if not sc.is_supported():
        pytest.skip("search config backend not supported on this engine build")

    config = sc.create(
        name="Default",
        description="Primary catalog search",
        searchable_fields=[
            SearchFieldInput("title", 2.0, "standard", True),
            SearchFieldInput("sku", 1.0),
        ],
        facets=[FacetConfigInput("brand", "Brand", "value", 1, 10)],
        synonyms=[SynonymGroupInput("sneaker", ["trainer", "runner"])],
        boost_rules=[BoostRuleInput("brand", "acme", 1.5)],
    )
    assert config.id
    assert config.name == "Default"
    assert config.description == "Primary catalog search"
    assert len(config.searchable_fields) == 2
    assert config.searchable_fields[0].field_name == "title"
    assert config.searchable_fields[0].weight == 2.0
    assert config.searchable_fields[0].tokenizer == "standard"
    assert config.searchable_fields[0].enabled is True
    assert config.facets[0].facet_type == "value"
    assert config.facets[0].display_name == "Brand"
    assert config.facets[0].max_values == 10
    assert config.synonyms[0].canonical == "sneaker"
    assert config.synonyms[0].synonyms == ["trainer", "runner"]
    assert config.boost_rules[0].boost_factor == 1.5
    assert config.created_at

    # get and list find the config
    found = sc.get(config.id)
    assert found is not None
    assert found.id == config.id
    assert any(c.id == config.id for c in sc.list(name="Default"))

    # update changes the name and rules
    updated = sc.update(
        config.id,
        name="Default v2",
        boost_rules=[BoostRuleInput("brand", "acme", 2.5)],
    )
    assert updated.name == "Default v2"
    assert updated.boost_rules[0].boost_factor == 2.5

    # set_active / get_active
    active = sc.set_active(config.id)
    assert active.is_active is True
    current = sc.get_active()
    assert current is not None
    assert current.id == config.id

    # delete removes it
    sc.delete(config.id)
    assert sc.get(config.id) is None


def test_invalid_inputs_raise(commerce):
    sc = commerce.search_config
    if not sc.is_supported():
        pytest.skip("search config backend not supported on this engine build")

    with pytest.raises(ValueError):
        sc.get("not-a-uuid")
    with pytest.raises(ValueError):
        sc.create(
            name="Bad",
            searchable_fields=[SearchFieldInput("title", 1.0, "not_a_tokenizer")],
        )
    with pytest.raises(ValueError):
        sc.create(
            name="Bad",
            facets=[FacetConfigInput("brand", "Brand", "not_a_facet_type")],
        )
