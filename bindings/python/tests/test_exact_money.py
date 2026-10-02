"""Exact-money output tests for the stateset_embedded Python bindings.

The Rust core holds every monetary amount as a base-10 ``Decimal``. The binding
narrows it to ``float`` on the way out, and a ``float`` cannot represent most
decimal cents. Each money field therefore ships a ``*_exact`` companion that
carries the true decimal as a string.

These tests are deliberately built on amounts with NO exact binary
floating-point representation (``19.99``, ``0.1 + 0.2``, ``0.07``, ``0.0825``)
and they check BOTH halves of the contract:

* ``Decimal(obj.field_exact)`` is the true decimal, and
* ``Decimal(obj.field)`` — the float companion — is NOT, so the exact field is
  the only lossless way to read the value back out.
"""

from decimal import Decimal

import pytest

from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def is_lossy(literal: str) -> bool:
    """True when `literal` has no exact binary floating-point representation."""
    return Decimal(float(literal)) != Decimal(literal)


def assert_exact(exact, approx, literal: str) -> None:
    """The `_exact` string is `literal`; the float companion cannot be."""
    assert is_lossy(literal), f"{literal} is representable — pick a lossier amount"
    assert isinstance(exact, str)
    assert Decimal(exact) == Decimal(literal)
    assert Decimal(approx) != Decimal(literal)


# ---------------------------------------------------------------------------
# premise
# ---------------------------------------------------------------------------


def test_the_amounts_these_tests_use_are_not_representable_as_floats():
    """Without this the rest of the file would prove nothing."""
    for literal in ("19.99", "0.1", "0.2", "0.3", "0.07", "0.0825", "0.0725"):
        assert is_lossy(literal)
    # the textbook case the engine has to get right
    assert 0.1 + 0.2 != 0.3


# ---------------------------------------------------------------------------
# StockLevel — 0.1 + 0.2 must come back as 0.3
# ---------------------------------------------------------------------------


def test_stock_level_exact_survives_the_classic_float_sum(commerce):
    commerce.inventory.create_item(sku="EXACT-1", name="Widget")
    commerce.inventory.adjust(sku="EXACT-1", quantity=0.1, reason="receipt")
    commerce.inventory.adjust(sku="EXACT-1", quantity=0.2, reason="receipt")

    stock = commerce.inventory.get_stock("EXACT-1")

    # the engine added in Decimal, so the exact string is a clean 0.3 ...
    assert_exact(stock.total_on_hand_exact, stock.total_on_hand, "0.3")
    # ... while the float companion is the familiar 0.2999999999999999888...
    assert Decimal(stock.total_on_hand) < Decimal("0.3")

    assert Decimal(stock.total_allocated_exact) == Decimal("0")
    assert_exact(stock.total_available_exact, stock.total_available, "0.3")

    # and the float fields are untouched — still floats, still the same value
    assert isinstance(stock.total_on_hand, float)
    assert stock.total_on_hand == pytest.approx(0.3)


# ---------------------------------------------------------------------------
# ItemCost — six money fields, every one of them exact
# ---------------------------------------------------------------------------


def test_item_cost_exposes_every_cost_component_exactly(commerce):
    cost = commerce.cost_accounting.set_item_cost(
        sku="EXACT-1",
        standard_cost=19.99,
        material_cost=0.1,
        labor_cost=0.2,
        overhead_cost=0.07,
    )

    assert_exact(cost.standard_cost_exact, cost.standard_cost, "19.99")
    assert_exact(cost.material_cost_exact, cost.material_cost, "0.1")
    assert_exact(cost.labor_cost_exact, cost.labor_cost, "0.2")
    assert_exact(cost.overhead_cost_exact, cost.overhead_cost, "0.07")
    assert_exact(cost.average_cost_exact, cost.average_cost, "19.99")
    assert_exact(cost.last_cost_exact, cost.last_cost, "19.99")

    # material + labor + overhead adds up exactly in Decimal ...
    components = (
        Decimal(cost.material_cost_exact)
        + Decimal(cost.labor_cost_exact)
        + Decimal(cost.overhead_cost_exact)
    )
    assert components == Decimal("0.37")
    # ... but not from the floats
    assert (
        Decimal(cost.material_cost)
        + Decimal(cost.labor_cost)
        + Decimal(cost.overhead_cost)
    ) != Decimal("0.37")

    fetched = commerce.cost_accounting.get_item_cost("EXACT-1")
    assert fetched is not None
    assert fetched.standard_cost_exact == cost.standard_cost_exact


# ---------------------------------------------------------------------------
# ApplyPromotionsResult / AppliedPromotion — a whole settlement, exact
# ---------------------------------------------------------------------------


@pytest.fixture
def ten_percent_coupon(commerce):
    promo = commerce.promotions.create(
        name="Ten Percent",
        promotion_type="percentage_off",
        percentage_off=0.1,
    )
    commerce.promotions.activate(promo.id)
    commerce.promotions.create_coupon(promotion_id=promo.id, code="EXACT10")
    return promo


def test_apply_promotions_result_settles_exactly(commerce, ten_percent_coupon):
    result = commerce.promotions.apply(
        subtotal=19.99, coupon_codes=["EXACT10"], shipping_amount=0.07
    )

    assert_exact(result.original_subtotal_exact, result.original_subtotal, "19.99")
    assert_exact(result.discounted_subtotal_exact, result.discounted_subtotal, "17.99")
    assert_exact(result.grand_total_exact, result.grand_total, "18.06")
    # 10% of 19.99, rounded to the cent. The string keeps the Decimal's scale:
    # a companion derived from the f64 would have printed "2", not "2.00".
    assert result.total_discount_exact == "2.00"
    assert Decimal(result.shipping_discount_exact) == Decimal("0")

    # the ledger closes exactly in Decimal
    assert (
        Decimal(result.original_subtotal_exact) - Decimal(result.total_discount_exact)
        == Decimal(result.discounted_subtotal_exact)
    )
    assert (
        Decimal(result.discounted_subtotal_exact) + Decimal("0.07")
        == Decimal(result.grand_total_exact)
    )
    # the float grand total, read back exactly, is short of 18.06
    assert Decimal(result.grand_total) < Decimal("18.06")

    [applied] = result.applied_promotions
    assert applied.discount_amount_exact == "2.00"
    assert applied.discount_amount == 2.0
    assert applied.coupon_code == "EXACT10"


def test_promotion_rate_and_cap_carry_exact_companions(commerce):
    promo = commerce.promotions.create(
        name="Capped",
        promotion_type="percentage_off",
        percentage_off=0.1,
        max_discount_amount=5.07,
    )

    assert_exact(promo.percentage_off_exact, promo.percentage_off, "0.1")
    assert_exact(promo.max_discount_amount_exact, promo.max_discount_amount, "5.07")
    # an absent Option<Decimal> stays absent — Option<f64> -> Option<str>
    assert promo.fixed_amount_off is None
    assert promo.fixed_amount_off_exact is None

    fetched = commerce.promotions.get(promo.id)
    assert fetched.percentage_off_exact == "0.1"


def test_promotion_usage_discount_is_exact(commerce, ten_percent_coupon):
    usage = commerce.promotions.record_usage(
        promotion_id=ten_percent_coupon.id,
        discount_amount=1.999,
        currency="USD",
    )
    assert_exact(usage.discount_amount_exact, usage.discount_amount, "1.999")


# ---------------------------------------------------------------------------
# rate-bearing classes — a rate like 0.0825 is exactly what floats mangle
# ---------------------------------------------------------------------------


def test_tax_rate_is_exact(commerce):
    jurisdiction = commerce.tax.create_jurisdiction(
        name="Testville", code="US-EXACT", country_code="US"
    )
    rate = commerce.tax.create_rate(
        jurisdiction_id=jurisdiction.id,
        rate=0.0825,
        name="Combined",
        effective_from="2020-01-01",
    )

    assert_exact(rate.rate_exact, rate.rate, "0.0825")
    assert commerce.tax.get_rate(rate.id).rate_exact == "0.0825"


def test_us_state_tax_rate_from_the_engine_tables_is_exact(commerce):
    california = commerce.tax.get_us_state_info("CA")

    # sourced from the engine's own Decimal table — no float ever touched it
    assert_exact(california.state_rate_exact, california.state_rate, "0.0725")


def test_eu_vat_optional_rates_mirror_their_float_companions(commerce):
    germany = commerce.tax.get_eu_vat_info("DE")

    assert_exact(germany.standard_rate_exact, germany.standard_rate, "0.19")
    assert_exact(germany.reduced_rate_exact, germany.reduced_rate, "0.07")
    # Option<f64> -> Option<str>, and None stays None on both sides
    assert germany.super_reduced_rate is None
    assert germany.super_reduced_rate_exact is None
    assert germany.parking_rate is None
    assert germany.parking_rate_exact is None


def test_canadian_tax_rates_are_exact(commerce):
    bc = commerce.tax.get_canadian_tax_info("BC")

    assert_exact(bc.gst_rate_exact, bc.gst_rate, "0.05")
    assert_exact(bc.pst_rate_exact, bc.pst_rate, "0.07")
    assert bc.hst_rate is None
    assert bc.hst_rate_exact is None
    assert bc.qst_rate is None
    assert bc.qst_rate_exact is None
    # gst + pst, added in Decimal by the engine
    assert_exact(bc.total_rate_exact, bc.total_rate, "0.12")
    assert Decimal(bc.gst_rate_exact) + Decimal(bc.pst_rate_exact) == Decimal(
        bc.total_rate_exact
    )
    # the same sum over the float companions misses
    assert Decimal(bc.gst_rate) + Decimal(bc.pst_rate) != Decimal("0.12")


def test_exchange_rate_and_conversion_keep_full_precision(commerce):
    commerce.currency.set_rate(
        base_currency="USD", quote_currency="EUR", rate=0.9175
    )

    rate = commerce.currency.get_rate("USD", "EUR")
    assert_exact(rate.rate_exact, rate.rate, "0.9175")

    conversion = commerce.currency.convert(
        amount=19.99, from_currency="USD", to_currency="EUR"
    )
    assert_exact(
        conversion.original_amount_exact, conversion.original_amount, "19.99"
    )
    assert_exact(
        conversion.converted_amount_exact, conversion.converted_amount, "18.340825"
    )
    assert_exact(conversion.rate_exact, conversion.rate, "0.9175")

    # the inverse rate carries far more significant digits than an f64 holds
    inverse = Decimal(conversion.inverse_rate_exact)
    assert len(inverse.as_tuple().digits) > 17
    assert Decimal(conversion.inverse_rate) != inverse
    # and it really is the reciprocal, to the engine's full precision
    assert inverse * Decimal("0.9175") == Decimal(1)
    # the float companion, multiplied back, is not
    assert Decimal(conversion.inverse_rate) * Decimal("0.9175") != Decimal(1)


# ---------------------------------------------------------------------------
# credit — balances derived by the engine, not echoed back from the input
# ---------------------------------------------------------------------------


def test_credit_account_balances_are_exact(commerce):
    customer = commerce.customers.create(
        email="credit@example.com", first_name="Cred", last_name="Holder"
    )
    account = commerce.credit.create_credit_account(
        customer_id=customer.id, credit_limit=1000.07
    )

    assert_exact(account.credit_limit_exact, account.credit_limit, "1000.07")
    assert Decimal(account.current_balance_exact) == Decimal("0")
    # available = limit - balance, computed in Decimal by the engine
    assert_exact(account.available_credit_exact, account.available_credit, "1000.07")

    check = commerce.credit.check_credit(customer.id, 19.99)
    assert check.approved is True
    assert_exact(check.available_credit_exact, check.available_credit, "1000.07")


# ---------------------------------------------------------------------------
# Invoice
# ---------------------------------------------------------------------------


def test_invoice_exposes_exact_companions_for_all_four_money_fields(commerce):
    """Invoice.subtotal/tax_amount/total/amount_paid each gained an exact twin.

    `Invoices.create` does not accept line items today, so the reachable
    amounts are zero; what is asserted here is that every money field carries a
    decimal-string companion and that the four of them are mutually consistent
    in Decimal arithmetic.
    """
    customer = commerce.customers.create(
        email="invoice@example.com", first_name="Ivy", last_name="Noice"
    )
    invoice = commerce.invoices.create(customer_id=customer.id)

    for exact in (
        invoice.subtotal_exact,
        invoice.tax_amount_exact,
        invoice.total_exact,
        invoice.amount_paid_exact,
    ):
        assert isinstance(exact, str)
        Decimal(exact)  # parses as a decimal, not a float repr

    assert Decimal(invoice.subtotal_exact) + Decimal(
        invoice.tax_amount_exact
    ) == Decimal(invoice.total_exact)
    assert Decimal(invoice.total_exact) - Decimal(
        invoice.amount_paid_exact
    ) == Decimal(str(invoice.balance_due))

    # the pre-existing float fields are untouched
    assert isinstance(invoice.total, float)
    assert isinstance(invoice.amount_paid, float)

    fetched = commerce.invoices.get(invoice.id)
    assert fetched.total_exact == invoice.total_exact


def test_invoice_and_tax_calculation_declare_their_exact_attributes():
    """The exact companions are real attributes on the classes themselves.

    `TaxCalculationResult` is registered with the module but no binding method
    returns one yet, so this is the only way to pin its shape.
    """
    from stateset_embedded.stateset_embedded import Invoice, TaxCalculationResult

    for name in (
        "subtotal",
        "subtotal_exact",
        "tax_amount",
        "tax_amount_exact",
        "total",
        "total_exact",
        "amount_paid",
        "amount_paid_exact",
    ):
        assert hasattr(Invoice, name), f"Invoice.{name} is missing"

    for name in (
        "total_tax",
        "total_tax_exact",
        "subtotal",
        "subtotal_exact",
        "total",
        "total_exact",
        "shipping_tax",
        "shipping_tax_exact",
    ):
        assert hasattr(
            TaxCalculationResult, name
        ), f"TaxCalculationResult.{name} is missing"


# ---------------------------------------------------------------------------
# nothing that existed before changed shape
# ---------------------------------------------------------------------------


def test_float_fields_keep_their_names_and_types(commerce):
    """The exact fields are additive — every float companion is still a float."""
    commerce.inventory.create_item(sku="EXACT-2", name="Gizmo")
    commerce.inventory.adjust(sku="EXACT-2", quantity=0.07, reason="receipt")

    stock = commerce.inventory.get_stock("EXACT-2")
    assert isinstance(stock.total_on_hand, float)
    assert isinstance(stock.total_available, float)
    assert isinstance(stock.total_on_hand_exact, str)

    cost = commerce.cost_accounting.set_item_cost(sku="EXACT-2", standard_cost=0.07)
    assert isinstance(cost.standard_cost, float)
    assert isinstance(cost.standard_cost_exact, str)
    assert cost.standard_cost == pytest.approx(0.07)
    assert_exact(cost.standard_cost_exact, cost.standard_cost, "0.07")
