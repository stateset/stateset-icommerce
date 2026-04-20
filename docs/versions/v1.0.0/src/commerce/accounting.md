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
