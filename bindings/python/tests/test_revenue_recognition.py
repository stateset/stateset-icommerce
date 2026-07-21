"""Revenue Recognition (ASC 606) API tests for the Python bindings.

Money is exchanged as exact decimal strings; dates cross as ISO strings;
enums as snake_case strings.
"""

import pytest
from stateset_embedded import Commerce, PerformanceObligationInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_revenue_recognition_api_exists(commerce):
    assert commerce.revenue_recognition is not None
    assert commerce.revenue_recognition.is_supported() is True


def test_revenue_contract_full_lifecycle(commerce):
    rr = commerce.revenue_recognition
    customer = commerce.customers.create(
        email="rev@example.com", first_name="Rev", last_name="Customer"
    )

    contract = rr.create_contract(
        customer_id=customer.id,
        transaction_price="1200.00",
        effective_date="2026-01-01",
        obligations=[
            PerformanceObligationInput(
                description="Annual support",
                allocated_amount="1200.00",
                recognition_method="ratable_over_time",
                recognition_start="2026-01-01",
                recognition_end="2026-12-31",
            )
        ],
    )
    assert contract.id
    assert contract.contract_number.startswith("RC-")
    assert contract.transaction_price == "1200.00"
    assert len(contract.obligations) == 1
    assert contract.obligations[0].allocated_amount == "1200.00"
    assert contract.obligations[0].recognition_method == "ratable_over_time"
    assert contract.total_recognized == "0"
    assert contract.deferred_balance == "1200.00"

    # get_contract and list_contracts find the contract
    found = rr.get_contract(contract.id)
    assert found is not None
    assert found.id == contract.id
    listed = rr.list_contracts(customer_id=customer.id)
    assert any(c.id == contract.id for c in listed)

    # update_contract activates the contract
    updated = rr.update_contract(contract.id, status="active")
    assert updated.status == "active"

    # list_obligations returns the obligation
    obligations = rr.list_obligations(contract.id)
    assert len(obligations) == 1
    obligation = obligations[0]
    assert obligation.contract_id == contract.id
    assert obligation.deferred_amount == "1200.00"

    # generate_schedule builds a 12-month ratable schedule
    schedule = rr.generate_schedule(obligation.id)
    assert schedule.obligation_id == obligation.id
    assert schedule.method == "ratable_over_time"
    assert len(schedule.entries) == 12
    assert schedule.total_amount == "1200.00"
    assert schedule.entries[0].amount == "100.00"
    assert schedule.entries[0].status == "deferred"
    assert schedule.entries[0].period_start == "2026-01-01"
    assert schedule.deferred_total == "1200.00"

    persisted = rr.get_schedule(obligation.id)
    assert persisted is not None
    assert len(persisted.entries) == 12

    # recognize recognizes periods through a date
    schedule = rr.recognize(obligation.id, "2026-03-15")
    assert schedule.recognized_total == "300.00"
    assert schedule.deferred_total == "900.00"
    assert schedule.entries[0].status == "recognized"
    assert schedule.entries[2].status == "recognized"
    assert schedule.entries[3].status == "deferred"

    after = rr.get_contract(contract.id)
    assert after.total_recognized == "300.00"
    assert after.deferred_balance == "900.00"


def test_invalid_inputs_raise(commerce):
    rr = commerce.revenue_recognition
    with pytest.raises(ValueError):
        rr.get_contract("not-a-uuid")
    customer = commerce.customers.create(
        email="rev2@example.com", first_name="Rev", last_name="Two"
    )
    with pytest.raises(ValueError):
        # ratable_over_time requires recognition_start/recognition_end
        rr.create_contract(
            customer_id=customer.id,
            transaction_price="100.00",
            effective_date="2026-01-01",
            obligations=[
                PerformanceObligationInput(
                    description="Bad",
                    allocated_amount="100.00",
                    recognition_method="ratable_over_time",
                )
            ],
        )
    with pytest.raises(ValueError):
        rr.create_contract(
            customer_id=customer.id,
            transaction_price="100.00",
            effective_date="2026-01-01",
            obligations=[
                PerformanceObligationInput(
                    description="Bad",
                    allocated_amount="100.00",
                    recognition_method="not_a_method",
                )
            ],
        )
