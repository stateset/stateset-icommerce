"""Line items on the Python create paths.

`Invoices.create` and `PurchaseOrders.create` used to hardcode an empty item
list and expose no way to pass one, while the Node binding took `items` on
both. The engine inserts each line and then recalculates, so a Python-created
invoice always came back with subtotal, tax and total of zero — and
`record_payment` then refused every payment, because the balance due was zero.

These tests assert the totals the engine derives from the items, so they fail
against the old header-only surface.
"""

from decimal import Decimal

import pytest

from stateset_embedded import (
    Commerce,
    CreateInvoiceItemInput,
    CreatePurchaseOrderItemInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def a_customer(commerce):
    return commerce.customers.create(
        email="lines@example.com", first_name="Line", last_name="Items"
    )


def test_invoice_totals_come_from_its_line_items(commerce):
    customer = a_customer(commerce)
    invoice = commerce.invoices.create(
        customer.id,
        items=[
            CreateInvoiceItemInput("Widget", 2.0, 19.99),
            CreateInvoiceItemInput("Gasket", 1.0, 0.07),
        ],
    )

    # 2 x 19.99 + 1 x 0.07 = 40.05, exactly.
    assert Decimal(invoice.subtotal_exact) == Decimal("40.05")
    assert Decimal(invoice.total_exact) == Decimal("40.05")
    assert Decimal(invoice.total_exact) > 0, "a header-only invoice totalled zero"


def test_an_invoice_with_items_can_actually_be_paid(commerce):
    customer = a_customer(commerce)
    invoice = commerce.invoices.create(
        customer.id, items=[CreateInvoiceItemInput("Widget", 1.0, 19.99)]
    )

    # This is the failure the empty item list caused: balance due was 0, so
    # every positive payment was refused as exceeding it.
    paid = commerce.invoices.record_payment(invoice.id, 19.99)
    assert Decimal(paid.amount_paid_exact) == Decimal("19.99")
    assert Decimal(paid.amount_paid_exact) == Decimal(paid.total_exact), "paid in full"


def test_invoice_line_item_optional_fields_are_carried(commerce):
    customer = a_customer(commerce)
    invoice = commerce.invoices.create(
        customer.id,
        items=[
            CreateInvoiceItemInput(
                "Widget", 1.0, 10.00, sku="SKU-1", unit_of_measure="ea", tax_amount=0.83
            )
        ],
    )
    # The engine sums each line's `line_total`, which already includes that
    # line's tax, so 10.00 + 0.83 reaches both subtotal and total. The
    # invoice-level tax_amount is a header field taken from the engine's own
    # input rather than a sum of the lines, so it stays zero.
    assert Decimal(invoice.total_exact) == Decimal("10.83")
    assert Decimal(invoice.subtotal_exact) == Decimal("10.83")
    assert Decimal(invoice.tax_amount_exact) == Decimal("0")


def test_creating_an_invoice_without_items_still_works(commerce):
    customer = a_customer(commerce)
    invoice = commerce.invoices.create(customer.id)
    assert Decimal(invoice.total_exact) == Decimal("0")


def test_a_malformed_line_item_is_refused_and_nothing_is_written(commerce):
    customer = a_customer(commerce)
    before = len(commerce.invoices.list())
    with pytest.raises(ValueError):
        commerce.invoices.create(
            customer.id,
            items=[
                CreateInvoiceItemInput(
                    "Widget", 1.0, 10.00, product_id="not-a-uuid"
                )
            ],
        )
    assert len(commerce.invoices.list()) == before


def test_purchase_order_carries_its_line_items(commerce):
    supplier = commerce.purchase_orders.create_supplier("Acme", "acme@example.com")
    po = commerce.purchase_orders.create(
        supplier.id,
        items=[CreatePurchaseOrderItemInput("SKU-1", "Widget", 3.0, 4.50)],
    )
    # 3 x 4.50 = 13.50. A header-only purchase order totalled zero.
    assert Decimal(po.total_amount_exact) == Decimal("13.50")


def test_creating_a_purchase_order_without_items_still_works(commerce):
    supplier = commerce.purchase_orders.create_supplier("Acme", "acme@example.com")
    po = commerce.purchase_orders.create(supplier.id)
    assert Decimal(po.total_amount_exact) == Decimal("0")
