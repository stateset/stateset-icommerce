# Payments Flow

## Statuses

pending -> processing -> completed
pending -> failed
completed -> refunded / partially_refunded

## Common Commands

- `stateset --apply "create payment for order ORD-123 amount 129.99 via credit_card"`
- `stateset --apply "refund payment PAY-123 amount 29.99 reason 'return'"`
- `stateset-pay send --to <address> --amount 10 --asset usdc`

## Notes

- Stablecoin operations use `stateset-pay` and chain configs.
- Always verify order totals before capturing payment.
