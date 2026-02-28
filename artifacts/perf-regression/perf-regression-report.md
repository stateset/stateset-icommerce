# Perf Regression Report

- Threshold multiplier: `1.0`
- Threshold file: `scripts/ci/perf-thresholds.json`

| Benchmark | Status | Median (ns) | Limit (ns) |
|---|---|---:|---:|
| `sqlite_batch_insert_orders/batch_orders_100` | MISSING | - | 250000000 |
| `sqlite_batch_insert_customers/batch_customers_100` | MISSING | - | 200000000 |
| `api/create_customer` | MISSING | - | 15000000 |
| `api/inventory_get_stock` | MISSING | - | 4000000 |
| `api/create_order_single_item` | MISSING | - | 30000000 |
| `api/analytics_sales_summary` | MISSING | - | 100000000 |

Missing estimates:
- `sqlite_batch_insert_orders/batch_orders_100`
- `sqlite_batch_insert_customers/batch_customers_100`
- `api/create_customer`
- `api/inventory_get_stock`
- `api/create_order_single_item`
- `api/analytics_sales_summary`
