# General Ledger Reference

## Account Type Hierarchy

| Type | Normal Balance | Subtypes |
|------|---------------|----------|
| Asset | Debit | Cash, AccountsReceivable, Inventory, PrepaidExpenses, FixedAssets, AccumulatedDepreciation, OtherAssets |
| Liability | Credit | AccountsPayable, AccruedLiabilities, UnearnedRevenue, NotesPayable, OtherLiabilities |
| Equity | Credit | OwnersEquity, RetainedEarnings, CommonStock, OtherEquity |
| Revenue | Credit | SalesRevenue, ServiceRevenue, InterestIncome, OtherIncome |
| Expense | Debit | CostOfGoodsSold, Salaries, Rent, Utilities, Depreciation, OtherExpenses |

## Account Structure

- `account_number`: Unique identifier (e.g., "1000", "4100")
- `account_type` / `account_subtype`: Classification
- `is_header`: True for summary accounts (cannot post to)
- `parent_id`: Hierarchy for sub-accounts
- `currency`: Account currency (default: store currency)
- `status`: Active, Inactive, Archived

## Journal Entry Rules

1. Every entry must have at least 2 lines
2. Total debits MUST equal total credits
3. Can only post to "posting" accounts (is_header = false)
4. Can only post to Open periods
5. Posted entries are immutable; void or reverse instead

## Auto-Posting Configuration

Automate GL entries from commerce events:

| Source | Debit Account | Credit Account |
|--------|--------------|----------------|
| AutoInvoice | Accounts Receivable | Sales Revenue |
| AutoPayment | Cash | Accounts Receivable |
| AutoBill | Expense/Inventory | Accounts Payable |
| AutoBillPayment | Accounts Payable | Cash |
| AutoInventory | COGS | Inventory |
| AutoWriteOff | Bad Debt Expense | Accounts Receivable |

## Financial Statements

### Trial Balance
- Lists all accounts with debit/credit balances
- Total debits must equal total credits
- Used to verify GL accuracy before closing

### Balance Sheet
- Assets = Liabilities + Equity
- Point-in-time snapshot

### Income Statement
- Revenue - Expenses = Net Income
- Period-based (month, quarter, year)

## Period Management

```
Future → Open → Closed → Locked
```

- **Future**: Period not yet started
- **Open**: Accepts journal entries
- **Closed**: Soft close; can reopen if needed
- **Locked**: Permanent; no changes allowed
