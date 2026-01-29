# Accounts Payable Reference

## Bill Lifecycle

```
Draft → Pending → Approved → PartiallyPaid → Paid
                                    ↓
                              Overdue (auto)
                                    ↓
                              Disputed (manual)
```

## Bill Fields

- `bill_number`: Auto-generated (BILL-YYYY-NNNN)
- `supplier_id`: Linked supplier
- `invoice_number`, `invoice_date`: Supplier's reference
- `due_date`: Payment deadline
- `payment_terms`: Net 30, Net 60, etc.
- `total_amount`, `amount_paid`, `balance_due`: Financial tracking
- `currency`: Bill currency

## Bill Items

- `description`: Line item description
- `quantity`, `unit_price`: Pricing
- `amount`: Line total
- `gl_account_id`: Posting account
- `tax_amount`: Applicable tax

## Payment Allocation

Payments can be allocated across multiple bills:
- `bill_id`: Target bill
- `amount`: Amount applied to this bill
- Partial payments leave a remaining balance

## Payment Runs (Batch Processing)

Group multiple bill payments into a single run:
1. Create payment run (Draft)
2. Add bills to the run
3. Approve the run
4. Process all payments
5. Mark completed

## AP Aging Report

| Bucket | Days Past Due |
|--------|--------------|
| Current | Not yet due |
| 1-30 | 1 to 30 days overdue |
| 31-60 | 31 to 60 days overdue |
| 61-90 | 61 to 90 days overdue |
| 90+ | Over 90 days overdue |

## AP Summary by Supplier

- Total outstanding balance
- Number of open bills
- Overdue amount
- Last payment date
- Aging breakdown by bucket
