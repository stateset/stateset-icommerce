# Suppliers and Purchase Orders

## PO Statuses

draft -> pending_approval -> approved -> sent -> acknowledged -> received

## Common Commands

- `stateset --apply "create supplier Example Supplier"`
- `stateset --apply "create purchase order for supplier SUP-123"`
- `stateset --apply "approve purchase order PO-123"`
- `stateset --apply "send purchase order PO-123"`
