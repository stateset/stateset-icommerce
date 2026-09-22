"""The Python binding against the shared semantic corpus.

`bindings/test-vectors/semantics-v1.json` pins the MEANING of money across
every binding: currency scale, the exact decimal a value renders to, the
published tax tables, and which inputs must be refused. The Rust test at
`crates/stateset-embedded/tests/semantics_vectors.rs` keeps that file honest
against the engine, so conforming to it means conforming to the engine.

This file asserts the categories the Python surface can reach, and declares
the ones it cannot. The declaration is enforced: adding a category to the
corpus, or exposing one of these on the binding, fails this test until
someone decides what to do about it. A silent skip would let the corpus grow
while coverage quietly stood still.
"""

import json
from decimal import Decimal
from pathlib import Path

import pytest

from stateset_embedded import Commerce, CreateOrderItemInput, TaxApi

CORPUS = Path(__file__).resolve().parents[2] / "test-vectors" / "semantics-v1.json"

# Categories this binding cannot assert yet, with the reason. Keep this honest:
# it is compared against the corpus, so it cannot drift out of date.
NOT_REACHABLE = {
    "currency_decimals": "the binding exposes no currency-scale accessor",
}


@pytest.fixture(scope="module")
def corpus():
    doc = json.loads(CORPUS.read_text())
    assert doc["version"] == 1, "corpus version must be 1"
    return doc


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def rows(corpus, category):
    return corpus["categories"][category]["rows"]


def test_every_corpus_category_is_either_asserted_or_declared_unreachable(corpus):
    """A new category must not slip in unnoticed."""
    asserted = {
        "rejected_inputs",
        "accepted_inputs",
        "canadian_tax_rates",
        "decimal_render",
        "money_scale_enforced",
    }
    declared = asserted | set(NOT_REACHABLE)
    present = set(corpus["categories"])
    assert present == declared, (
        f"corpus categories {sorted(present)} do not match what this binding "
        f"accounts for {sorted(declared)} — assert the new one or declare why not"
    )


def test_canadian_tax_rates_match_the_corpus(corpus):
    """The same table the Rust test pins, read through the Python surface.

    Quebec's rates were ten times too large until Sep 2026 and nothing pinned
    them. Reintroducing that bug fails this test in every binding at once.
    """
    for row in rows(corpus, "canadian_tax_rates"):
        info = TaxApi.get_canadian_tax_info(row["province"])
        assert info is not None, f"{row['province']} is in the table"
        for field in ("gst", "pst", "hst", "qst", "total"):
            want = row[field]
            got = getattr(info, f"{field}_rate" if field != "total" else "total_rate")
            if want is None:
                assert got is None, f"{row['id']}: {field} should be absent, got {got}"
            else:
                assert got is not None, f"{row['id']}: {field} missing"
                assert Decimal(str(got)) == Decimal(want), (
                    f"{row['id']}: {field} is {got}, corpus says {want}"
                )


def _customer(commerce):
    return commerce.customers.create(
        email="vectors@example.com", first_name="V", last_name="Ectors"
    )


def test_rejected_currencies_are_refused(commerce, corpus):
    customer = _customer(commerce)
    for row in rows(corpus, "rejected_inputs"):
        if row["kind"] != "currency":
            continue
        with pytest.raises(ValueError) as caught:
            commerce.orders.create(customer.id, [], currency=row["value"])
        assert "currency" in str(caught.value).lower(), (
            f"{row['id']}: the error should name the field, got {caught.value!r}"
        )


def test_rejected_uuids_are_refused(commerce, corpus):
    for row in rows(corpus, "rejected_inputs"):
        if row["kind"] != "uuid":
            continue
        with pytest.raises(ValueError):
            commerce.orders.create(row["value"], [])


def test_accepted_currencies_still_work(commerce, corpus):
    """Tightening input parsing must not narrow what already worked."""
    customer = _customer(commerce)
    for row in rows(corpus, "accepted_inputs"):
        if row["kind"] != "currency":
            continue
        order = commerce.orders.create(
            customer.id,
            [CreateOrderItemInput("SKU-1", "Widget", 1, 10.0)],
            currency=row["value"],
        )
        assert order.id, f"{row['id']}: {row['value']!r} must stay acceptable"


def test_a_rejected_input_writes_nothing(commerce, corpus):
    """Refusal is only meaningful if the record is not written anyway."""
    customer = _customer(commerce)
    before = len(commerce.orders.list())
    with pytest.raises(ValueError):
        commerce.orders.create(customer.id, [], currency="EURO")
    assert len(commerce.orders.list()) == before


def test_decimal_render_survives_the_boundary(commerce, corpus):
    """The exact strings the corpus pins must come back intact.

    This is the category a float-based binding cannot satisfy: the value, the
    scale, or both are lost on the way out. Python returned floats for 84
    money fields until Sep 2026.
    """
    customer = _customer(commerce)
    for row in rows(corpus, "decimal_render"):
        if row["op"] != "add" or not row["money_scale_ok"]:
            # Rows marked arithmetic-only exceed what the currency permits as
            # an amount; the engine refuses them on input, correctly, and the
            # Rust test covers their arithmetic.
            continue
        # Two order lines at unit quantity, so the engine's own arithmetic
        # produces the total a caller reads back.
        items = [
            CreateOrderItemInput(f"SKU-{i}", f"line-{i}", 1, float(operand))
            for i, operand in enumerate(row["operands"])
        ]
        order = commerce.orders.create(customer.id, items)
        want = sum((Decimal(o) for o in row["operands"]), Decimal(0))
        assert Decimal(order.total_amount_exact) == want, (
            f"{row['id']}: total_amount_exact is {order.total_amount_exact}, want {want}"
        )


def test_money_scale_is_enforced_per_currency(commerce, corpus):
    """An amount with more decimal places than its currency permits is refused.

    A binding that hardcodes two decimal places cannot express this: it takes
    the JPY fraction and scales it wrongly. Only USD rows run here, because
    orders in this fixture are denominated in the store default.
    """
    customer = _customer(commerce)
    for row in rows(corpus, "money_scale_enforced"):
        if row["currency"] != "USD":
            continue
        items = [CreateOrderItemInput("SKU-1", "Widget", 1, float(row["amount"]))]
        if row["must_reject"]:
            with pytest.raises(RuntimeError) as caught:
                commerce.orders.create(customer.id, items)
            assert "decimal places" in str(caught.value), (
                f"{row['id']}: the error should say why, got {caught.value!r}"
            )
        else:
            order = commerce.orders.create(customer.id, items)
            assert Decimal(order.total_amount_exact) == Decimal(row["amount"])


def test_decimal_render_multiplication_survives_the_boundary(commerce, corpus):
    """A quantity times a price, which is a different path from a sum.

    `19.99 x 2` is the classic case: the price has no exact binary
    representation, so a binding that multiplies in floating point lands near
    39.98 rather than on it.
    """
    customer = _customer(commerce)
    for row in rows(corpus, "decimal_render"):
        if row["op"] != "mul" or not row["money_scale_ok"]:
            continue
        price, quantity = row["operands"]
        items = [CreateOrderItemInput("SKU-1", "Widget", int(quantity), float(price))]
        order = commerce.orders.create(customer.id, items)
        assert Decimal(order.total_amount_exact) == Decimal(row["expected"]), (
            f"{row['id']}: {quantity} x {price} came back as "
            f"{order.total_amount_exact}, want {row['expected']}"
        )
