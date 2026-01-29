# Invoices Flow

## Statuses

- draft -> sent -> viewed -> partially_paid -> paid
- overdue is a state after due date

## Common Commands

- `stateset --apply "create invoice for customer CUST-123 with 10 units of CONSULTING"`
- `stateset --apply "send invoice INV-123"`
- `stateset --apply "record invoice payment INV-123 amount 5000 via bank_transfer"`
- `stateset "overdue invoices 30 days"`
