"""Read APIs that used to be write-only from Python.

* refunds: ``get_refund`` / ``get_refunds`` and the pending -> completed | failed
  lifecycle (``complete_refund`` / ``fail_refund``), with the payment's
  ``amount_refunded`` advancing only when a refund settles;
* the promotion usage ledger (``promotions.list_usage``);
* ``orders.get_by_number`` and the order's tax / shipping / discount amounts.
"""

import uuid

import pytest

from stateset_embedded import Commerce, CreateOrderItemInput, CreateProductVariantInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def make_order(commerce, sku="READ-1"):
    customer = commerce.customers.create(
        email=f"read-{uuid.uuid4()}@example.com", first_name="Read", last_name="Api"
    )
    product = commerce.products.create(
        name=sku, variants=[CreateProductVariantInput(sku=sku, price=10.0, name=sku)]
    )
    commerce.products.update(product.id, status="active")
    order = commerce.orders.create(
        customer_id=customer.id,
        items=[
            CreateOrderItemInput(
                sku=sku, name=sku, quantity=2, unit_price=10.0, product_id=product.id
            )
        ],
    )
    return customer, order


def test_refunds_are_readable_and_follow_their_lifecycle(commerce):
    _, order = make_order(commerce)
    payment = commerce.payments.create_exact(amount="20.00", order_id=order.id)
    commerce.payments.complete(payment.id)
    assert payment.amount_refunded_exact == "0"

    first = commerce.payments.create_refund_exact(payment.id, "5.25", reason="damaged")
    assert first.status == "pending"
    assert first.currency == "USD"
    assert first.refunded_at is None
    assert commerce.payments.get(payment.id).amount_refunded_exact == "0"
    assert commerce.payments.get_refund(first.id).id == first.id
    assert [r.id for r in commerce.payments.get_refunds(payment.id)] == [first.id]

    done = commerce.payments.complete_refund(first.id)
    assert done.status == "completed"
    assert done.refunded_at is not None
    settled = commerce.payments.get(payment.id)
    assert settled.amount_refunded_exact == "5.25"
    assert settled.status == "partially_refunded"

    second = commerce.payments.create_refund_exact(payment.id, "1.00")
    failed = commerce.payments.fail_refund(second.id, "declined")
    assert failed.status == "failed"
    assert failed.failure_reason == "declined"
    assert commerce.payments.get(payment.id).amount_refunded_exact == "5.25"
    assert sorted(r.status for r in commerce.payments.get_refunds(payment.id)) == [
        "completed",
        "failed",
    ]
    assert commerce.payments.get_refund(str(uuid.uuid4())) is None


def test_list_usage_reads_the_ledger_by_order_and_promotion(commerce):
    alice, order_a = make_order(commerce, "LEDGER-A")
    bob, order_b = make_order(commerce, "LEDGER-B")
    ten = commerce.promotions.create(
        name="Ten", promotion_type="percentage_off", percentage_off=0.1
    )
    five = commerce.promotions.create(
        name="Five", promotion_type="percentage_off", percentage_off=0.05
    )
    commerce.promotions.record_usage(
        ten.id, 2.0, "USD", customer_id=alice.id, order_id=order_a.id
    )
    commerce.promotions.record_usage(
        five.id, 1.0, "USD", customer_id=alice.id, order_id=order_a.id
    )
    commerce.promotions.record_usage(
        ten.id, 3.0, "USD", customer_id=bob.id, order_id=order_b.id
    )

    assert len(commerce.promotions.list_usage()) == 3
    for_a = commerce.promotions.list_usage(order_id=order_a.id)
    assert sorted(u.promotion_id for u in for_a) == sorted([ten.id, five.id])
    assert sorted(u.discount_amount_exact for u in for_a) == ["1", "2"]
    ten_bob = commerce.promotions.list_usage(promotion_id=ten.id, customer_id=bob.id)
    assert [u.order_id for u in ten_bob] == [order_b.id]
    assert len(commerce.promotions.list_usage(limit=2)) == 2
    assert commerce.promotions.list_usage(order_id=str(uuid.uuid4())) == []
    with pytest.raises(ValueError):
        commerce.promotions.list_usage(order_id="not-a-uuid")


def test_get_by_number_and_order_money_breakdown(commerce):
    _, order = make_order(commerce, "BYNUM")
    found = commerce.orders.get_by_number(order.order_number)
    assert found is not None and found.id == order.id
    assert commerce.orders.get_by_number("ORD-NOPE") is None
    for field in ("tax_amount", "shipping_amount", "discount_amount"):
        assert isinstance(getattr(found, field), float)
        assert isinstance(getattr(found, f"{field}_exact"), str)
    assert found.shipping_amount_exact == "0"
    assert found.discount_amount_exact == "0"
