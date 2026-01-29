# Lot & Serial Traceability Reference

## Lot Tracking

### Lot Fields
- `lot_number`: Unique batch identifier (LOT-YYYY-XXXX)
- `sku`: Product SKU
- `quantity_initial` / `quantity_available`: Starting and current quantity
- `production_date` / `expiration_date`: Lifecycle dates
- `supplier_id`, `purchase_order_id`: Source tracking
- `status`: Active, Consumed, Expired, Quarantine, Transferred

### Lot Operations

| Operation | Description |
|-----------|-------------|
| Consume | Reduce quantity (for orders, production) |
| Adjust | Correct quantity (cycle count, damage) |
| Transfer | Move lot to another warehouse |
| Split | Divide one lot into two |
| Merge | Combine lots of same SKU |
| Quarantine | Hold lot for quality investigation |
| Release | Release from quarantine |

### FIFO/LIFO

- **FIFO** (First In, First Out): Consume oldest lots first (default for perishables)
- **LIFO** (Last In, First Out): Consume newest lots first

### Lot Certificates

| Type | Purpose |
|------|---------|
| COA | Certificate of Analysis - lab test results |
| COC | Certificate of Compliance - meets specifications |
| MSDS | Material Safety Data Sheet - handling/safety info |

### Lot Traceability

Bidirectional tracing:
- **Upstream**: Where did this lot come from? (supplier, PO, production run)
- **Downstream**: Where did this lot go? (orders, customers, locations)

## Serial Number Tracking

### Serial Fields
- `serial_number`: Unique unit identifier
- `sku`: Product SKU
- `lot_number`: Associated lot (optional)
- `status`: Full lifecycle tracking
- `customer_id`: Current owner
- `location`: Current warehouse location
- `warranty_id`: Linked warranty

### Serial Lifecycle

```
Available → Reserved → Sold → Shipped → InService
                                            ↓
                                        Returned
                                       ↙        ↘
                                  Repaired    Scrapped
                                     ↓
                                 InService
```

### Serial Reservations
- Reserve serials for specific orders
- Reservations have expiration dates
- Auto-release if not confirmed

### Bulk Serial Creation
- Generate sequentially numbered serials
- Optional prefix (e.g., "SN-W001-")
- Automatically assigned to SKU

### Serial Lookup
Complete serial information including:
- Current status and location
- Customer/owner
- Full event history
- Warranty status
- Lot association

## Industry Use Cases

| Industry | Lot Tracking | Serial Tracking |
|----------|-------------|-----------------|
| Pharma | Batch expiration, recall | Individual unit tracking |
| Food | Best-by dates, FIFO | N/A typically |
| Electronics | Component batches | Individual device warranty |
| Automotive | Parts lots | VIN-level tracking |
| Medical Devices | Sterilization batches | UDI compliance |
