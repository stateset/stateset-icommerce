"""Filter-threading tests for list endpoints that previously took no args.

Covers purchase_orders.list, work_orders.list, quality.list_inspections and
quality.list_ncrs. Proves the flattened keyword filters reach the store.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_purchase_orders_list_filters_by_supplier(commerce):
    s1 = commerce.purchase_orders.create_supplier(name="Supplier One")
    s2 = commerce.purchase_orders.create_supplier(name="Supplier Two")
    commerce.purchase_orders.create(supplier_id=s1.id)
    commerce.purchase_orders.create(supplier_id=s1.id)
    commerce.purchase_orders.create(supplier_id=s2.id)

    # Zero-arg still works.
    assert len(commerce.purchase_orders.list()) == 3

    by_supplier = commerce.purchase_orders.list(supplier_id=s1.id)
    assert len(by_supplier) == 2
    assert all(p.supplier_id == s1.id for p in by_supplier)

    limited = commerce.purchase_orders.list(limit=1)
    assert len(limited) == 1

    # Invalid filter values are rejected.
    with pytest.raises(ValueError):
        commerce.purchase_orders.list(status="not-a-status")


def test_work_orders_list_filters_by_product_and_status(commerce):
    product = commerce.products.create(name="WO Product")
    bom = commerce.bom.create(name="BOM", product_id=product.id)
    wo1 = commerce.work_orders.create(
        product_id=product.id, quantity_to_build=5, bom_id=bom.id
    )
    commerce.work_orders.create(
        product_id=product.id, quantity_to_build=3, bom_id=bom.id
    )

    assert len(commerce.work_orders.list()) == 2

    by_product = commerce.work_orders.list(product_id=product.id)
    assert len(by_product) == 2

    by_status = commerce.work_orders.list(status=wo1.status)
    assert all(w.status == wo1.status for w in by_status)
    assert len(by_status) >= 1

    limited = commerce.work_orders.list(limit=1)
    assert len(limited) == 1


def test_quality_list_filters(commerce):
    ship = commerce.quality.create_inspection(
        reference_type="shipment",
        reference_id="11111111-1111-1111-1111-111111111111",
        inspection_type="final",
    )
    commerce.quality.create_inspection(
        reference_type="purchase_order",
        reference_id="22222222-2222-2222-2222-222222222222",
        inspection_type="incoming",
    )

    assert len(commerce.quality.list_inspections()) == 2

    finals = commerce.quality.list_inspections(inspection_type="final")
    assert [i.id for i in finals] == [ship.id]

    by_ref = commerce.quality.list_inspections(
        reference_id="11111111-1111-1111-1111-111111111111"
    )
    assert len(by_ref) == 1

    assert len(commerce.quality.list_inspections(limit=1)) == 1

    widget = commerce.quality.create_ncr(
        sku="WIDGET",
        description="widget defect",
        quantity_affected=2.0,
        source="supplier_issue",
        severity="major",
    )
    commerce.quality.create_ncr(
        sku="GADGET",
        description="gadget defect",
        quantity_affected=1.0,
        source="internal_audit",
        severity="minor",
    )

    assert len(commerce.quality.list_ncrs()) == 2

    by_sku = commerce.quality.list_ncrs(sku="WIDGET")
    assert [n.id for n in by_sku] == [widget.id]

    majors = commerce.quality.list_ncrs(severity="major")
    assert len(majors) == 1
    assert majors[0].id == widget.id

    assert len(commerce.quality.list_ncrs(limit=1)) == 1
