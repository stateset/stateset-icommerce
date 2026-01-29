# Receiving Flow Reference

## Inbound Logistics Pipeline

```
PO / Transfer / Return
  └─ Receipt Created (Expected)
       └─ Goods Arrive (InProgress)
            └─ Items Received (quantities recorded)
                 └─ Quality Inspection (if required)
                      └─ Put-Away (move to warehouse location)
                           └─ Completed
```

## Receipt Fields

- `receipt_number`: Auto-generated (RCV-YYYY-NNNN)
- `reference_type` / `reference_id`: Links to PO, transfer, or return
- `carrier`, `tracking_number`: ASN (Advanced Shipping Notice) info
- `expected_date` / `received_date`: Timing tracking
- `dock_door`: Physical receiving location

## Receipt Item Fields

- `sku`, `product_name`: Item identification
- `quantity_expected`: From PO or source document
- `quantity_received`: Actual count
- `quantity_rejected`: Failed inspection
- `lot_number`, `serial_numbers`: Traceability data
- `condition`: New, Used, Damaged, Refurbished
- `pending_inspection_quantity`: Awaiting QC

## Receipt Types

| Type | Source | Typical Flow |
|------|--------|-------------|
| PurchaseOrder | Supplier shipment | Full inspection, put-away |
| Transfer | Another warehouse | Minimal inspection, direct put-away |
| Return | Customer return | Condition assessment, quarantine or restock |
| Production | Manufacturing floor | QC inspection, finished goods storage |
| Adjustment | Inventory correction | Direct put-away |

## Put-Away

After receiving and inspection:
1. Create put-away task with target location
2. Assign to warehouse worker
3. Worker moves goods and confirms placement
4. Inventory updated at destination location

## Common Operations

```bash
stateset --apply "create receipt for PO PO-2025-0100"
stateset --apply "receive items on RCV-2025-0042 SKU WIDGET-001 quantity 50"
stateset --apply "create put-away for RCV-2025-0042 to location LOC-A1-05"
stateset --apply "complete put-away PUTAWAY-001"
```
