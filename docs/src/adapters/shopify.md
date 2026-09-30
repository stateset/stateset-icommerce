# Shopify Adapter

Import Shopify data via CSV exports or the Shopify API, with real-time webhook sync.

## CSV Import

### Export from Shopify

In Shopify Admin: Settings > Export > CSV

Export products, orders, and customers.

### Import

```bash
# Import from CSV files
stateset --apply "import shopify data from csv" --filePath ./exports/

# Or specify individual files
stateset --apply "import shopify products" --filePath ./exports/products.csv
stateset --apply "import shopify orders" --filePath ./exports/orders.csv
stateset --apply "import shopify customers" --filePath ./exports/customers.csv
```

The CSV parser handles Shopify's export format including:
- Product variants (multiple rows per product)
- Order line items
- Customer addresses
- Inventory quantities

## API Import

```bash
stateset --apply "import shopify data" \
    --shopifyDomain mystore.myshopify.com \
    --shopifyAccessToken shpat_...
```

This imports all products, orders, customers, and inventory via the Shopify Admin API.

## Webhook Sync

For real-time updates, configure Shopify webhooks to point at your webhook server:

| Topic | Description |
|-------|-------------|
| `products/create` | New product |
| `products/update` | Product updated |
| `orders/create` | New order |
| `orders/updated` | Order status change |
| `customers/create` | New customer |
| `customers/update` | Customer updated |

## Data Mapping

Product variant prices use the native `priceExact` and `compareAtPriceExact`
fields; order lines use `unitPriceExact`. They remain decimal strings throughout
mapping, including export. Malformed prices and invalid quantities are rejected
instead of silently becoming zero. Native shipping addresses use `line1`,
`line2`, `city`, `state`, `postalCode` and `country`.

Update webhooks now write native customer fields, product fields and variants,
order status/address/note fields, and shipment status/tracking/carrier fields.
Variant IDs are retained when a SKU changes. Omitted fields in partial updates
are preserved. The result lists `updatedFields`; metadata-only events can return
`unchanged`. A missing native write method or rejected update raises an error,
and the ID-map snapshot advances only after the writes succeed. Events older
than the stored `updated_at` snapshot are skipped.

These are field-sync guarantees, not a certification of the complete Shopify
transaction lifecycle. Changed order totals or line economics relative to a
stored snapshot require explicit order editing and financial reconciliation.
Clearing an existing customer phone requires nullable-field support. Product
variant deletion, guest orders, fulfillment creation, inventory location/absolute
quantity semantics, concurrent deliveries, and recovery between native writes
and ID-map commits still require integration work. An external payment status
update does not create a payment transaction or ledger reconciliation.

Native shipment updates enforce the declared lifecycle on SQLite and PostgreSQL:
`pending → processing → ready_to_ship → shipped → in_transit → out_for_delivery → delivered`.
Cancellation is permitted only before handoff to the carrier. Rejected transitions
leave the native shipment and ID-map snapshot unchanged; missing carrier stages
require explicit reconciliation rather than fabricated intermediate events.
Updates accept an optional `expectedVersion` to reject stale writes. Changed
fields increment the shipment version and write an outbox fact in the same
transaction. Reapplying an unchanged patch preserves the version and timestamps
when its optional version precondition is satisfied; stale versions still fail.
Cancellation retains shipment items and tracking history. This does not establish durable
provider idempotency or atomicity between native writes and the ID map.

The current HTTP client still uses REST. Its authenticated requests are confined
to the configured shop and API version, refuse redirects, time out after 15
seconds and detect repeated pagination links. GraphQL migration and live-shop
verification remain tracked in
[`kernel/foundation-program.json`](../../../kernel/foundation-program.json).

Local native integration regressions:

```sh
node --test cli/test/integration/shopify-native-sync.test.js
```

| Shopify Entity | iCommerce Entity |
|---------------|-----------------|
| Product | Product + Variants |
| Order | Order + Line Items |
| Customer | Customer |
| Inventory Level | Inventory Item |

## ID Mapping

The adapter maintains a bidirectional ID map:

```
Shopify Product ID ↔ iCommerce Product ID
Shopify Order ID   ↔ iCommerce Order ID
Shopify Customer ID ↔ iCommerce Customer ID
```

This enables write-back operations to reference the correct external entity.

## Write-Back

```javascript
await toolkit.executeTool('shopify_write_back', {
    type: 'order_status',
    shopifyOrderId: '5678',
    status: 'fulfilled',
    trackingNumber: 'FEDEX-789'
});
```

## CSV Format Details

The Shopify CSV parser handles standard Shopify export format:

### Products CSV

| Column | Required | Description |
|--------|----------|-------------|
| `Handle` | Yes | Product slug (unique identifier) |
| `Title` | Yes | Product name |
| `Variant SKU` | Yes | SKU for each variant |
| `Variant Price` | Yes | Price per variant |
| `Variant Inventory Qty` | No | Stock quantity |
| `Body (HTML)` | No | Product description |
| `Type` | No | Product category |
| `Tags` | No | Comma-separated tags |
| `Option1 Name/Value` | No | Variant attribute (e.g., Size/Large) |
| `Option2 Name/Value` | No | Second variant attribute |

**Variant handling**: Products with multiple variants appear as multiple rows with the same `Handle`. The first row contains the product title; subsequent rows contain only variant-specific data.

### Orders CSV

| Column | Required | Description |
|--------|----------|-------------|
| `Name` | Yes | Order number (e.g., #1001) |
| `Email` | Yes | Customer email |
| `Total` | Yes | Order total |
| `Lineitem name` | Yes | Product name per line |
| `Lineitem quantity` | Yes | Quantity per line |
| `Lineitem price` | Yes | Unit price per line |
| `Financial Status` | Yes | Payment status |
| `Fulfillment Status` | No | Shipping status |

### Encoding

CSV files must be UTF-8 encoded. If you see garbled characters after import, check the file encoding.

## Troubleshooting

### "CSV parse error: unexpected column count"

Shopify CSV exports sometimes include extra commas in description fields. The parser handles quoted fields, but if descriptions contain unescaped quotes, wrap them manually.

### "Duplicate product after re-import"

The ID mapping store (`id-map-store.js`) tracks Shopify ID → iCommerce ID mappings. If you delete and re-create the database but keep the ID map, duplicates won't occur. If the ID map is also deleted, duplicates may appear — use `--force` to overwrite.

## MCP Tools

| Tool | Description |
|------|-------------|
| `configure_shopify` | Set up Shopify adapter (domain, access token) |
| `import_shopify_csv` | Import from CSV exports (products, orders, customers) |
| `import_shopify_api` | Import via Shopify Admin API |
| `shopify_write_back` | Push order status/tracking back to Shopify |
