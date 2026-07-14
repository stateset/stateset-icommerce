# Data Migration & Import

iCommerce supports bulk import from external commerce platforms. Migrate your existing store data — customers, products, inventory, orders, and fulfillments — into the embedded commerce engine.

## Supported Platforms

| Platform | Method | Entities |
|----------|--------|----------|
| **Shopify** | CSV export + API | Customers, products, orders, fulfillments, inventory |
| **WooCommerce** | REST API | Customers, products, orders, inventory |
| **Stripe** | Webhook sync | Customers, payments, subscriptions, invoices |
| **Custom CSV** | File upload | Any entity type |

## Quick Import

### Shopify

```bash
# Export from Shopify Admin → Settings → Export
# Then import the CSV files:
stateset --apply "import shopify data from ./shopify-exports/"
```

Or via MCP tools:

```javascript
await toolkit.executeTool('import_shopify', {
    csvDir: './shopify-exports/',
    entities: ['customers', 'products', 'orders', 'inventory', 'fulfillments'],
});
```

### WooCommerce

```javascript
await toolkit.executeTool('import_woocommerce', {
    siteUrl: 'https://mystore.example.com',
    consumerKey: 'ck_...',
    consumerSecret: 'cs_...',
    entities: ['customers', 'products', 'orders', 'inventory'],
});
```

### Stripe

```javascript
await toolkit.executeTool('import_stripe', {
    apiKey: 'sk_...',
    entities: ['customers', 'payments', 'subscriptions', 'invoices'],
});
```

## Import Process

```
1. Platform detection → identify source format
2. Entity extraction  → parse CSV/API response into normalized records
3. Deduplication      → check for existing entities by external ID
4. Validation         → schema validation per entity type
5. Creation           → write to SQLite (respects --apply safety model)
6. ID mapping         → store external ID ↔ internal ID mapping
7. Status report      → summary of created, skipped, failed records
```

## Import Status

Track import progress and results:

```javascript
const status = await toolkit.executeTool('import_status', {});
// → {
//     platform: 'shopify',
//     timestamp: '2026-03-17T10:30:45Z',
//     result: {
//         entities: {
//             customers: { processed: 1200, created: 1150, skipped: 45, failed: 5 },
//             products: { processed: 850, created: 830, skipped: 15, failed: 5 },
//             orders: { processed: 5000, created: 4980, skipped: 18, failed: 2 },
//             inventory: { processed: 850, created: 850, skipped: 0, failed: 0 },
//         },
//     },
// }
```

## Shadow Parity Check

After import, verify entity counts match between the source platform and iCommerce:

```javascript
const parity = await toolkit.executeTool('import_shadow_parity', {});
// → [
//     { entityType: 'customers', localCount: 1150, projectedCreates: 1150, match: true },
//     { entityType: 'products', localCount: 830, projectedCreates: 830, match: true },
//     { entityType: 'orders', localCount: 4980, projectedCreates: 4980, match: true },
// ]
```

## ID Mapping

Each imported entity is tracked with a bidirectional ID mapping:

```
External: shopify_customer_12345 ↔ Internal: cust-abc-def-123
```

This enables:
- Webhook reconciliation (Shopify sends updates with their ID → mapped to internal)
- Write-back (push status updates from iCommerce → external platform)
- Deduplication (re-importing skips already-mapped entities)

## Incremental Import

After the initial import, keep data in sync:

1. **Webhook sync** — Real-time updates from the platform. See [Adapter docs](../adapters/overview.md).
2. **Periodic re-import** — Re-run import; already-mapped entities are skipped.
3. **Delta import** — Import only records modified after a timestamp:

```javascript
await toolkit.executeTool('import_shopify', {
    csvDir: './shopify-exports/',
    entities: ['orders'],
    since: '2026-03-15T00:00:00Z',  // only orders created after this date
});
```

## Custom CSV Import

For platforms not directly supported:

```javascript
await toolkit.executeTool('import_csv', {
    entityType: 'customers',
    filePath: './data/customers.csv',
    mapping: {
        'Full Name': 'name',
        'Email Address': 'email',
        'Phone': 'phone',
        'Company': 'company',
        'Created Date': 'createdAt',
    },
});
```

The mapping object translates CSV column headers to iCommerce field names.

## Error Handling

Failed records are logged with the specific error:

```javascript
const errors = await toolkit.executeTool('import_errors', {});
// → [
//     { entity: 'customer', externalId: 'cust-999', error: 'Invalid email format', row: 1234 },
//     { entity: 'product', externalId: 'prod-456', error: 'SKU already exists', row: 567 },
// ]
```

Failed records do not block the rest of the import. Fix the source data and re-import — the deduplication system will skip already-imported records and retry the failed ones.

## MCP Tools

| Tool | Description |
|------|-------------|
| `import_shopify` | Import from Shopify CSV exports |
| `import_woocommerce` | Import from WooCommerce API |
| `import_stripe` | Import from Stripe API |
| `import_csv` | Import from custom CSV with field mapping |
| `import_status` | Check last import status and results |
| `import_shadow_parity` | Verify entity count parity |
| `import_errors` | List failed records with errors |
