"""Promotion targeting from Python.

`Promotions.apply` hardcoded an empty line-item list, no customer, no cart and
no shipping destination, and `Promotions.create` could set neither conditions
nor product/SKU/category/customer scoping. So from Python only order-wide,
unconditional promotions could exist, and any scoped promotion created
elsewhere never matched a Python quote. `add_condition` did not exist, and the
engine's version stored nothing.
"""

from decimal import Decimal

import pytest

from stateset_embedded import (
    Commerce,
    CreateOrderItemInput,
    PromotionConditionInput,
    PromotionLineItemInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def line(sku, quantity, unit_price):
    return PromotionLineItemInput(
        f"line-{sku}", quantity, unit_price, quantity * unit_price, sku=sku
    )


def discount(result):
    return Decimal(result.total_discount_exact)


def activate(commerce, promotion):
    commerce.promotions.activate(promotion.id)
    return promotion


def test_a_sku_scoped_promotion_discounts_only_its_lines(commerce):
    activate(
        commerce,
        commerce.promotions.create(
            "Socks 20%",
            promotion_type="percentage_off",
            trigger="automatic",
            percentage_off=0.20,
            applicable_skus=["SOCKS"],
        ),
    )

    result = commerce.promotions.apply(
        60.0, line_items=[line("SOCKS", 2, 10.0), line("SHOES", 1, 40.0)]
    )
    # 20% of the $20 of socks, not of the $60 cart.
    assert discount(result) == Decimal("4.00")

    no_socks = commerce.promotions.apply(40.0, line_items=[line("SHOES", 1, 40.0)])
    assert discount(no_socks) == 0


def test_conditions_are_created_returned_and_enforced(commerce):
    promotion = activate(
        commerce,
        commerce.promotions.create(
            "Big cart 10%",
            promotion_type="percentage_off",
            trigger="automatic",
            percentage_off=0.10,
            conditions=[
                PromotionConditionInput("minimum_subtotal", "greater_than_or_equal", "50")
            ],
        ),
    )
    assert [(c.condition_type, c.operator, c.value) for c in promotion.conditions] == [
        ("minimum_subtotal", "greater_than_or_equal", "50")
    ]

    assert discount(commerce.promotions.apply(40.0, line_items=[line("A", 1, 40.0)])) == 0
    assert discount(
        commerce.promotions.apply(60.0, line_items=[line("A", 1, 60.0)])
    ) == Decimal("6.00")


def test_add_condition_is_stored(commerce):
    promotion = activate(
        commerce,
        commerce.promotions.create(
            "US only", promotion_type="percentage_off", trigger="automatic", percentage_off=0.10
        ),
    )
    updated = commerce.promotions.add_condition(
        promotion.id, PromotionConditionInput("shipping_country", "equals", "US")
    )
    assert len(updated.conditions) == 1
    assert len(commerce.promotions.get(promotion.id).conditions) == 1

    items = [line("A", 1, 100.0)]
    us = commerce.promotions.apply(100.0, line_items=items, shipping_country="US")
    ca = commerce.promotions.apply(100.0, line_items=items, shipping_country="CA")
    assert discount(us) == Decimal("10.00")
    assert discount(ca) == 0


def test_a_malformed_condition_is_a_value_error_and_stores_nothing(commerce):
    promotion = commerce.promotions.create("Promo", percentage_off=0.10)
    with pytest.raises(ValueError, match="banana"):
        commerce.promotions.add_condition(
            promotion.id, PromotionConditionInput("first_order", "equals", "banana")
        )
    with pytest.raises(ValueError, match="Unknown promotion condition type"):
        commerce.promotions.add_condition(
            promotion.id, PromotionConditionInput("first_ordr", "equals", "true")
        )
    with pytest.raises(ValueError, match="fifty"):
        commerce.promotions.create(
            "Broken",
            percentage_off=0.10,
            conditions=[PromotionConditionInput("minimum_subtotal", "greater_than", "fifty")],
        )
    assert commerce.promotions.get(promotion.id).conditions == []


def test_a_first_order_discount_follows_the_customers_order_history(commerce):
    activate(
        commerce,
        commerce.promotions.create(
            "Welcome", promotion_type="first_order_discount", trigger="automatic",
            percentage_off=0.10,
        ),
    )
    customer = commerce.customers.create(
        email="welcome@example.com", first_name="New", last_name="Customer"
    )
    items = [line("A", 1, 100.0)]

    first = commerce.promotions.apply(100.0, line_items=items, customer_id=customer.id)
    assert discount(first) == Decimal("10.00")

    commerce.orders.create(customer.id, [CreateOrderItemInput("A", "Widget", 1, 100.0)])
    second = commerce.promotions.apply(100.0, line_items=items, customer_id=customer.id)
    assert discount(second) == 0, "a returning customer's order is not a first order"


def test_currency_defaults_to_the_store_base_currency(commerce):
    # Discounts round to the request currency's minor unit. The quote used to
    # assume USD, so a yen store was quoted fractions of a yen.
    commerce.currency.set_base_currency("JPY")
    activate(
        commerce,
        commerce.promotions.create(
            "Ten percent", promotion_type="percentage_off", trigger="automatic",
            percentage_off=0.10,
        ),
    )
    result = commerce.promotions.apply(1234.0, line_items=[line("A", 1, 1234.0)])
    assert result.total_discount_exact == "123"


def test_malformed_line_item_ids_are_refused(commerce):
    with pytest.raises(ValueError, match="product_id"):
        commerce.promotions.apply(
            10.0,
            line_items=[PromotionLineItemInput("l1", 1, 10.0, 10.0, product_id="nope")],
        )
