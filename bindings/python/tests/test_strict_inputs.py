"""Strict-input tests: malformed arguments are refused, never silently coerced.

Every case here used to be accepted by the binding and quietly turned into
something else — a mistyped product id became the nil UUID and the order line
pointed at nothing, an unknown currency priced the row in the store default, a
malformed date filter became "no filter", and an unknown enum value became
whichever variant the parser happened to default to. The binding now raises
``ValueError`` naming the field instead, and writes nothing.
"""

import uuid

import pytest
from stateset_embedded import (
    Commerce,
    CreateOrderItemInput,
    CreateProductVariantInput,
    CreateReturnItemInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


@pytest.fixture
def customer(commerce):
    return commerce.customers.create(
        email="strict@example.com", first_name="Strict", last_name="Inputs"
    )


@pytest.fixture
def product(commerce):
    created = commerce.products.create(
        name="Strict Widget",
        variants=[CreateProductVariantInput(sku="STRICT-1", price=10.0, name="Strict Widget")],
    )
    return commerce.products.update(created.id, status="active")


def new_id():
    return str(uuid.uuid4())


def assert_names(excinfo, field):
    assert field in str(excinfo.value), f"error does not name {field!r}: {excinfo.value}"


# ---------------------------------------------------------------------------
# Currencies
# ---------------------------------------------------------------------------


def test_unknown_currency_is_refused_on_order_create(commerce, customer, product):
    with pytest.raises(ValueError) as excinfo:
        commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="STRICT-1",
                    name="Strict Widget",
                    quantity=1,
                    unit_price=10.0,
                    product_id=product.id,
                )
            ],
            currency="DOLLARS",
        )
    assert_names(excinfo, "currency")
    assert commerce.orders.count() == 0


def test_unknown_currency_is_refused_on_cart_and_payment(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.carts.create(currency="US$")
    assert_names(excinfo, "currency")

    with pytest.raises(ValueError) as excinfo:
        commerce.payments.create(amount=10.0, currency="DOLLARS")
    assert_names(excinfo, "currency")


def test_unknown_currency_is_refused_on_plan_and_promotion_apply(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.subscriptions.create_plan(
            code="PLAN-1", name="Plan", price=9.0, currency="DOLLARS"
        )
    assert_names(excinfo, "currency")

    with pytest.raises(ValueError) as excinfo:
        commerce.promotions.apply(subtotal=10.0, currency="DOLLARS")
    assert_names(excinfo, "currency")

    with pytest.raises(ValueError) as excinfo:
        commerce.promotions.create(name="Promo", currency="DOLLARS")
    assert_names(excinfo, "currency")
    assert commerce.promotions.list() == []


def test_mixed_case_currency_is_still_accepted(commerce, customer, product):
    order = commerce.orders.create(
        customer_id=customer.id,
        items=[
            CreateOrderItemInput(
                sku="STRICT-1",
                name="Strict Widget",
                quantity=1,
                unit_price=10.0,
                product_id=product.id,
            )
        ],
        currency="usd",
    )
    assert order.currency == "USD"


# ---------------------------------------------------------------------------
# Ids
# ---------------------------------------------------------------------------


def test_non_uuid_product_id_refuses_the_order(commerce, customer):
    """The nil-UUID bug: a mistyped product id used to be written as an order line."""
    with pytest.raises(ValueError) as excinfo:
        commerce.orders.create(
            customer_id=customer.id,
            items=[
                CreateOrderItemInput(
                    sku="STRICT-1",
                    name="Strict Widget",
                    quantity=1,
                    unit_price=10.0,
                    product_id="product-42",
                )
            ],
        )
    assert_names(excinfo, "product_id")
    assert commerce.orders.count() == 0
    assert commerce.orders.list() == []


def test_non_uuid_return_item_id_refuses_the_return(commerce, customer, product):
    order = commerce.orders.create(
        customer_id=customer.id,
        items=[
            CreateOrderItemInput(
                sku="STRICT-1",
                name="Strict Widget",
                quantity=1,
                unit_price=10.0,
                product_id=product.id,
            )
        ],
    )

    with pytest.raises(ValueError) as excinfo:
        commerce.returns.create(
            order_id=order.id,
            reason="defective",
            items=[CreateReturnItemInput("line-1", 1)],
        )
    assert_names(excinfo, "order_item_id")
    assert commerce.returns.list() == []


def test_non_uuid_optional_ids_are_refused(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.receiving.create_receipt(warehouse_id="1", supplier_id="supplier-7")
    assert_names(excinfo, "supplier_id")

    with pytest.raises(ValueError) as excinfo:
        commerce.promotions.list_coupons(promotion_id="promo-7")
    assert_names(excinfo, "promotion_id")

    with pytest.raises(ValueError) as excinfo:
        commerce.tax.list_rates(jurisdiction_id="jurisdiction-7")
    assert_names(excinfo, "jurisdiction_id")

    with pytest.raises(ValueError) as excinfo:
        commerce.promotions.record_usage(
            promotion_id=new_id(),
            discount_amount=1.0,
            currency="USD",
            customer_id="customer-9",
        )
    assert_names(excinfo, "customer_id")


# ---------------------------------------------------------------------------
# Id lists — one bad entry refuses the whole list
# ---------------------------------------------------------------------------


def test_one_bad_id_refuses_the_whole_exemption_list(commerce, customer):
    with pytest.raises(ValueError) as excinfo:
        commerce.tax.create_exemption(
            customer_id=customer.id,
            exemption_type="resale",
            effective_from="2026-01-01",
            jurisdiction_ids=[new_id(), "jurisdiction-2", new_id()],
        )
    assert_names(excinfo, "jurisdiction_ids[1]")
    assert commerce.tax.get_customer_exemptions(customer.id) == []


def test_one_bad_id_refuses_the_whole_wave(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.fulfillment.create_wave(warehouse_id="1", order_ids=[new_id(), "order-2"])
    assert_names(excinfo, "order_ids[1]")
    assert commerce.fulfillment.list_waves() == []


# ---------------------------------------------------------------------------
# Timestamps and dates
# ---------------------------------------------------------------------------


def test_malformed_rfc3339_timestamp_is_refused(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.promotions.create(name="Promo", starts_at="2026-09-21 12:00")
    assert_names(excinfo, "starts_at")
    assert commerce.promotions.list() == []

    with pytest.raises(ValueError) as excinfo:
        commerce.lots.create_lot(
            sku="STRICT-1", lot_number="LOT-1", quantity=1.0, expiration_date="2026-13-45T00:00:00Z"
        )
    assert_names(excinfo, "expiration_date")


def test_well_formed_rfc3339_timestamp_is_still_accepted(commerce):
    promo = commerce.promotions.create(
        name="Promo", starts_at="2026-09-21T12:00:00Z", ends_at="2026-09-30T12:00:00Z"
    )
    assert promo.id is not None


def test_malformed_iso_date_is_refused(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.general_ledger.get_trial_balance(as_of_date="21/09/2026")
    assert_names(excinfo, "as_of_date")

    jurisdiction = commerce.tax.create_jurisdiction(
        name="United States", code="US-STRICT", country_code="US"
    )
    with pytest.raises(ValueError) as excinfo:
        commerce.tax.create_rate(
            jurisdiction_id=jurisdiction.id,
            rate=0.07,
            name="Rate",
            effective_from="2026-01-01",
            effective_to="2026-02-31",
        )
    assert_names(excinfo, "effective_to")


def test_well_formed_iso_date_is_still_accepted(commerce):
    balance = commerce.general_ledger.get_trial_balance(as_of_date="2026-09-21")
    assert balance is not None


# ---------------------------------------------------------------------------
# JSON
# ---------------------------------------------------------------------------


def test_malformed_json_is_refused(commerce):
    with pytest.raises(ValueError) as excinfo:
        commerce.execute_kernel_command("{not json", "{}")
    assert "kernel command" in str(excinfo.value)


# ---------------------------------------------------------------------------
# Enum families — one unknown value each
# ---------------------------------------------------------------------------


def _return_with_reason(commerce, reason):
    commerce.returns.create(order_id=new_id(), reason=reason, items=[])


ENUM_CASES = [
    ("return reason", lambda c: _return_with_reason(c, "teleported"), "return reason"),
    (
        "payment method",
        lambda c: c.payments.create(amount=1.0, payment_method="wampum"),
        "payment_method",
    ),
    (
        "payment method (exact)",
        lambda c: c.payments.create_exact(amount="1.00", payment_method="wampum"),
        "payment_method",
    ),
    (
        "shipping carrier",
        lambda c: c.shipments.create(
            order_id=new_id(), recipient_name="R", shipping_address="A", carrier="pigeon"
        ),
        "carrier",
    ),
    (
        "shipping method",
        lambda c: c.shipments.create(
            order_id=new_id(), recipient_name="R", shipping_address="A", shipping_method="teleport"
        ),
        "shipping_method",
    ),
    (
        "claim resolution",
        lambda c: c.warranties.complete_claim(new_id(), "shrug"),
        "resolution",
    ),
    (
        "analytics period",
        lambda c: c.analytics.sales_summary(period="fortnight"),
        "period",
    ),
    (
        "analytics granularity",
        lambda c: c.analytics.revenue_by_period(granularity="fortnightly"),
        "granularity",
    ),
    (
        "forecast granularity",
        lambda c: c.analytics.revenue_forecast(granularity="fortnightly"),
        "granularity",
    ),
    (
        "rounding mode",
        lambda c: c.currency.update_settings(
            base_currency="USD", enabled_currencies=["USD"], rounding_mode="sideways"
        ),
        "rounding_mode",
    ),
    ("plan status", lambda c: c.subscriptions.list_plans(status="halfway"), "status"),
    ("subscription status", lambda c: c.subscriptions.list(status="halfway"), "status"),
    (
        "billing cycle status",
        lambda c: c.subscriptions.list_billing_cycles(status="halfway"),
        "status",
    ),
    (
        "promotion type",
        lambda c: c.promotions.create(name="P", promotion_type="mystery"),
        "promotion_type",
    ),
    ("promotion trigger", lambda c: c.promotions.create(name="P", trigger="mystery"), "trigger"),
    ("promotion target", lambda c: c.promotions.create(name="P", target="mystery"), "target"),
    ("stacking behavior", lambda c: c.promotions.create(name="P", stacking="mystery"), "stacking"),
    ("promotion status", lambda c: c.promotions.list(status="mystery"), "status"),
    ("coupon status", lambda c: c.promotions.list_coupons(status="mystery"), "status"),
    ("tax type", lambda c: c.tax.list_rates(tax_type="mystery"), "tax_type"),
    (
        "product tax category",
        lambda c: c.tax.list_rates(product_category="mystery"),
        "product_category",
    ),
    (
        "jurisdiction level",
        lambda c: c.tax.create_jurisdiction(
            name="J", code="J-1", country_code="US", level="planet"
        ),
        "level",
    ),
    (
        "exemption type",
        lambda c: c.tax.create_exemption(
            customer_id=new_id(), exemption_type="mystery", effective_from="2026-01-01"
        ),
        "exemption_type",
    ),
    (
        "inspection type",
        lambda c: c.quality.create_inspection(
            reference_type="receipt", reference_id=new_id(), inspection_type="mystery"
        ),
        "inspection_type",
    ),
    (
        "non-conformance source",
        lambda c: c.quality.create_ncr(
            sku="STRICT-1", description="D", quantity_affected=1, source="mystery", severity="major"
        ),
        "source",
    ),
    (
        "severity",
        lambda c: c.quality.create_ncr(
            sku="STRICT-1",
            description="D",
            quantity_affected=1,
            source="inspection",
            severity="mystery",
        ),
        "severity",
    ),
    ("serial status", lambda c: c.serials.change_status(new_id(), "mystery"), "status"),
    (
        "location type",
        lambda c: c.warehouse.create_location(
            warehouse_id="1", code="A-1", location_type="mystery"
        ),
        "location_type",
    ),
    (
        "AP payment method",
        lambda c: c.accounts_payable.pay_bill(new_id(), 1.0, payment_method="wampum"),
        "payment_method",
    ),
    (
        "credit memo reason",
        lambda c: c.accounts_receivable.create_credit_memo(new_id(), 1.0, "mystery"),
        "reason",
    ),
    (
        "cost method",
        lambda c: c.cost_accounting.get_inventory_valuation(cost_method="mystery"),
        "cost_method",
    ),
    (
        "backorder priority",
        lambda c: c.backorder.create_backorder(
            order_id=new_id(), customer_id=new_id(), sku="STRICT-1", quantity=1, priority="whenever"
        ),
        "priority",
    ),
    (
        "work order priority",
        lambda c: c.work_orders.create(
            product_id=new_id(), quantity_to_build=1, priority="whenever"
        ),
        "priority",
    ),
    (
        "GL account type",
        lambda c: c.general_ledger.create_account(
            account_number="1000", name="Cash", account_type="mystery"
        ),
        "account_type",
    ),
]


@pytest.mark.parametrize(
    "call,field", [pytest.param(c, f, id=name) for name, c, f in ENUM_CASES]
)
def test_unknown_enum_value_is_refused(commerce, call, field):
    with pytest.raises(ValueError) as excinfo:
        call(commerce)
    message = str(excinfo.value)
    assert field in message, f"error does not name {field!r}: {message}"
    assert "expected one of" in message, f"error does not list the accepted values: {message}"


MIXED_CASE_CASES = [
    ("promotion status filter", lambda c: c.promotions.list(status="ACTIVE")),
    ("plan status filter", lambda c: c.subscriptions.list_plans(status="Active")),
    ("subscription status filter", lambda c: c.subscriptions.list(status="Past_Due")),
    ("coupon status filter", lambda c: c.promotions.list_coupons(status="Active")),
    ("analytics period", lambda c: c.analytics.sales_summary(period="LAST30DAYS")),
    ("analytics granularity", lambda c: c.analytics.revenue_by_period(granularity="Monthly")),
    ("tax type filter", lambda c: c.tax.list_rates(tax_type="Sales_Tax")),
    ("jurisdiction level filter", lambda c: c.tax.list_jurisdictions(level="Country")),
    ("cost method", lambda c: c.cost_accounting.get_inventory_valuation(cost_method="FIFO")),
    (
        "GL account type",
        lambda c: c.general_ledger.create_account(
            account_number="1000", name="Cash", account_type="Asset"
        ),
    ),
]


@pytest.mark.parametrize("call", [pytest.param(c, id=name) for name, c in MIXED_CASE_CASES])
def test_known_enum_value_in_mixed_case_is_still_accepted(commerce, call):
    call(commerce)
