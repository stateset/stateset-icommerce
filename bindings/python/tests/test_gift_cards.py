"""Gift Cards API tests for the stateset_embedded Python bindings.

Money is exchanged as exact decimal strings (no float precision loss).
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_gift_cards_api_exists(commerce):
    assert commerce.gift_cards is not None
    assert commerce.gift_cards.is_supported() is True


def test_gift_card_full_lifecycle(commerce):
    gc = commerce.gift_cards

    card = gc.create(
        initial_balance="50.00",
        currency="USD",
        code="GIFT-PY-001",
        recipient_email="ada@example.com",
    )
    assert card.code == "GIFT-PY-001"
    assert card.initial_balance == "50.00"
    assert card.current_balance == "50.00"
    assert card.currency == "USD"
    assert card.status == "active"
    assert card.recipient_email == "ada@example.com"
    assert card.id

    # get_by_code
    found = gc.get_by_code("GIFT-PY-001")
    assert found is not None
    assert found.id == card.id

    # charge — exact decimal arithmetic
    txn = gc.charge(card.id, "19.99", "order-123")
    assert txn.amount == "19.99"
    assert txn.balance_after == "30.01"
    assert txn.reference_id == "order-123"
    assert gc.get(card.id).current_balance == "30.01"

    # refund
    txn2 = gc.refund(card.id, "5.00", "refund-1")
    assert txn2.balance_after == "35.01"
    assert gc.get(card.id).current_balance == "35.01"

    # transactions
    txns = gc.get_transactions(card.id)
    assert len(txns) >= 2

    # list
    cards = gc.list()
    assert any(c.id == card.id for c in cards)

    # disable
    disabled = gc.disable(card.id)
    assert disabled.status == "disabled"


def test_invalid_amount_raises(commerce):
    card = commerce.gift_cards.create(initial_balance="10.00", currency="USD")
    with pytest.raises(ValueError):
        commerce.gift_cards.charge(card.id, "not-a-number")
