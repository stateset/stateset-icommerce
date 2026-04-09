# Semantic Search

iCommerce includes hybrid semantic + BM25 search across products, customers, orders, and inventory. Queries are matched by meaning — "wireless earbuds" finds "Bluetooth headphones" — not just keywords.

## How It Works

```
Query: "red winter jacket"
          │
          ├─── Semantic: embed query → cosine similarity against product embeddings
          │    (finds "Crimson parka" and "Burgundy down coat")
          │
          ├─── BM25: term frequency matching
          │    (finds "Red Winter Jacket - Women's" exactly)
          │
          └─── Hybrid: weighted combination → ranked results
```

Both scores are normalized and combined:

```
finalScore = (semanticWeight × semanticScore) + (bm25Weight × bm25Score)
```

Default weights: 70% semantic, 30% BM25.

## Requirements

Semantic search requires an OpenAI API key for embedding generation:

```bash
export OPENAI_API_KEY=sk-...
```

The embedding model is `text-embedding-3-small` (1536 dimensions). When `OPENAI_API_KEY` is not set, search falls back to BM25-only mode.

## Searching Products

```javascript
const results = await toolkit.executeTool('vector_search_products', {
    query: 'wireless noise cancelling headphones',
    limit: 10,
});
// → [
//     { productId: 'prod-001', name: 'QuietComfort ANC Headphones', score: 0.92 },
//     { productId: 'prod-002', name: 'Wireless Bluetooth Earbuds Pro', score: 0.87 },
//     { productId: 'prod-003', name: 'Over-Ear Studio Monitors', score: 0.71 },
// ]
```

## Searching Other Entities

```javascript
// Search customers by description
const customers = await toolkit.executeTool('vector_search_customers', {
    query: 'enterprise retail buyers in California',
    limit: 5,
});

// Search orders by context
const orders = await toolkit.executeTool('vector_search_orders', {
    query: 'orders mentioning delayed shipment',
    limit: 10,
});

// Search inventory
const stock = await toolkit.executeTool('vector_search_inventory', {
    query: 'low stock electronic components',
    limit: 20,
});
```

## Indexing

Products are indexed when created or updated. You can also manually trigger indexing:

```javascript
// Index a single product
await toolkit.executeTool('vector_index_product', {
    product_id: 'prod-001',
});

// Bulk re-index all products
await toolkit.executeTool('vector_reindex_products', {});

// Index all entity types
await toolkit.executeTool('vector_reindex_all', {});
```

### What Gets Indexed

| Entity | Indexed Fields |
|--------|---------------|
| Products | Name, description, SKU, category, tags, attributes |
| Customers | Name, email, company, notes, tags |
| Orders | Order number, customer name, item names, notes, status |
| Inventory | SKU, product name, warehouse, location |

## CLI Usage

```bash
# Natural language search via CLI
stateset "find products similar to wireless earbuds"
stateset "search customers like enterprise retail buyers"
stateset "find orders mentioning backorder or late shipment"
```

The CLI automatically routes natural language queries to semantic search when `OPENAI_API_KEY` is available.

## Performance

| Metric | Value |
|--------|-------|
| Embedding generation | ~50ms per entity |
| Search latency | <100ms for 100K products |
| Index storage | ~6KB per product (1536-dim float32) |
| Model | OpenAI `text-embedding-3-small` |

For datasets over 100K products, consider using the `vector_reindex_products` tool during off-peak hours.

## Fallback Behavior

When `OPENAI_API_KEY` is not set:
- Semantic search returns an error explaining the requirement
- BM25 keyword search remains fully operational via standard `list_*` and `search_*` tools
- No API calls are made; all search is local

## MCP Tools

| Tool | Description |
|------|-------------|
| `vector_search_products` | Semantic product search |
| `vector_search_customers` | Semantic customer search |
| `vector_search_orders` | Semantic order search |
| `vector_search_inventory` | Semantic inventory search |
| `vector_index_product` | Index a single product |
| `vector_reindex_products` | Bulk re-index all products |
| `vector_reindex_all` | Re-index all entity types |
| `vector_search_stats` | Index statistics and health |
