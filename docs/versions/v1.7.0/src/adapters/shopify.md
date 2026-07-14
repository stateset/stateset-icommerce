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
