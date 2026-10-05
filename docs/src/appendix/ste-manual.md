# Operation and Maintenance Manual (ASD-STE100)

Operators and maintenance personnel work from a standalone manual written in
ASD Simplified Technical English (ASD-STE100 Issue 8):

**[SSE-ICOMMERCE-STE-001: iCommerce Operation and Maintenance Manual](https://github.com/stateset/stateset-icommerce/blob/master/docs/ASD_STE100_ICOMMERCE_MANUAL.md)**

It covers, one numbered procedure at a time:

- Installation and initialization (`stateset-init`, MCP servers, governed writes)
- Core operations: customers, products, inventory, carts and checkout, orders,
  payments and refunds, shipments, returns, promotions, tax, and gift cards
- Servicing: backup, restore, and the month-end audit check
- Fault isolation with a symptom-cause-action table

Notes:

- The manual lives at `docs/ASD_STE100_ICOMMERCE_MANUAL.md` in the repository,
  outside this book, so its controlled-language wording stays byte-for-byte
  stable. This page is only a pointer.
- Procedures that use the `stateset` command need a model API key
  (`stateset-config set-key anthropic`). Library use needs no key.
- Writes are preview-only by default; add `--apply` only when you intend to
  change data.
