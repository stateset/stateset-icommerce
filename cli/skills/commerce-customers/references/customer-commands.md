# Customer Commands

## CLI (Natural Language)

- `stateset "list customers"`
- `stateset "find customer customer@example.com"`
- `stateset --apply "create customer customer@example.com Example Customer"`
- `stateset --apply "update customer CUST-123 phone +1-555-0101"`

## Direct CLI

- `stateset-direct customers list`
- `stateset-direct customers get <id-or-email>`

## Common Fields

- `email`, `first_name`, `last_name`, `phone`
- `accepts_marketing`
