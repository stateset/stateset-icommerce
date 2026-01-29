# Credit Management Reference

## Credit Account Fields

- `customer_id`: Linked customer
- `credit_limit`: Maximum credit allowed
- `credit_used`: Current outstanding balance
- `credit_available`: limit minus used
- `risk_rating`: Low, Medium, High, Critical
- `payment_terms`: Net 30, Net 60, etc.
- `last_review_date` / `next_review_date`: Review schedule

## Credit Check Flow

```
Order Placed
  └─ Check Credit
       ├─ Approved → Order proceeds
       ├─ Denied → Order rejected, notify customer
       └─ RequiresApproval → Hold placed, manager reviews
```

## Credit Hold Types

| Hold Type | Trigger | Resolution |
|-----------|---------|------------|
| OverLimit | Order exceeds available credit | Payment received or limit increased |
| PastDue | Customer has overdue invoices | Outstanding invoices paid |
| Manual | Credit manager decision | Manager releases hold |
| NewCustomer | First order from new customer | Credit application approved |
| HighRisk | Risk rating elevated | Risk review completed |

## Credit Application

Application includes:
- Business information (name, tax ID, years in business)
- Requested credit limit
- Trade references
- Bank references
- Financial statements

Review workflow:
1. Pending (submitted)
2. UnderReview (assigned to analyst)
3. MoreInfoNeeded (additional docs requested)
4. Approved (with approved limit, terms) or Denied (with reason)

## Credit Transactions

| Type | Effect on Credit Used |
|------|----------------------|
| Charge | Increases (order placed) |
| Payment | Decreases (payment received) |
| CreditMemo | Decreases (credit issued) |
| Adjustment | Increases or decreases |
| WriteOff | Decreases (bad debt) |
| LimitChange | No effect on used; changes limit |

## Risk Rating Guidelines

| Rating | Criteria |
|--------|----------|
| Low | Strong payment history, low utilization |
| Medium | Occasional late payments, moderate utilization |
| High | Frequent late payments, high utilization |
| Critical | Past-due > 90 days, over limit |
