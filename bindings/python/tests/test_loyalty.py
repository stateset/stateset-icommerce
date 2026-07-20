"""Loyalty API tests for the stateset_embedded Python bindings.

Points are integers; reward `value` is an exact decimal string. Program tiers
round-trip (fixed in the engine alongside these bindings).
"""

import pytest
from stateset_embedded import Commerce, LoyaltyTierInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_loyalty_api_exists(commerce):
    assert commerce.loyalty is not None
    assert commerce.loyalty.is_supported() is True


def test_program_with_tiers_round_trip(commerce):
    program = commerce.loyalty.create_program(
        name="Rewards Club",
        points_per_dollar=2,
        description="Earn points on every order",
        tiers=[
            LoyaltyTierInput("Silver", 0, 1.0, ["free shipping"]),
            LoyaltyTierInput("Gold", 1000, 1.5, ["priority support", "early access"]),
        ],
    )
    assert program.name == "Rewards Club"
    assert program.points_per_dollar == 2
    assert len(program.tiers) == 2

    fetched = commerce.loyalty.get_program(program.id)
    assert len(fetched.tiers) == 2
    assert fetched.tiers[1].name == "Gold"
    assert fetched.tiers[1].min_points == 1000
    assert fetched.tiers[1].perks == ["priority support", "early access"]


def test_accounts_points_and_rewards(commerce):
    program = commerce.loyalty.create_program(name="Club", points_per_dollar=1)

    customer = commerce.customers.create(
        email="loyal@example.com", first_name="Loyal", last_name="Customer"
    )
    account = commerce.loyalty.enroll(customer.id, program.id)
    assert account.customer_id == customer.id
    assert account.points_balance == 0

    txn = commerce.loyalty.adjust_points(
        account.id, 150, "earn", reference_id="order-1", description="Purchase"
    )
    assert txn.points == 150
    assert txn.transaction_type == "earn"
    assert commerce.loyalty.get_account(account.id).points_balance == 150

    txns = commerce.loyalty.get_transactions(account.id)
    assert len(txns) >= 1

    reward = commerce.loyalty.create_reward(
        program.id, "$5 off", 100, "discount", value="5.00"
    )
    assert reward.points_cost == 100
    assert reward.reward_type == "discount"
    assert reward.value == "5.00"
    assert reward.is_active is True

    assert commerce.loyalty.get_reward(reward.id).id == reward.id
    assert any(r.id == reward.id for r in commerce.loyalty.list_rewards())

    commerce.loyalty.delete_reward(reward.id)
    assert commerce.loyalty.get_reward(reward.id) is None
