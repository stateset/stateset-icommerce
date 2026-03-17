# Products & Catalog

The product catalog manages SKUs, variants, pricing, categories, and search.

## Creating Products

### Rust

```rust
let product = commerce.products().create(CreateProduct {
    name: "Premium Widget".into(),
    sku: "WIDGET-001".into(),
    price: Decimal::new(2999, 2),  // $29.99
    description: Some("High-quality widget with premium finish".into()),
    category: Some("Widgets".into()),
    ..Default::default()
})?;
```

### Node.js

```javascript
const product = commerce.products.create({
    name: 'Premium Widget',
    sku: 'WIDGET-001',
    price: 29.99,
    description: 'High-quality widget with premium finish',
    category: 'Widgets'
});
```

### Python

```python
product = commerce.products.create(
    name="Premium Widget",
    sku="WIDGET-001",
    price=29.99,
    description="High-quality widget with premium finish",
    category="Widgets"
)
```

### CLI

```bash
stateset --apply "create a product called Premium Widget, SKU WIDGET-001, at $29.99"
```

## Product Variants

Products can have variants — combinations of attributes like size, color, or material — each with their own SKU, price, and inventory:

```javascript
// Create a parent product
const product = commerce.products.create({
    name: 'Widget',
    sku: 'WIDGET-BASE',
    price: 29.99
});

// Add variants
commerce.products.createVariant(product.id, {
    sku: 'WIDGET-RED-S',
    name: 'Widget - Red, Small',
    price: 29.99,
    attributes: { color: 'Red', size: 'Small' }
});

commerce.products.createVariant(product.id, {
    sku: 'WIDGET-RED-L',
    name: 'Widget - Red, Large',
    price: 34.99,   // Large costs more
    attributes: { color: 'Red', size: 'Large' }
});

commerce.products.createVariant(product.id, {
    sku: 'WIDGET-BLUE-S',
    name: 'Widget - Blue, Small',
    price: 29.99,
    attributes: { color: 'Blue', size: 'Small' }
});
```

Each variant has its own inventory tracked independently:

```javascript
commerce.inventory.createItem({ sku: 'WIDGET-RED-S', name: 'Widget Red Small', initialQuantity: 50 });
commerce.inventory.createItem({ sku: 'WIDGET-RED-L', name: 'Widget Red Large', initialQuantity: 30 });
```

## Product Operations

| Operation | Description |
|-----------|-------------|
| `create(params)` | Create a new product |
| `get(id)` | Get product by ID |
| `list()` | List all products |
| `update(id, params)` | Update product details |
| `delete(id)` | Remove a product |
| `createVariant(productId, params)` | Add a variant |
| `listVariants(productId)` | List variants for a product |

## Search

### Keyword Search

```bash
stateset "find products under $50 in the Widgets category"
```

### Semantic Search (Vector)

With `OPENAI_API_KEY` set, hybrid search combines semantic embeddings with BM25 keyword matching:

```bash
stateset "find products similar to wireless earbuds"
```

Programmatic:

```javascript
const results = await toolkit.executeTool('search_products', {
    query: 'noise cancelling headphones',
    limit: 10,
    minPrice: 20,
    maxPrice: 200
});
```

### How Hybrid Search Works

1. **Embedding query**: The search query is converted to a vector embedding via OpenAI
2. **BM25 scoring**: Full-text search scores products by keyword relevance
3. **Vector similarity**: Cosine similarity ranks products by semantic meaning
4. **Fusion**: Scores are combined with configurable weights
5. **Ranking**: Results are sorted by fused score

If `OPENAI_API_KEY` is not set, search falls back to BM25 keyword matching only.

## Custom Fields

Extend products with custom fields for your domain:

```javascript
// Define a custom field
commerce.customObjects.defineField('products', {
    name: 'material',
    type: 'enum',
    values: ['plastic', 'metal', 'wood', 'ceramic']
});

// Set the field on a product
commerce.customObjects.set(product.id, 'products', {
    material: 'metal'
});
```

## Reviews

```javascript
// Create a product review
await toolkit.executeTool('create_review', {
    productId: product.id,
    customerId: customer.id,
    rating: 5,
    title: 'Excellent quality',
    body: 'Best widget I have ever purchased.'
});

// Get average rating
await toolkit.executeTool('get_product_reviews', {
    productId: product.id
});
// → { averageRating: 4.7, reviewCount: 23, reviews: [...] }
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_products` | List all products |
| `get_product` | Get product by ID |
| `create_product` | Create a new product |
| `update_product` | Update product details |
| `delete_product` | Remove a product |
| `search_products` | Semantic + keyword search |
| `create_product_variant` | Add a variant |
| `list_product_variants` | Variants for a product |
| `create_review` | Add a product review |
| `get_product_reviews` | Get reviews and ratings |
