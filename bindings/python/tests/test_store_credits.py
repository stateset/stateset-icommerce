"""Store Credits API tests for the stateset_embedded Python bindings.

Money is exchanged as exact decimal strings (no float precision loss).
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_store_credits_api_exists(commerce):
    assert commerce.store_credits is not None
    assert commerce.store_credits.is_supported() is True


def test_store_credit_full_lifecycle(commerce):
    sc = commerce.store_credits
    customer = commerce.customers.create(
        email="credit@example.com", first_name="Cred", last_name="Holder"
    )

    # issue — exact-string balances
    credit = sc.create(
        customer_id=customer.id,
        amount="25.00",
        currency="USD",
        reason="compensation",
        note="goodwill",
    )
    assert credit.customer_id == customer.id
    assert credit.original_balance == "25.00"
    assert credit.current_balance == "25.00"
    assert credit.currency == "USD"
    assert credit.status == "active"
    assert credit.reason == "compensation"
    assert credit.note == "goodwill"
    assert credit.id

    # get by id
    found = sc.get(credit.id)
    assert found is not None
    assert found.id == credit.id

    # apply — recorded as a negative debit, exact decimal arithmetic
    txn = sc.apply(credit.id, "10.00", "order-42")
    assert txn.amount == "-10.00"
    assert txn.balance_after == "15.00"
    assert txn.reference_id == "order-42"
    assert sc.get(credit.id).current_balance == "15.00"

    # adjust up
    adjusted = sc.adjust(credit.id, "5.00", note="top-up")
    assert adjusted.current_balance == "20.00"

    # transactions recorded
    txns = sc.get_transactions(credit.id)
    assert len(txns) >= 2

    # list by customer
    listed = sc.list(customer_id=customer.id)
    assert any(c.id == credit.id for c in listed)


def test_adjust_cannot_go_negative(commerce):
    sc = commerce.store_credits
    customer = commerce.customers.create(
        email="neg@example.com", first_name="Neg", last_name="Test"
    )
    credit = sc.create(customer_id=customer.id, amount="20.00", currency="USD")

    with pytest.raises(Exception, match="(?i)negative balance"):
        sc.adjust(credit.id, "-30.00")

    # balance unchanged after the rejected adjustment
    assert sc.get(credit.id).current_balance == "20.00"
