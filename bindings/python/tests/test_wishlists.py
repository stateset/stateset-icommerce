"""Wishlists API tests for the stateset_embedded Python bindings."""

import pytest
from stateset_embedded import Commerce, CreateProductVariantInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def _customer_and_product(commerce):
    customer = commerce.customers.create(
        email="wisher@example.com", first_name="Wish", last_name="Er"
    )
    product = commerce.products.create(
        name="Wished Widget",
        variants=[CreateProductVariantInput(sku="WISH-PY-001", price=9.99, name="Default")],
    )
    return customer, product


def test_wishlists_api_exists(commerce):
    assert commerce.wishlists is not None
    assert commerce.wishlists.is_supported() is True


def test_wishlist_full_lifecycle(commerce):
    w = commerce.wishlists
    customer, product = _customer_and_product(commerce)

    wishlist = w.create(customer_id=customer.id, name="Holiday picks", is_public=True)
    assert wishlist.customer_id == customer.id
    assert wishlist.name == "Holiday picks"
    assert wishlist.is_public is True
    assert wishlist.items == []
    assert wishlist.id

    # add an item with variant / quantity / priority — all must round-trip
    item = w.add_item(
        wishlist.id, product.id, variant_id="VAR-1", quantity=3, note="the blue one", priority=1
    )
    assert item.product_id == product.id
    assert item.variant_id == "VAR-1"
    assert item.quantity == 3
    assert item.note == "the blue one"
    assert item.priority == 1

    fetched = w.get(wishlist.id)
    assert len(fetched.items) == 1
    stored = fetched.items[0]
    assert stored.product_id == product.id
    assert stored.variant_id == "VAR-1"
    assert stored.quantity == 3
    assert stored.priority == 1

    # update
    updated = w.update(wishlist.id, name="Renamed", is_public=False)
    assert updated.name == "Renamed"
    assert updated.is_public is False

    # list by customer
    assert any(x.id == wishlist.id for x in w.list(customer_id=customer.id))

    # remove item
    w.remove_item(wishlist.id, product.id)
    assert len(w.get(wishlist.id).items) == 0

    # delete
    w.delete(wishlist.id)
    assert w.get(wishlist.id) is None
