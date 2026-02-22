# Return Processes

## Return Reasons

Returns are initiated by customers for one of the following reasons:

- **defective** — Item arrived damaged or does not function as expected
- **wrong_item** — Customer received a different item than ordered
- **not_as_described** — Item does not match the product listing
- **no_longer_needed** — Customer changed their mind or no longer wants the item
- **changed_mind** — Synonym for no_longer_needed
- **exchange** — Customer wants a different size, color, or variant

## RMA Workflow

RMA (Return Merchandise Authorization) is the formal process for processing returns.

```
create_return → approve_return / reject_return → receive → inspect → refund
```

### Steps in Detail

1. **Request** (`create_return`) — Customer submits return request with order ID, items, quantities, and reason
2. **Review** — Agent reviews request against policy (within return window, eligible items)
3. **Approve** (`approve_return`) — Issues RMA number; customer is notified to ship items back
4. **Reject** (`reject_return`) — Reject with a reason (outside window, non-returnable item, policy violation)
5. **Receive** — Warehouse confirms receipt of returned package
6. **Inspect** — Quality check performed on returned items
7. **Refund** — Refund issued based on inspection result

## Return Window

- Default return window: **30 days from delivery date**
- Returns outside this window require manager override
- Digital goods and perishables may have shorter or no return windows
- Defective items may qualify for extended return windows

## Refund Methods

| Method | Description |
|--------|-------------|
| **Original payment** | Refund to the card or wallet used at purchase |
| **Store credit** | Credit added to customer account for future purchases |
| **Exchange** | Replacement item shipped; no monetary refund |
| **Partial refund** | When only some items in a return are approved |

Refunds to original payment typically take 3-10 business days to appear.

## Quality Inspection and Restocking

After a return is received, items go through inspection:

- **Pass** — Item is in resellable condition; returned to available inventory
- **Damaged** — Item is not resellable; written off or sent to refurbishment
- **Defective** — Manufacturer defect confirmed; warranty or replacement applies

Restocking adjusts inventory: `adjust_inventory` with reason `returned`.

## Return Analytics

- `get_return_metrics` — return rate, total refunds, common return reasons
- High return rates on specific SKUs may indicate product quality or listing issues
