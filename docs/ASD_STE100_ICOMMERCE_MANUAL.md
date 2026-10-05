# StateSet iCommerce Engine — Operation and Maintenance Manual

## In ASD Simplified Technical English (ASD-STE100)

| Item                 | Data                                             |
| -------------------- | ------------------------------------------------ |
| Document ID          | SSE-ICOMMERCE-STE-001                            |
| Issue                | 001, 2026-10-02                                  |
| Language rule        | ASD-STE100 Issue 8, Simplified Technical English |
| Engine version       | 1.37.0                                           |
| Reader               | Maintenance personnel and operators              |
| Store file (default) | `./store.db`                                     |

---

## 1. About this manual

### 1.1 Purpose

This manual gives operation and maintenance data for the StateSet iCommerce engine.

It tells you how to install the engine, operate the store, and correct faults.

### 1.2 Scope

This manual covers the default SQLite configuration and these functions:

- Customers, products, and inventory
- Carts, checkout, and orders
- Payments and refunds
- Shipments, returns, and store credit
- Promotions, coupons, tax, and gift cards
- Analytics, audit log, backup, and restore

This manual does not cover the optional PostgreSQL backend in detail.

It does not cover finance, manufacturing, and warehouse extensions in detail.

### 1.3 How to use this manual

Read section 2 before you do a procedure.

Each procedure has a number (for example, OP-07).

Do the steps in the given sequence.

Obey all WARNINGS, CAUTIONS, and NOTES.

---

## 2. Writing rules used in this manual

This manual obeys ASD-STE100. The rules that follow apply to all text.

1. Use short sentences. A sentence in a procedure has 20 words maximum.
2. A descriptive sentence has 25 words maximum.
3. Use only approved meanings. One word has one meaning.
4. Use the imperative form for steps (for example, “Open the file.”).
5. Give one instruction in one sentence.
6. Use the active voice.
7. Do not use the present participle as a verb (avoid “-ing” forms).
8. Do not use synonyms. Always use the same noun for the same object.
9. Do not use idioms, slang, or ambiguous verbs such as “get” alone.
10. Use approved forms: DO, DO NOT, MAKE SURE, CHECK, REFER TO.
11. Write numbers and values clearly. Use exact decimal strings for money.
12. Start each step with a verb or a condition.

Approved verbs used here include: OPEN, CLOSE, START, STOP, INSTALL, REMOVE,
CREATE, ENTER, SELECT, SET, SEND, CHECK, MAKE SURE, WAIT, RECORD, KEEP,
RESTORE, COPY, REFER TO, CONTACT, DO, DO NOT.

## 3. Warnings, cautions, and notes

- WARNING: A WARNING tells you that injury to persons is possible.
- CAUTION: A CAUTION tells you that damage to data or equipment is possible.
- NOTE: A NOTE gives additional data.

### General safety

WARNING: DO NOT SHARE OPERATOR POLICY FILES OR PRINCIPAL FILES WITH UNAUTHORIZED PERSONS. LOSS OF THESE FILES GIVES WRITE ACCESS TO THE STORE.

CAUTION: WRITES ARE PREVIEW-ONLY BY DEFAULT. ADD `--apply` ONLY WHEN YOU INTEND TO CHANGE DATA. AN UNPLANNED WRITE CHANGES THE STORE DATABASE.

CAUTION: DO NOT DELETE THE STORE DATABASE FILE UNLESS YOU HAVE A TESTED BACKUP. DELETION REMOVES ALL ORDERS, CUSTOMERS, AND STOCK DATA.

NOTE: The engine runs in your process. It uses one database file. No network service is necessary for the default configuration.

---

## 4. Description and operation

### 4.1 General description

The StateSet iCommerce engine is an embedded commerce program.

It manages orders, inventory, customers, products, carts, checkout, payments,
returns, shipments, promotions, tax, gift cards, store credit, and analytics.

It operates against one database file. The default file is `./store.db`.

No external service is necessary. No API key is necessary. No rate limit applies.

### 4.2 Main components

- Store database: one SQLite file that holds all commerce data.
- Engine library: Rust, Node.js, and Python interfaces to the store database.
- Command line interface (CLI): `stateset` and `stateset-init` commands.
- MCP server: controlled access for AI agents (`stateset-mcp`, `stateset-mcp-http`).
- Kernel policy: operator-owned files that control write authority.

### 4.3 Money rule

Money is always an exact decimal value. Money is never a float.

ENTER money as a decimal string (for example, `"49.99"`).

### 4.4 Audit rule

Each change to the store writes a best-effort entry to the audit log.

KEEP the audit log. DO NOT EDIT the audit log manually.

### 4.5 Tool access rule

Read operations run freely. Write operations show a preview by default.

ADD `--apply` to do a write. Without `--apply`, the tool changes nothing.

Agent write access requires operator-owned policy and principal files.

Identity and policy never come from model arguments.

---

## 5. Installation and initialization

### 5.1 System requirements

- A computer with Rust 1.85 or a newer version (for Rust use).
- Node.js with npm (for CLI, MCP server, or storefront use).
- Python with pip (for Python use).
- Disk space for the store database file.
- A model API key for the `stateset` command (not necessary for library use).

### 5.2 Install the engine library

Select one language channel.

For Rust, ENTER:

```bash
cargo add stateset-sdk --features full
```

For Node.js, ENTER:

```bash
npm install @stateset/embedded@1.37.0
```

For Python, ENTER:

```bash
pip install stateset-embedded==1.37.0
```

### 5.3 Install the CLI and agent servers

For the CLI and the MCP servers, ENTER:

```bash
npm install -g @stateset/cli@1.37.0
```

### 5.4 Initialize the store database (Procedure INIT-01)

Purpose: Create a new store database with demo data.

Conditions: The CLI is installed. No store database exists at the target path.

Steps:

1. OPEN a terminal in the store folder.
2. ENTER `stateset-init --quickstart`.
3. MAKE SURE the tool creates `./store.db`.
4. MAKE SURE the tool creates `./.stateset/config.json`.
5. RECORD the database path for the next procedures.

NOTE: `--quickstart` needs no input. It creates schema and demo data.

Use `--db <path>` to select a different database location.

Use `--force` to overwrite an existing database.

Use `--demo` to add demo data without the standalone configuration.

### 5.5 Start the MCP server for agents (Procedure INIT-02)

Purpose: Give a local agent controlled access to the store.

Steps:

1. OPEN a terminal in the store folder.
2. ENTER `stateset-mcp --db ./store.db --profile core` for read and preview access.
3. If you must permit writes, ADD `--apply` with operator policy files (REFER TO section 5.6).
4. MAKE SURE the client configuration points to the correct database path.
5. TEST the connection with a read request before you permit writes.

### 5.6 Enable governed writes (Procedure INIT-03)

Purpose: Permit an agent to use approved write commands only.

Conditions: You hold the operator policy file, principal file, and store ID.

Steps:

1. STORE the policy file and principal file outside agent control.
2. START the server with `--apply`, `--kernel-policy`, `--kernel-principal`, and `--kernel-store-id`.
3. USE policy `./kernel-policy.json` and store ID `store:production`.
4. MAKE SURE the agent shows only approved commands.
5. TEST with a preview request first.
6. Next, TEST with one small approved write.
7. REMOVE `--apply` immediately if behavior is not correct.

WARNING: DO NOT PUT POLICY DATA IN TOOL ARGUMENTS. KEEP IDENTITY IN OPERATOR FILES ONLY.

### 5.7 Start the HTTP server (Procedure INIT-04)

Purpose: Share the store over MCP Streamable HTTP.

Steps:

1. ENTER `npx -y -p @stateset/cli stateset-mcp-http --db ./store.db --port 8090`.
2. ADD `--host 0.0.0.0` only to expose the server to the network.
3. If you expose the server, SET `--allowed-host` to the public host name.
4. ADD `--read-only` to prohibit all writes at the transport boundary.
5. MAKE SURE clients connect to `http://localhost:8090/mcp`.
6. STOP the server when the task is complete.

---

## 6. Operation procedures

General rules for all procedures that follow:

- READ the full procedure before you START.
- The `stateset` command needs a model API key. SET the key before you START.
- USE `stateset-config set-key anthropic` or EXPORT `ANTHROPIC_API_KEY`.
- USE `--apply` only for the WRITE step.
- CHECK the result after each write.
- RECORD IDs (customer ID, order ID, payment ID) for the next steps.

### OP-01 — Create a customer

Purpose: Add a new customer to the store.

Steps:

1. SHOW a preview of the command without `--apply`.
2. ENTER the create command with `--apply`, with email, first name, and last name.
3. MAKE SURE the tool returns a customer ID.
4. RECORD the customer ID and email.

Example: `stateset --apply "create a customer named Alice Smith with email alice@example.com"`.

Library form: `customers.create` with `email`, `firstName`, `lastName`.

### OP-02 — Create a product

Purpose: Add a new product to the catalog.

Steps:

1. SHOW a preview of the command without `--apply`.
2. ENTER the create command with `--apply`, with product name and price.
3. MAKE SURE the tool returns a product ID.
4. RECORD the product ID, name, and price.

Example: `stateset --apply "create a product called Widget at $29.99"`.

### OP-03 — Add and adjust inventory

Purpose: Set stock quantity for a stock keeping unit (SKU).

Steps:

1. IDENTIFY the SKU (for example, `RUST-BOOK-001`).
2. SHOW a preview of the adjustment without `--apply`.
3. ENTER the adjust command with `--apply`, with SKU, quantity, and reason.
4. CHECK the new quantity with a stock report.
5. RECORD the SKU and final quantity.

Example: `stateset --apply "add 100 units to SKU RUST-BOOK-001 for initial stock"`.

CAUTION: USE EXACT QUANTITIES. A WRONG QUANTITY CAUSES TOO MUCH STOCK OR NO STOCK.

Library form: `inventory.create_item`, then `inventory.adjust`.

### OP-04 — Check low stock

Purpose: Find products with low stock before the stock is empty.

Steps:

1. ENTER `stateset "what products are low on stock?"`.
2. READ the list of SKUs and quantities.
3. RECORD SKUs below the reorder limit.
4. REQUEST replenishment stock (REFER TO OP-03).

### OP-05 — Operate the cart and checkout

Purpose: Build a cart and complete checkout for a customer.

Steps:

1. CREATE a cart for the customer ID.
2. ADD each item with product ID, SKU, name, quantity, and unit price.
3. SET the shipping address on the cart.
4. SET the payment method on the cart.
5. APPLY the coupon code if the customer provides one.
6. CALCULATE tax on the cart.
7. CHECK the cart total. Money is an exact decimal string.
8. BEGIN checkout only after the customer confirms the total.
9. COMPLETE checkout.
10. RECORD the order ID and order number.

NOTE: Promotion setup is operator configuration. Agents redeem coupons but do not create them.

### OP-06 — Create an order directly

Purpose: Create an order without a cart (for service orders).

Steps:

1. IDENTIFY the customer ID and each order item.
2. SHOW a preview of the create command without `--apply`.
3. ENTER the create command with `--apply`.
4. MAKE SURE the tool returns an order ID and order number.
5. RECORD the order ID, order number, and total amount.

### OP-07 — Create and complete a payment

Purpose: Receive payment for an order.

Steps:

1. IDENTIFY the order ID, customer ID, and exact amount.
2. CREATE the payment for the order.
3. MARK the payment as completed only after funds are confirmed.
4. MAKE SURE the payment status is `completed`.
5. RECORD the payment ID and amount.

CAUTION: DO NOT MARK A PAYMENT AS COMPLETED BEFORE FUNDS ARE CONFIRMED. A FALSE STATUS CAUSES REVENUE ERROR.

Library form: `payments.create`, then `payments.mark_completed`.

### OP-08 — Issue a refund

Purpose: Return funds to the customer for a paid order.

Steps:

1. IDENTIFY the payment ID and refund amount.
2. CONFIRM the refund reason with the supervisor if policy requires it.
3. SHOW a preview of the refund without `--apply`.
4. DO the refund with `--apply`.
5. CHECK that the refund status is complete.
6. RECORD the refund ID, amount, and reason.

### OP-09 — Create and send a shipment

Purpose: Ship an order and record tracking data.

Steps:

1. IDENTIFY the order ID and recipient name.
2. ENTER the tracking number if the carrier provides one.
3. CREATE the shipment with `--apply`.
4. MAKE SURE the tool returns a shipment number.
5. SEND the tracking number to the customer.
6. RECORD the shipment number and tracking number.

### OP-10 — Create and process a return

Purpose: Accept returned goods for stock or quarantine.

Steps:

1. IDENTIFY the order ID and the items to return.
2. CREATE the return with `--apply`.
3. ENTER tracking data when the goods are in transit.
4. INSPECT the goods on receipt.
5. MOVE serviceable goods to stock.
6. MOVE damaged goods to quarantine.
7. RECORD the return ID and final disposition.

### OP-11 — Apply a promotion or coupon

Purpose: Redeem an existing coupon on a cart.

Steps:

1. CONFIRM the coupon code with the customer.
2. APPLY the coupon to the cart with the discount command.
3. CHECK the new cart total.
4. DO NOT CREATE new promotions at the agent endpoint.
5. CONTACT the store operator for new promotions.

### OP-12 — Calculate tax

Purpose: Apply correct tax to a cart or order.

Steps:

1. MAKE SURE the shipping address is set.
2. CALCULATE the tax for the cart.
3. CHECK the tax amount and total.
4. DO NOT COMPLETE checkout if tax data is missing.

### OP-13 — Make and accept a gift card

Purpose: Make a gift card and accept it as payment.

Steps:

1. CREATE the gift card with an exact decimal value.
2. RECORD the gift card code.
3. SEND the code to the customer securely.
4. When the customer pays, ENTER the gift card code.
5. CHECK the remaining balance after redemption.
6. RECORD the redemption against the order ID.

### OP-14 — Check analytics and revenue

Purpose: Check store performance and revenue.

Steps:

1. ENTER `stateset "what is my revenue this month?"`.
2. READ the revenue, order count, and payment count.
3. COMPARE the values with the previous period.
4. REPORT large differences to the supervisor.
5. DO NOT USE analytics values as accounting records without verification.

### OP-15 — Manage subscriptions (if configured)

Purpose: Check subscription status for a customer.

Steps:

1. IDENTIFY the customer ID.
2. CHECK the subscription status.
3. CHECK the next billing date and amount.
4. CANCEL only on customer request and with confirmation.
5. RECORD the change with date and reason.

---

## 7. Servicing

### 7.1 Back up the store database (Procedure SVC-01)

Purpose: Protect all commerce data against loss.

Steps:

1. STOP all writes to the store database.
2. COPY `./store.db` to a dated backup path.
3. COPY the `-wal` and `-shm` files if they exist.
4. VERIFY the backup file size is greater than zero.
5. STORE one copy in a separate location.
6. RECORD the backup date, path, and engine version.

CAUTION: DO NOT SKIP STEP 1. A BACKUP DURING WRITES CAN BE INCOMPLETE.

Recommended interval: back up daily for production stores.

### 7.2 Restore the store database (Procedure SVC-02)

Purpose: Return the store to a known good state.

Steps:

1. STOP the engine, CLI, and all MCP servers.
2. COPY the current database to a quarantine path.
3. COPY the backup file to `./store.db`.
4. START the engine in read-only or preview mode.
5. CHECK customers, orders, and stock quantities.
6. RETURN to normal operation only after checks pass.

WARNING: RESTORE REPLACES CURRENT DATA. ALL CHANGES AFTER THE BACKUP DATE ARE LOST. CONFIRM WITH THE SUPERVISOR BEFORE YOU RESTORE.

### 7.3 Month-end and audit check (Procedure SVC-03)

Purpose: Confirm that commerce data and audit data agree.

Steps:

1. COPY the order totals for the period to a file.
2. COPY the completed payment totals for the period to a file.
3. COPY the refund totals for the period to a file.
4. COMPARE the three files.
5. FIND the cause of all differences.
6. EXAMINE the audit log for writes without approval.
7. RECORD the results and keep them with finance records.

---

## 8. Fault isolation

### 8.1 General fault procedure

1. RECORD the exact error message.
2. RECORD the command, database path, and engine version.
3. REPEAT the read operation to confirm the fault.
4. DO NOT REPEAT a failed write with `--apply` until you know the cause.
5. REFER TO Table 1. Then CONTACT support if the fault remains.

Table 1 — Fault isolation

| Symptom                         | Probable cause                               | Action                                                             |
| ------------------------------- | -------------------------------------------- | ------------------------------------------------------------------ |
| `store.db` not found            | Wrong folder or path                         | CHECK the path. ENTER the correct `--db <path>`.                   |
| Write shows preview only        | `--apply` is missing                         | ADD `--apply` only if you intend a change.                         |
| Unrecognized flag error         | Strict parsing does not accept unknown flags | REMOVE the flag. PUT detail in the request text.                   |
| Payment stays pending           | Funds not confirmed                          | DO NOT mark completed. CONFIRM funds first.                        |
| Stock quantity is wrong         | Incorrect adjustment                         | EXAMINE the audit log. ENTER a new adjustment with reason.         |
| Agent can create promotions     | Wrong policy file                            | STOP the server. INSTALL the strict operator policy.               |
| MCP client cannot connect       | Wrong port or host                           | CHECK port 8090 and `--host`. TEST again with a read.              |
| HTTP 4xx on old client          | Protocol mismatch                            | REMOVE `--strict-protocol` or INSTALL a new version of the client. |
| Totals show fractions of a cent | Float used for money                         | STOP. USE decimal strings (for example, `"29.99"`).                |
| Database locked                 | Concurrent writer                            | WAIT. STOP other writers. REPEAT the operation.                    |

### 8.2 Data to send to support

SEND these items with each fault report:

- Engine version (for example, `1.37.0`).
- Full command text (hide secrets).
- Database path and backend type (SQLite or PostgreSQL).
- Exact error text and time.
- Relevant audit log entries.

---

## 9. Approved nouns and abbreviations

Use these nouns consistently. Do not substitute synonyms.

- store database — the single SQLite file (default `./store.db`).
- customer — a buyer record with email and name.
- product — a catalog item with name and price.
- SKU — stock keeping unit; the inventory identifier. Define on first use.
- cart — the pre-order container for items, address, and payment data.
- checkout — the action that converts a cart to an order.
- order — the confirmed purchase with order ID and order number.
- payment — the funds record for an order.
- refund — the return of funds to the customer.
- shipment — the dispatch record with shipment number and tracking number.
- return — the receipt of goods back from the customer.
- coupon — a redeemable promotion code.
- gift card — prepaid value with a redemption code.
- store credit — customer balance held by the store.
- audit log — the append-only record of store changes.
- quarantine — a separate location for damaged goods. Do not sell quarantined goods.
- serviceable — fit for sale and free of damage.
- MCP — Model Context Protocol. Define on first use.
- CLI — command line interface. Define on first use.

---

## 10. STE compliance checklist for future authors

Before you add text to this manual:

1. COUNT the words in each new sentence.
2. KEEP procedures to 20 words per sentence maximum.
3. KEEP descriptions to 25 words per sentence maximum.
4. START each step with an approved verb.
5. USE one instruction per sentence.
6. USE the same noun for the same object.
7. REPLACE non-approved verbs (`get`, `handle`, `process` alone) with precise verbs.
8. EXPAND each abbreviation on first use in each procedure.
9. TEST each new procedure on a clean store database.
10. RECORD the test result with engine version and date.

---

## 11. References

- Engine README and QUICKSTART in the repository root.
- Getting Started: `src/getting-started.md`.
- CLI Quickstart: `src/standalone-quickstart.md`.
- Tool catalog (authoritative): `../cli/docs/TOOLS.md`.
- Strict policy example: `../kernel/examples/strict-policy.json`.
- API references: `src/api/`.
- Audit implementation: `../crates/stateset-db/src/audit.rs`.

---

_End of manual SSE-ICOMMERCE-STE-001 Issue 001._
