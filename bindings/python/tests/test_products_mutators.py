import pytest
from stateset_embedded import Commerce, CreateProductVariantInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_product_update_and_variant_update(commerce: Commerce):
    # Create product with one variant
    product = commerce.products.create(
        name="Mutator Test Product",
        description="Initial description",
        variants=[
            CreateProductVariantInput(sku="MUT-001", price=10.00, name="Default"),
        ],
    )
    assert product.id

    # Update product description and status via kwargs (signature must be preserved)
    updated = commerce.products.update(
        product.id,
        description="Updated description",
        status="active",
    )
    assert updated.id == product.id
    assert updated.description == "Updated description"
    assert updated.status == "active"

    # Locate existing variant by SKU
    variant = commerce.products.get_variant_by_sku("MUT-001")
    assert variant is not None

    # Update variant price and SKU
    updated_variant = commerce.products.update_variant(
        variant.id,
        CreateProductVariantInput(
            sku="MUT-001-NEW",
            price=12.50,
            name="Default Updated",
            compare_at_price=15.00,
        ),
    )
    assert updated_variant.id == variant.id
    # Kernel does not change SKU on variant update; price and other fields update.
    assert updated_variant.sku == "MUT-001"
    assert pytest.approx(updated_variant.price, rel=1e-6) == 12.50
    assert pytest.approx(updated_variant.compare_at_price, rel=1e-6) == 15.00

    # Verify lookups: original SKU still resolves and reflects new price; new SKU does not exist.
    got_old = commerce.products.get_variant_by_sku("MUT-001")
    assert got_old is not None
    assert got_old.id == variant.id
    assert pytest.approx(got_old.price, rel=1e-6) == 12.50
    assert commerce.products.get_variant_by_sku("MUT-001-NEW") is None
