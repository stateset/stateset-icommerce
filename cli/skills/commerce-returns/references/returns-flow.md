# Returns Flow

## Statuses

requested -> approved -> received -> refunded
requested -> rejected

## Common Commands

- `stateset --apply "create return for order ORD-123 reason 'defective'"`
- `stateset --apply "approve return RET-123"`
- `stateset --apply "reject return RET-123 reason 'Outside window'"`
- `stateset --apply "refund return RET-123 amount 79.99"`
