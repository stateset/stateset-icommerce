"""Payment obligation API tests for the stateset_embedded Python bindings.

Money is exchanged as exact decimal strings (no float precision loss);
dates cross as ISO strings; enums as snake_case strings.
"""

import uuid

from decimal import Decimal

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_payment_obligation_lifecycle(commerce):
    api = commerce.payment_obligations
    if not api.is_supported():
        pytest.skip("payment obligations backend not available in this build")

    supplier_id = str(uuid.uuid4())
    obligation = api.create(
        supplier_id=supplier_id,
        amount="1000.00",
        due_date="2026-01-31",
        currency="USD",
        notes="Net 30",
    )
    assert obligation.id
    assert obligation.number
    assert obligation.supplier_id == supplier_id
    assert obligation.amount == "1000.00"
    assert obligation.outstanding == "1000.00"
    assert obligation.currency == "USD"
    assert obligation.due_date == "2026-01-31"
    assert obligation.status == "pending"
    assert obligation.linked_bill_ids == []
    assert obligation.notes == "Net 30"

    # get and list find the obligation
    found = api.get(obligation.id)
    assert found is not None
    assert found.id == obligation.id
    listed = api.list(supplier_id=supplier_id)
    assert any(o.id == obligation.id for o in listed)

    # status transitions
    scheduled = api.set_status(obligation.id, "scheduled")
    assert scheduled.status == "scheduled"

    # partial payment moves to partially_paid and reduces outstanding
    partial = api.record_payment(obligation.id, "400.00")
    assert partial.amount_paid == "400.00"
    assert partial.outstanding == "600.00"
    assert partial.status == "partially_paid"

    # linking an AP bill records the reference
    bill_id = str(uuid.uuid4())
    linked = api.link_bill(obligation.id, bill_id)
    assert bill_id in linked.linked_bill_ids

    # dashboard reports the obligation as open and (past due date) overdue
    dashboard = api.dashboard("2026-02-15")
    assert dashboard.open_count >= 1
    assert dashboard.overdue_count >= 1
    assert dashboard.total_outstanding == "600.00"
    assert dashboard.overdue_amount == "600.00"

    # paying the remainder closes it out
    paid = api.record_payment(obligation.id, "600.00")
    assert Decimal(paid.outstanding) == Decimal("0")
    assert paid.status == "paid"


def test_invalid_inputs_raise(commerce):
    api = commerce.payment_obligations
    if not api.is_supported():
        pytest.skip("payment obligations backend not available in this build")

    with pytest.raises(ValueError):
        api.get("not-a-uuid")
    with pytest.raises(ValueError):
        api.list(status="not_a_status")
    with pytest.raises(ValueError):
        api.create(
            supplier_id=str(uuid.uuid4()),
            amount="10.00",
            due_date="not-a-date",
        )
