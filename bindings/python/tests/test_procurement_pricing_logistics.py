"""Lifecycle tests for the procurement / pricing / logistics domains of the
stateset_embedded Python bindings: prepayments, vendor credits, price
schedules, price levels, transfer orders, production batches, supplier SKUs,
and inbound shipments.

Money and quantities cross as exact decimal strings; timestamps as RFC 3339
strings; enums as snake_case strings.
"""

import uuid

import pytest
from stateset_embedded import (
    Commerce,
    InboundShipmentItemInput,
    SupplierSkuBulkItemInput,
    TransferOrderItemInput,
)


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_prepayment_full_lifecycle(commerce):
    api = commerce.prepayments
    assert api.is_supported() is True

    supplier_id = str(uuid.uuid4())
    prepayment = api.create(
        supplier_id=supplier_id,
        amount="1000.00",
        method="wire",
        reference="WIRE-42",
        memo="advance for Q3",
    )
    assert prepayment.id
    assert prepayment.supplier_id == supplier_id
    assert prepayment.amount == "1000.00"
    assert prepayment.remaining == "1000.00"
    assert prepayment.status == "open"
    assert prepayment.method == "wire"

    found = api.get(prepayment.id)
    assert found is not None and found.id == prepayment.id
    assert any(p.id == prepayment.id for p in api.list(supplier_id=supplier_id, status="open"))

    target_id = str(uuid.uuid4())
    applied = api.apply(prepayment.id, "bill", target_id, "400.00")
    assert applied.remaining == "600.00"

    applications = api.list_applications(prepayment.id)
    assert len(applications) == 1
    assert applications[0].target_type == "bill"
    assert applications[0].target_id == target_id
    assert applications[0].amount == "400.00"
    assert applications[0].reversed is False

    reversed_ = api.reverse_application(prepayment.id, applications[0].id)
    assert reversed_.remaining == "1000.00"

    refunded = api.refund(prepayment.id)
    assert refunded.status == "refunded"
    assert refunded.remaining == "0"


def test_vendor_credit_full_lifecycle(commerce):
    api = commerce.vendor_credits
    assert api.is_supported() is True

    supplier_id = str(uuid.uuid4())
    credit = api.create(supplier_id=supplier_id, amount="250.00", memo="pricing adjustment")
    assert credit.id
    assert credit.amount == "250.00"
    assert credit.remaining == "250.00"
    assert credit.status == "open"

    found = api.get(credit.id)
    assert found is not None and found.id == credit.id
    assert any(c.id == credit.id for c in api.list(supplier_id=supplier_id))

    applied = api.apply(credit.id, "bill", str(uuid.uuid4()), "100.00")
    assert applied.remaining == "150.00"

    applications = api.list_applications(credit.id)
    assert len(applications) == 1
    assert applications[0].amount == "100.00"

    reversed_ = api.reverse_application(credit.id, applications[0].id)
    assert reversed_.remaining == "250.00"

    cancelled = api.cancel(credit.id)
    assert cancelled.status == "cancelled"


def test_price_schedule_full_lifecycle(commerce):
    api = commerce.price_schedules
    assert api.is_supported() is True

    product_id = str(uuid.uuid4())
    schedule = api.create(
        name="Black Friday",
        code="BF-2026",
        starts_at="2026-11-27T00:00:00Z",
        ends_at="2026-11-30T23:59:59Z",
        priority=10,
    )
    assert schedule.id
    assert schedule.name == "Black Friday"
    assert schedule.is_active is True
    assert schedule.priority == 10

    found = api.get(schedule.id)
    assert found is not None and found.id == schedule.id
    assert any(s.id == schedule.id for s in api.list(is_active=True))

    updated = api.update(schedule.id, name="Black Friday Sale")
    assert updated.name == "Black Friday Sale"

    entry = api.set_entry(schedule.id, product_id, "19.99")
    assert entry.price == "19.99"
    assert entry.product_id == product_id
    assert len(api.list_entries(schedule.id)) == 1

    assert api.resolve_price(product_id, "2026-11-28T12:00:00Z") == "19.99"
    assert api.resolve_price(product_id, "2026-12-25T12:00:00Z") is None

    api.delete_entry(schedule.id, product_id)
    assert api.list_entries(schedule.id) == []

    api.delete(schedule.id)
    assert api.get(schedule.id) is None


def test_price_level_full_lifecycle(commerce):
    api = commerce.price_levels
    assert api.is_supported() is True

    product_id = str(uuid.uuid4())
    level = api.create(
        name="Wholesale",
        code="WHOLESALE",
        adjustment_type="percentage_discount",
        adjustment_value="10",
    )
    assert level.id
    assert level.code == "WHOLESALE"
    assert level.adjustment_type == "percentage_discount"
    assert level.adjustment_value == "10"
    assert level.is_active is True

    found = api.get(level.id)
    assert found is not None and found.id == level.id
    assert any(l.id == level.id for l in api.list(is_active=True))

    updated = api.update(level.id, adjustment_value="15")
    assert updated.adjustment_value == "15"

    entry = api.set_entry(level.id, product_id, "42.00")
    assert entry.price == "42.00"
    entries = api.list_entries(level.id)
    assert len(entries) == 1 and entries[0].product_id == product_id

    api.delete_entry(level.id, product_id)
    assert api.list_entries(level.id) == []

    api.delete(level.id)
    assert api.get(level.id) is None


def test_transfer_order_full_lifecycle(commerce):
    api = commerce.transfer_orders
    assert api.is_supported() is True

    source = str(uuid.uuid4())
    destination = str(uuid.uuid4())
    product_id = str(uuid.uuid4())

    order = api.create(
        source_warehouse_id=source,
        destination_warehouse_id=destination,
        items=[TransferOrderItemInput(product_id, "10")],
        notes="restock east coast",
    )
    assert order.id
    assert order.number.startswith("TO-")
    assert order.status == "draft"
    assert len(order.items) == 1
    assert order.items[0].quantity == "10"

    found = api.get(order.id)
    assert found is not None and found.id == order.id
    assert any(o.id == order.id for o in api.list(source_warehouse_id=source))

    shipped = api.ship(order.id)
    assert shipped.status == "in_transit"
    assert shipped.items[0].quantity_shipped == "10"
    assert shipped.shipped_at is not None

    partial = api.receive_line(order.id, shipped.items[0].id, "4")
    assert partial.status == "partially_received"
    assert partial.items[0].quantity_received == "4"

    full = api.receive_line(order.id, shipped.items[0].id, "6")
    assert full.status == "received"
    assert full.received_at is not None

    other = api.create(
        source_warehouse_id=source,
        destination_warehouse_id=destination,
        items=[TransferOrderItemInput(product_id, "5")],
    )
    assert api.cancel(other.id).status == "cancelled"


def test_production_batch_full_lifecycle(commerce):
    api = commerce.production_batches
    assert api.is_supported() is True

    work_order_a = str(uuid.uuid4())
    work_order_b = str(uuid.uuid4())

    batch = api.create(name="July widgets", work_order_ids=[work_order_a], notes="first run")
    assert batch.id
    assert batch.status == "planned"
    assert batch.work_order_ids == [work_order_a]

    found = api.get(batch.id)
    assert found is not None and found.id == batch.id
    assert any(b.id == batch.id for b in api.list(status="planned"))

    updated = api.update(batch.id, name="July widgets v2", status="in_progress")
    assert updated.name == "July widgets v2"
    assert updated.status == "in_progress"

    added = api.add_work_orders(batch.id, [work_order_b])
    assert len(added.work_order_ids) == 2

    removed = api.remove_work_order(batch.id, work_order_a)
    assert removed.work_order_ids == [work_order_b]

    api.delete(batch.id)
    assert api.get(batch.id) is None


def test_supplier_sku_full_lifecycle(commerce):
    api = commerce.supplier_skus
    assert api.is_supported() is True

    supplier_id = str(uuid.uuid4())
    product_id = str(uuid.uuid4())

    record = api.create(
        product_id=product_id,
        supplier_id=supplier_id,
        sku="ACME-001",
        unit_cost="12.50",
        min_order_qty="100",
        lead_time_days=14,
    )
    assert record.id
    assert record.sku == "ACME-001"
    assert record.unit_cost == "12.50"
    assert record.min_order_qty == "100"
    assert record.lead_time_days == 14
    assert record.is_preferred is False

    found = api.get(record.id)
    assert found is not None and found.id == record.id
    assert any(r.id == record.id for r in api.list(supplier_id=supplier_id))

    updated = api.update(record.id, unit_cost="11.75", is_preferred=True)
    assert updated.unit_cost == "11.75"
    assert updated.is_preferred is True

    count = api.bulk_upsert(
        supplier_id,
        [
            SupplierSkuBulkItemInput(product_id, "ACME-001-B", "11.00"),
            SupplierSkuBulkItemInput(str(uuid.uuid4()), "ACME-002", "3.25"),
        ],
    )
    assert count == 2

    api.delete(record.id)
    assert api.get(record.id) is None


def test_inbound_shipment_full_lifecycle(commerce):
    api = commerce.inbound_shipments
    assert api.is_supported() is True

    supplier_id = str(uuid.uuid4())
    warehouse_id = str(uuid.uuid4())
    product_id = str(uuid.uuid4())

    shipment = api.create(
        supplier_id=supplier_id,
        items=[InboundShipmentItemInput(product_id, "SKU-1", "10")],
        warehouse_id=warehouse_id,
        carrier="DHL",
        tracking_number="1Z999",
    )
    assert shipment.id
    assert shipment.status == "pending"
    assert shipment.carrier == "DHL"
    assert len(shipment.items) == 1
    assert shipment.items[0].quantity_expected == "10"

    found = api.get(shipment.id)
    assert found is not None and found.id == shipment.id
    assert any(s.id == shipment.id for s in api.list(supplier_id=supplier_id, status="pending"))

    assert api.mark_in_transit(shipment.id).status == "in_transit"
    assert api.mark_arrived(shipment.id).status == "arrived"

    partial = api.receive_line(shipment.id, shipment.items[0].id, "4")
    assert partial.status == "partially_received"
    assert partial.items[0].quantity_received == "4"

    full = api.receive_line(shipment.id, shipment.items[0].id, "6")
    assert full.status == "received"
    assert full.received_at is not None

    other = api.create(
        supplier_id=supplier_id,
        items=[InboundShipmentItemInput(product_id, "SKU-2", "5")],
    )
    assert api.cancel(other.id).status == "cancelled"
