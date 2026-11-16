"""Vendor Returns API tests for the stateset_embedded Python bindings.

Money and quantities are exchanged as exact decimal strings (no float
precision loss); timestamps cross as RFC 3339 strings, enums as snake_case.
"""

import uuid

import pytest
from stateset_embedded import Commerce, VendorReturnItemInput, CreateProductVariantInput


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def seed_return_stock(commerce):
    sku = f"RETURN-{uuid.uuid4()}"
    supplier = commerce.purchase_orders.create_supplier(name=sku)
    product = commerce.products.create(name=sku, variants=[CreateProductVariantInput(sku=sku, price=10)])
    warehouse = commerce.warehouse.create_warehouse(code=sku, name=sku)
    location = commerce.warehouse.create_location(warehouse.id, "SOURCE")
    commerce.warehouse.adjust_inventory(int(location.id), sku, "5", "receipt")
    return supplier.id, product.id, sku, int(warehouse.id), int(location.id)


def test_vendor_returns_api_exists(commerce):
    assert commerce.vendor_returns is not None


def test_vendor_return_full_lifecycle(commerce):
    vr = commerce.vendor_returns
    if not vr.is_supported():
        pytest.skip("vendor returns are not supported by this engine build")

    supplier_id, product_id, sku, warehouse_id, source_location_id = seed_return_stock(commerce)

    created = vr.create(
        supplier_id=supplier_id,
        warehouse_id=warehouse_id,
        items=[
            VendorReturnItemInput(
                product_id=product_id,
                sku=sku,
                source_location_id=source_location_id,
                quantity="3",
                unit_cost="10.00",
                reason="defective",
            )
        ],
        notes="damaged on arrival",
    )
    assert created.id
    assert created.number.startswith("VR-")
    assert created.status == "draft"
    assert created.supplier_id == supplier_id
    assert created.currency == "USD"
    assert created.credit_generated is False
    assert created.notes == "damaged on arrival"
    assert created.processed_at is None
    assert len(created.items) == 1

    item = created.items[0]
    assert item.vendor_return_id == created.id
    assert item.product_id == product_id
    assert item.quantity == "3"
    assert item.unit_cost == "10.00"
    assert item.line_total == "30.00"
    assert item.reason == "defective"
    assert created.total_credit == "30.00"

    # get and list find the return
    found = vr.get(created.id)
    assert found is not None
    assert found.id == created.id

    listed = vr.list(supplier_id=supplier_id, status="draft")
    assert any(r.id == created.id for r in listed)

    # submit: draft -> pending
    submitted = vr.submit(created.id)
    assert submitted.status == "pending"

    # process with credit generation: pending -> processed
    processed = vr.process(created.id, True)
    assert processed.status == "processed"
    assert processed.credit_generated is True
    assert processed.processed_at is not None

    # processed returns cannot be cancelled
    with pytest.raises(RuntimeError):
        vr.cancel(created.id)


def test_vendor_return_cancel_and_validation(commerce):
    vr = commerce.vendor_returns
    if not vr.is_supported():
        pytest.skip("vendor returns are not supported by this engine build")

    supplier_id, product_id, sku, warehouse_id, source_location_id = seed_return_stock(commerce)
    created = vr.create(
        supplier_id=supplier_id,
        warehouse_id=warehouse_id,
        items=[
            VendorReturnItemInput(
                product_id=product_id,
                sku=sku,
                source_location_id=source_location_id,
                quantity="1",
                unit_cost="5.50",
            )
        ],
    )
    # reason defaults to defective
    assert created.items[0].reason == "defective"

    cancelled = vr.cancel(created.id)
    assert cancelled.status == "cancelled"

    # unknown IDs return None; malformed IDs raise
    assert vr.get(str(uuid.uuid4())) is None
    with pytest.raises(ValueError):
        vr.get("not-a-uuid")
    with pytest.raises(ValueError):
        vr.list(status="bogus_status")
