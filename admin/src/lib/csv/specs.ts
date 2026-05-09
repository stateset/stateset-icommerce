// Canonical CSV column specs for admin entity exports.
//
// Each spec is the single source of truth for column order + naming for
// that entity's CSV export. Stable across releases — adding columns
// appends; removing or renaming requires a versioned schema bump.

import type { Customer, InventoryItem, Order } from '@/lib/types';
import type { CsvColumn } from '@/lib/csv/csv';

export const CUSTOMER_CSV_COLUMNS: CsvColumn<Customer>[] = [
  { key: 'customer_id', accessor: (c) => c.id },
  { key: 'email', accessor: (c) => c.email },
  { key: 'first_name', accessor: (c) => c.firstName ?? '' },
  { key: 'last_name', accessor: (c) => c.lastName ?? '' },
  { key: 'phone', accessor: (c) => c.phone ?? '' },
  { key: 'total_orders', accessor: (c) => c.totalOrders },
  { key: 'total_spent', accessor: (c) => c.totalSpent },
  { key: 'avg_order_value', accessor: (c) => c.averageOrderValue },
  { key: 'last_order_date', accessor: (c) => c.lastOrderDate ?? '' },
  { key: 'tags', accessor: (c) => c.tags.join(';') },
];

export const INVENTORY_CSV_COLUMNS: CsvColumn<InventoryItem>[] = [
  { key: 'sku', accessor: (i) => i.sku },
  { key: 'product_id', accessor: (i) => i.productId },
  { key: 'product_name', accessor: (i) => i.productName },
  { key: 'quantity', accessor: (i) => i.quantity },
  { key: 'reserved', accessor: (i) => i.reservedQuantity },
  { key: 'available', accessor: (i) => i.availableQuantity },
  { key: 'reorder_point', accessor: (i) => i.reorderPoint },
  { key: 'reorder_quantity', accessor: (i) => i.reorderQuantity },
  { key: 'warehouse_id', accessor: (i) => i.warehouseId ?? '' },
  { key: 'location', accessor: (i) => i.location ?? '' },
  { key: 'last_restocked', accessor: (i) => i.lastRestocked ?? '' },
];

/**
 * Same canonical column set the orders page already uses, kept here so
 * future pages can import everything from one place. The orders bulk
 * page continues to import from `lib/orders/csv` for backwards-compat.
 */
export const ORDER_CSV_COLUMNS: CsvColumn<Order>[] = [
  { key: 'order_id', accessor: (o) => o.id },
  { key: 'customer_id', accessor: (o) => o.customerId },
  { key: 'status', accessor: (o) => o.status },
  { key: 'total_amount', accessor: (o) => o.totalAmount },
  { key: 'currency', accessor: (o) => o.currency },
  { key: 'item_count', accessor: (o) => o.items.length },
  { key: 'created_at', accessor: (o) => o.createdAt },
  { key: 'updated_at', accessor: (o) => o.updatedAt },
];
