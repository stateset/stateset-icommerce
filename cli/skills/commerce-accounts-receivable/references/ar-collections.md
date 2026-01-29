# Accounts Receivable & Collections Reference

## AR Aging Buckets

| Bucket | Days Outstanding | Recommended Action |
|--------|-----------------|-------------------|
| Current | Not yet due | No action |
| 1-30 days | 1-30 past due | Reminder1 (friendly reminder) |
| 31-60 days | 31-60 past due | Reminder2 (follow-up) |
| 61-90 days | 61-90 past due | Reminder3 (urgent notice) |
| 90+ days | Over 90 past due | DemandLetter or CollectionNotice |

## Collection Activity Types

| Activity | Description |
|----------|-------------|
| DunningLetterSent | Automated or manual dunning letter |
| PhoneCall | Phone outreach to customer |
| Email | Email follow-up |
| InPersonVisit | On-site collection visit |
| PromiseToPay | Customer commitment to pay by date |
| PaymentPlanCreated | Installment arrangement |
| SentToCollections | Escalated to collections agency |
| WriteOffApproved | Balance written off |
| DisputeLogged | Customer disputes the charge |
| DisputeResolved | Dispute resolved |
| Note | General collection note |

## Credit Memos

Issue credits to reduce customer balance:

| Reason | Use Case |
|--------|----------|
| ReturnedGoods | Customer returned items |
| PricingError | Invoice was incorrect |
| Overpayment | Customer paid too much |
| Damaged | Goods arrived damaged |
| ServiceCredit | Service-level agreement credit |
| GoodwillAdjustment | Retention/satisfaction credit |

**Credit Memo Statuses:** Open -> PartiallyApplied -> FullyApplied (or Voided)

## Write-Offs

Remove uncollectible balances from AR:
- Requires approver authorization
- Posts to Bad Debt Expense GL account
- Tracks reason and approval date

## Customer Statement

Complete account summary including:
- Opening balance
- All transactions (invoices, payments, credits)
- Running balance
- Closing balance
- Aging summary
