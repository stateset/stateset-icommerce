# Accounting & Finance

iCommerce provides a general ledger, accounts payable and receivable, customer credit, inventory costing, fixed assets and revenue recognition on SQLite and PostgreSQL. Finance amounts at the Node and MCP boundaries are decimal strings. See the [ERP upgrade guide](../guides/erp-upgrade.md) before opening an existing store with this release.

## Invoices and customer settlements

```javascript
const invoice = await commerce.invoices.create({
  customerId: customer.id,
  currency: 'USD',
  dueDate: '2026-12-01T00:00:00Z',
  items: [{ description: 'Consulting', quantity: '1', unitPrice: '5000.00' }],
});
await commerce.invoices.send(invoice.id);
await commerce.invoices.recordPayment(invoice.id, {
  paymentId: crypto.randomUUID(), // Persist this ID and reuse it for retries.
  amount: '1250.00',
  paymentMethod: 'wire',
  reference: 'BANK-42',
});
const current = await commerce.invoices.get(invoice.id);
console.log(current.currency, current.balanceDue);
```

Only drafts can be edited. Issued invoices use credit memos, settlements or write-offs. Direct payment receipts survive subsequent recalculation. A payment allocated through `accountsReceivable.applyPaymentToInvoices` must be completed, belong to the same customer, have the same currency, and have enough funds after refunds and previous allocations. Unapply those allocations before refunding their funding.

`accountsReceivable.createWriteOff({ invoiceId, amount, reason }, actor)` reduces only the stated amount. An invoice becomes written off only when its remaining balance reaches zero. `reverseWriteOff(id)` restores that amount and reverses the associated bad-debt posting. Terminal invoices refuse payment unapplication until the write-off is reversed.

Customer statements use dated issued activity, direct receipts, allocations, credits and write-offs, retaining reversal history. Closing balance is the running balance. Specify a currency when a customer has multiple denominations. Legacy direct receipts without dated evidence require reconciliation; the engine does not invent their dates.

## Supplier bills

```javascript
const bill = await commerce.accountsPayable.createBill({
  supplierId: supplier.id,
  createdBy: 'user:buyer', // Trusted host identity; never use agent arguments.
  dueDate: '2026-12-01T00:00:00Z',
  currency: 'EUR',
  items: [{ description: 'Supplies', quantity: '1', unitPrice: '250.00' }],
});
await commerce.accountsPayable.approveBill(bill.id, undefined, 'user:controller');
await commerce.accountsPayable.createPayment({
  supplierId: supplier.id,
  amount: '250.00',
  paymentMethod: 'wire',
  allocations: [{ billId: bill.id, amount: '250.00' }],
});
```

Bill approval runs cumulative three-way matching against the purchase order and accepted receipts. A mismatch requires an explicit override reason. The maker cannot approve their own bill or payment run. A supplier payment derives its currency from the bills and rejects mixed currencies; each payment run has one currency.

Prepayments and vendor credits are settlements: applying or reversing one updates the bill or payment obligation in the same transaction, validating supplier, currency, status and remaining balance. Later cash-payment voids preserve these settlements. A settled bill cannot be cancelled. Resolving a dispute restores its prior state, adjusted for its current balance. Prepayment refunds record the refunded amount and time separately from applications.

## Ledger posting and close

```javascript
const accounts = await commerce.generalLedger.initializeChartOfAccounts();
const cash = accounts.find(a => a.accountNumber === '1010');
const sales = accounts.find(a => a.accountNumber === '4010');
// First create and open an accounting period containing this date.
const draft = await commerce.generalLedger.createJournalEntry({
  entryDate: '2026-10-15',
  description: 'Cash sale',
  lines: [
    { accountId: cash.id, debitAmount: '123.45' },
    { accountId: sales.id, creditAmount: '123.45' },
  ],
}, 'user:controller');
await commerce.generalLedger.postJournalEntry(draft.id, 'user:controller');
```

Posting checks exact balance, active posting accounts, valid account types and an open nonoverlapping period. Headers cannot receive postings. SQLite transactions and PostgreSQL parent/period locks keep these checks atomic. Reversals retain the original entry in dated reports; the reversing entry offsets it on its own date. Source-linked postings and their reversals preserve retry identity.

Configure automatic posting with `getAutoPostingConfig` and `setAutoPostingConfig`. Tax, shipping, deferred revenue, received-but-unbilled goods, inventory adjustments and work in progress have explicit account mappings. Invalid or incomplete required mappings fail; the engine does not silently substitute unrelated accounts. Invoice and bill postings separate their net amounts and mapped components. Operational stock postings maintain inventory and cost layers together. Fixed-asset and revenue-recognition postings commit with their subledger changes.

Python exposes the same configuration through `general_ledger.get_auto_posting_config()` and `set_auto_posting_config(config_json)`. They exchange JSON with snake_case account fields. Configure the required mappings before `close_month`; assets also need explicit asset, accumulated-depreciation and depreciation-expense accounts.

Dated balances include activity through the requested date. Balance sheets include unclosed earnings. Income statements exclude closing entries and retain reversed originals. Reopen a closed period to adjust it, then use `reclosePeriod(id, actor)` when a closing entry already exists. Close/lock/reopen on an invalid state returns a conflict. FX valuation uses dated rates and the functional currency becomes immutable after posting activity.

## Costing and credit

```javascript
await commerce.costAccounting.setItemCost({
  sku: 'WIDGET', costMethod: 'fifo', standardCost: '20.00',
  materialCost: '12.50', laborCost: '5.00', overheadCost: '2.50',
});
const costs = await commerce.costAccounting.getItemCost('WIDGET');
await commerce.credit.createCreditAccount({ customerId: customer.id, creditLimit: '10000.00' });
const eligibility = await commerce.credit.checkCredit(customer.id, '250.00');
```

Receipts create cost layers; issues consume them according to each SKU's method. Valuation includes owned stock in transit. Recursive BOM costing rejects cycles and applies component quantities through nested assemblies. Historical reports reconstruct stored movements and layer history.

A configured credit account applies to all unpaid orders and issued invoices, as well as manual charges and reservations. Related order/invoice exposure is not counted twice. Suspended/closed accounts and active holds refuse new credit exposure. Manual charges with no document linkage are conservatively included separately; reconcile them before relying on available headroom.

## HTTP, MCP and admin

HTTP finance approvals, payments, applications, write-offs and postings require `ManageFinance` (Admin). Posting and approval identities come from the authenticated principal. ERP mutation requests require an `Idempotency-Key`; reuse the same key and identical body for a retry. An unresolved prior execution refuses a retry for operator reconciliation.

MCP finance and operations profiles are smoke-gated and have seeded workflow tests. Use `--profile finance` or `--profile operations`, `--apply` for mutations and an operator-configured `--actor` (`STATESET_ERP_ACTOR`) for finance identity. Configure separate authenticated endpoints/principals for makers and approvers. Governed `post_journal_entry` additionally requires trusted kernel configuration and stamps its kernel principal. Strict kernel mode exposes only governed writes; it does not expose every legacy ERP action. See the [generated tool catalog](https://github.com/stateset/stateset-icommerce/blob/master/cli/docs/TOOLS.md).

The admin bills and receivables pages provide reviewed settlement actions. Mutations verify the signed-in user's administrator role and use that user's identity. The ledger page exposes posting settings, and the manufacturing setup page provides BOMs, batches and item costs. Demo engines do not execute these ERP mutations.
