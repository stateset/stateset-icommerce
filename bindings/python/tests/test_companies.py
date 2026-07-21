"""Companies (B2B accounts and contacts) API tests for the Python bindings.

Metadata crosses as a JSON string; timestamps as RFC 3339 strings; enums as
snake_case strings.
"""

import json

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_companies_full_lifecycle(commerce):
    api = commerce.companies
    if not api.is_supported():
        pytest.skip("companies backend is not supported on this engine build")

    company = api.create(
        name="Acme Industrial",
        reference="ACME-001",
        email="ap@acme.example",
        phone="+15555550100",
        currency="USD",
        payment_terms_days=30,
        tags=["wholesale"],
        metadata=json.dumps({"tier": "gold"}),
    )
    assert company.id
    assert company.name == "Acme Industrial"
    assert company.reference == "ACME-001"
    assert company.currency == "USD"
    assert company.payment_terms_days == 30
    assert company.status == "active"
    assert company.tags == ["wholesale"]
    assert json.loads(company.metadata)["tier"] == "gold"

    # get and list find the company
    found = api.get(company.id)
    assert found is not None
    assert found.id == company.id
    listed = api.list(status="active", search="Acme")
    assert any(c.id == company.id for c in listed)

    # update applies a partial change
    updated = api.update(company.id, name="Acme Industrial LLC", payment_terms_days=45)
    assert updated.name == "Acme Industrial LLC"
    assert updated.payment_terms_days == 45
    assert updated.status == "active"

    # a fresh company has no addresses or price overrides
    assert api.list_addresses(company.id) == []
    assert api.list_price_overrides(company.id) == []

    # contacts link to the company
    contact = api.create_contact(
        first_name="Ada",
        last_name="Lovelace",
        email="ada@acme.example",
        title="Buyer",
        company_ids=[company.id],
    )
    assert contact.id
    assert contact.first_name == "Ada"
    assert contact.company_ids == [company.id]
    assert contact.is_active is True
    assert contact.portal_enabled is False

    fetched = api.get_contact(contact.id)
    assert fetched is not None
    assert fetched.id == contact.id
    assert any(c.id == contact.id for c in api.list_contacts(company.id))

    # deactivate the company, then delete it
    inactive = api.update(company.id, status="inactive")
    assert inactive.status == "inactive"
    api.delete(company.id)
    assert api.get(company.id) is None


def test_companies_invalid_inputs_raise(commerce):
    api = commerce.companies
    if not api.is_supported():
        pytest.skip("companies backend is not supported on this engine build")

    with pytest.raises(ValueError):
        api.get("not-a-uuid")
    with pytest.raises(ValueError):
        api.list(status="bogus")
