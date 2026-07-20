"""Product Reviews API tests for the stateset_embedded Python bindings."""

import pytest
from stateset_embedded import Commerce, CreateProductVariantInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def _product_and_customer(commerce):
    product = commerce.products.create(
        name="Reviewed Widget",
        variants=[CreateProductVariantInput(sku="REV-PY-001", price=19.99, name="Default")],
    )
    customer = commerce.customers.create(
        email="reviewer@example.com", first_name="Rev", last_name="Iewer"
    )
    return product, customer


def test_reviews_api_exists(commerce):
    assert commerce.reviews is not None
    assert commerce.reviews.is_supported() is True


def test_review_full_lifecycle(commerce):
    r = commerce.reviews
    product, customer = _product_and_customer(commerce)

    review = r.create(
        product_id=product.id,
        customer_id=customer.id,
        rating=5,
        title="Excellent",
        body="Works great.",
        verified_purchase=True,
    )
    assert review.product_id == product.id
    assert review.customer_id == customer.id
    assert review.rating == 5
    assert review.title == "Excellent"
    assert review.verified_purchase is True
    assert review.helpful_count == 0
    assert review.reported_count == 0
    assert isinstance(review.status, str)
    assert review.id

    # get
    assert r.get(review.id).id == review.id

    # update rating + moderation status
    updated = r.update(review.id, rating=4, status="approved")
    assert updated.rating == 4
    assert updated.status == "approved"

    # counters
    r.mark_helpful(review.id)
    r.mark_reported(review.id)
    after = r.get(review.id)
    assert after.helpful_count == 1
    assert after.reported_count == 1

    # summary — single 4-star review
    summary = r.get_summary(product.id)
    assert summary.product_id == product.id
    assert len(summary.rating_distribution) == 5
    assert summary.total_reviews >= 1
    assert summary.rating_distribution[3] == 1
    assert summary.average_rating == 4

    # list by product
    listed = r.list(product_id=product.id)
    assert any(x.id == review.id for x in listed)

    # delete
    r.delete(review.id)
    assert r.get(review.id) is None


def test_create_rejects_out_of_range_rating(commerce):
    r = commerce.reviews
    product, customer = _product_and_customer(commerce)
    with pytest.raises(Exception, match="(?i)rating must be between 1 and 5"):
        r.create(product_id=product.id, customer_id=customer.id, rating=999)
