---
name: commerce-finance
description: Use when working the general ledger, month-end close, AP/AR workflows, fixed assets, or revenue recognition.
---

# Commerce Finance Skill

Domain knowledge for the finance suite: general ledger, period close, accounts
payable, accounts receivable, fixed assets, and ASC 606-style revenue recognition.

## Money Conventions (read first)

1. **Exact decimal strings** — monetary amounts cross the binding boundary as
   strings like `"1234.56"`, never floats. Preserve them as strings; do not
   round-trip through `parseFloat`.
2. **Idempotency on money-moving posts** — the HTTP API requires an
   `Idempotency-Key` on `POST /orders`, `/payments`, `/payments/{id}/refund`,
   and `/ap/payments`. A missing key returns **428**; replaying the same key
   with a different payload returns **422**. Never retry a money-moving post
   with a fresh key — reuse the original.

## General Ledger

### Chart of Accounts
```
1. initialize_chart_of_accounts   # Seed a standard chart (once per ledger)
2. list_gl_accounts / get_gl_account
3. create_gl_account              # Add custom accounts as needed
```

### Journal Entries
- `post_journal_entry` — lines must balance (debits = credits) and the period must be open
- `void_journal_entry` — reverses a posted entry; never edit posted entries
- `get_gl_account_balance` — single-account balance
- `list_journal_entries` / `get_journal_entry`

### Financial Statements
| Tool | Statement |
|------|-----------|
| `get_trial_balance` | All accounts, debit/credit totals must tie |
| `get_balance_sheet` | Assets = Liabilities + Equity |
| `get_income_statement` | Revenue - Expenses for a period |

Always run `get_trial_balance` before and after a close to confirm it stays balanced.

## Accounting Periods

```
create_gl_period ──► open_gl_period ──► [post entries] ──► close (via close_month)
                                                              │
                                                            lock (HTTP: period lock/reopen)
```

- `create_gl_period` — fiscal year + period number (1-13)
- `open_gl_period` — entries can only post to open periods
- `list_gl_periods` — review period statuses
- Close happens through `close_month`; the HTTP API additionally exposes
  period lock/reopen for hard-locking closed periods.

## Month-End Close (`close_month`)

One orchestrated operation, in order:
```
depreciation posting ──► revenue recognition ──► FX revaluation ──► period close
```

### Dry-Run-First Discipline
**Always run with `dryRun: true` first.** The dry run computes the full
per-step report without writing anything.

```javascript
close_month({ periodId, dryRun: true })   // preview
// review report, then:
close_month({ periodId })                  // real close (requires --apply)
```

### Reading the Per-Step Report
Each step reports entry counts, totals, and warnings. Per-item failures
**never abort the close** — they appear as warnings, so:
1. Check each step's entry count against expectations (e.g. one depreciation entry per in-service asset)
2. Read every warning; a skipped asset or contract is a warning, not an error
3. Compare dry-run totals to the real close totals — they should match
4. Use skip flags (`skipDepreciation`, `skipRevenueRecognition`, `skipRevaluation`, `skipPeriodClose`) to run a partial close when a step needs manual attention

### FX Revaluation
- `revalue_gl` (or the revaluation step of `close_month`) posts unrealized
  gain/loss on foreign-currency balances at the as-of rate
- Idempotent: re-running at the same rate posts no additional entry
- Posts one balanced adjusting entry to the configured FX gain/loss account,
  sign-aware for credit-normal accounts

## Accounts Payable

### Bill Lifecycle
```
create_bill ──► approve_bill ──► payment (POST /ap/payments, Idempotency-Key required)
      │
      └──► cancel_bill
```

### 3-Way Match
`three_way_match_bill` matches bill ↔ purchase order ↔ receipt with a
tolerance (computed on read, nothing written):
- **Within tolerance** — quantities and prices agree; safe to approve
- **Variance detected** — the report shows which line and dimension
  (quantity vs price) diverged; a domain event fires on variance
- Interpret tolerance as a percentage band, e.g. 2% allows a $100 PO line
  to be billed at $98–$102

### Aging & Payment Planning
- `get_accounts_payable_aging_summary` — buckets (current / 30 / 60 / 90+)
- `list_overdue_bills` / `list_bills_due_soon` — payment-run candidates
- `get_accounts_payable_total_outstanding` — total AP exposure
- Payment runs with allocations live on the HTTP API (`/ap/payments`)

### Vendor Credits & Prepayments
Apply `create_vendor_credit` / `apply_vendor_credit` against bills before
paying; `reverse_vendor_credit_application` corrects mistakes.

## Accounts Receivable

- `get_accounts_receivable_aging_summary` — AR buckets per customer
- `get_days_sales_outstanding` — DSO metric
- `get_accounts_receivable_total_outstanding` — total AR exposure
- **Credit memos**: `create_credit_memo` → apply against invoices;
  `void_credit_memo` for mistakes; `list_unapplied_credits` shows credits
  awaiting application
- **Payment application, write-offs, statements** — HTTP API (`/ar/...`)
- **Dunning-due queue** — `GET /ar/dunning/due` lists customers due for a
  collection touch; record collection activities via the AR collections API
- Invoice side: `create_invoice` → `send_invoice` → `record_invoice_payment`;
  `get_overdue_invoices` feeds the dunning process

## Fixed Assets

### Lifecycle
```
create_fixed_asset (draft) ──► place_asset_in_service ──► dispose_fixed_asset
                                        │                       (gain/loss computed)
                                        └──► write_off_fixed_asset
```

### Depreciation
- Methods: **straight-line** and **declining-balance**, with an exact
  final-period plug (schedule always sums exactly to depreciable basis)
- `generate_depreciation_schedule` / `get_depreciation_schedule` — preview
- `post_depreciation` — posts periods through a date; also runs as the
  first step of `close_month`
- With GL auto-posting enabled, posting creates a balanced journal entry
  (depreciation expense / accumulated depreciation)

## Revenue Recognition (ASC 606-style)

```
create_revenue_contract ──► obligations (allocation must sum to contract)
        │
        ▼
generate_revenue_schedule    # ratable | point_in_time | milestone
        │
        ▼
recognize_revenue            # recognize through a date (requires --apply)
```

- Ratable schedules spread evenly per period; rounding is capped so no
  period goes negative
- `get_revenue_schedule` — review before recognizing
- Recognition also runs as a step of `close_month`
- With GL auto-posting: deferred revenue → sales revenue entries

## Best Practices

1. **Dry-run every close** — never run `close_month` cold
2. **Trial balance before and after** — it must stay balanced
3. **Reuse idempotency keys on retries** — never mint a new key for the same payment
4. **Keep amounts as strings** — exact decimals end to end
5. **3-way match before approving bills** — investigate variances, don't override silently
6. **Void, don't edit** — posted journal entries and sent invoices are immutable

## Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `428 Precondition Required` | Missing Idempotency-Key on money post | Add a key; reuse it on retry |
| `422` on replay | Same key, different payload | Use the original payload or a new key for a genuinely new operation |
| `Period not open` | Posting to closed/locked period | `open_gl_period` or use current period |
| `Entry does not balance` | Debits ≠ credits | Fix line amounts |
| `Asset not in service` | Depreciating a draft asset | `place_asset_in_service` first |
| `Allocation mismatch` | Obligations don't sum to contract | Adjust obligation allocations |
