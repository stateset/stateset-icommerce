# Accounting & Finance

iCommerce includes core accounting modules: accounts payable, accounts receivable, cost accounting, credit management, and general ledger.

## Accounts Payable (A/P)

Track what you owe suppliers:

```javascript
// List outstanding payables
const bills = commerce.accountsPayable.list();

// Aging report
const aging = commerce.accountsPayable.aging();
```

## Accounts Receivable (A/R)

Track what customers owe you:

```javascript
// List outstanding receivables
const invoices = commerce.accountsReceivable.list();

// Aging report
const aging = commerce.accountsReceivable.aging();
```

## Invoicing

Create and manage B2B invoices:

```javascript
const invoice = commerce.invoices.create({
    customerId: customer.id,
    items: [
        { description: 'Consulting Services', amount: 5000.00 }
    ],
    dueDate: '2026-04-15',
    terms: 'Net 30'
});

// Mark as paid
commerce.invoices.markPaid(invoice.id);
```

The heartbeat monitor detects overdue invoices:

```json
{
    "id": "overdue-invoices",
    "checker": "overdue-invoices",
    "intervalMs": 86400000,
    "enabled": true
}
```

## Cost Accounting

Track standard costs, variances, and COGS:

```javascript
// Get cost breakdown for a product
const costs = commerce.costAccounting.getProductCost('WIDGET-001');
// → { materialCost: 12.50, laborCost: 5.00, overhead: 2.50, totalCost: 20.00 }
```

## Credit Management

Manage customer credit limits:

```javascript
// Set credit limit
commerce.credit.setLimit(customer.id, { limit: 10000.00, currency: 'USD' });

// Check available credit
const credit = commerce.credit.check(customer.id);
// → { limit: 10000.00, used: 3500.00, available: 6500.00 }

// Place a credit hold
commerce.credit.hold(customer.id, { reason: 'Payment overdue' });
```

## General Ledger

Journal entries and financial reporting:

```javascript
// Create a journal entry
commerce.generalLedger.createEntry({
    date: '2026-03-16',
    description: 'Monthly revenue accrual',
    lines: [
        { account: '4000', debit: 0, credit: 50000.00 },
        { account: '1200', debit: 50000.00, credit: 0 }
    ]
});
```

## Money-Integrity Guarantees

The finance modules enforce these invariants on both storage backends
(SQLite and Postgres):

**General ledger**
- Journal entries must balance exactly (decimal-exact debits == credits)
  and every line is a pure debit or a pure credit.
- Auto-posting is idempotent and race-free: the duplicate check, the source
  document read, and the posted entry share one write transaction, so
  retrying (or concurrently invoking) `auto_post_invoice`,
  `auto_post_payment_received`, `auto_post_bill`, `auto_post_bill_payment`,
  `auto_post_inventory_cost`, or `auto_post_write_off` for the same source
  document returns the existing journal entry instead of posting twice.
  A unique index on the journal's source-document key backs this at the
  database level, so even writers that bypass the application layer cannot
  double-post a single-entry document (voiding frees the key for a
  corrected re-post).
- Posting or voiding an entry requires its accounting period to be open —
  including through the governed kernel `ledger.post` command, which
  rejects durably with `commerce.ledger.period_not_open`.
- A period with a standing closing entry cannot be closed again. To adjust
  a closed period: reopen it, post adjustments, and call `reclose_period`,
  which voids the standing closing entry and closes again in one operation.
- The income statement excludes closing entries, so a closed period's P&L
  reports its actual activity.
- Trial balance, balance sheet, and dated account balances derive from
  posted journal lines dated on or before the requested date; an undated
  account-balance read returns the live running balance.
- A crashed reversal (claimed but never posted) resumes on retry; a
  completed reversal rejects a second attempt.

**Accounts payable**
- Bill line items can only be added or removed on draft/pending bills;
  bill creation commits header, lines, and totals in one transaction.
- Payments are bounded by the bill balance, serialized under a write
  lock, and payment/bill lifecycle transitions (clear, cancel, dispute,
  approve) are status-guarded.
- Payment runs follow a state machine (draft/pending → approved →
  completed, cancellable until processing). Processing atomically creates
  a real payment plus allocation per bill, re-reading each balance under
  lock; bills paid since run creation are skipped and recorded on the run.
- Three-way matching aggregates billed quantities per purchase-order line
  across the whole bill, so split or duplicated lines cannot pass
  over-billing through the match.

**Accounts receivable & revenue recognition**
- A payment can never be applied beyond its own amount, and applications
  and credit memos are rejected on voided or written-off invoices.
- Write-offs must be positive and bounded by the invoice balance;
  reversing a write-off restores the status the balance and due date
  actually imply.
- Direct payments recorded on an invoice survive later credit-memo and
  payment-application recalculations (`amount_paid` = direct payments +
  applications + applied credits).
- Customer statements list invoices, payments, credit memos, and
  write-offs in one dated running balance that starts from a derived
  opening balance.
- Revenue is only recognized on active contracts (never draft or
  cancelled), and a recognition or depreciation posts its GL entry inside
  the same transaction as the subledger update — the two commit or roll
  back together, so they can never diverge.

## Treasury

Fund management and cash flow:

```javascript
const forecast = await toolkit.executeTool('cash_flow_forecast', {
    period: '30d'
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_invoices` | List all invoices |
| `create_invoice` | Create an invoice |
| `mark_invoice_paid` | Record payment |
| `ap_aging_report` | A/P aging |
| `ar_aging_report` | A/R aging |
| `get_product_cost` | Cost breakdown |
| `check_credit` | Check customer credit |
| `create_journal_entry` | GL journal entry |
| `cash_flow_forecast` | Cash flow projection |
