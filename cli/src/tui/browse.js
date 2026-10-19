import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const DOMAINS = {
  orders: {
    search: ['id', 'order_number', 'customer_id', 'status'],
    sort: ['order_date DESC, id DESC', 'order_number COLLATE NOCASE, id'],
    relations: ['customer_id'],
    get: 'orders',
  },
  customers: {
    search: ['id', 'first_name', 'last_name', 'email'],
    sort: ['created_at DESC, id DESC', 'last_name COLLATE NOCASE, first_name COLLATE NOCASE, id'],
    relations: [],
    get: 'customers',
  },
  payments: {
    search: ['id', 'payment_number', 'order_id', 'customer_id', 'status'],
    sort: ['created_at DESC, id DESC', 'payment_number COLLATE NOCASE, id'],
    relations: ['order_id', 'customer_id'],
    get: 'payments',
  },
  inventory_items: {
    search: ['sku', 'name'],
    sort: ['id DESC', 'sku COLLATE NOCASE, id'],
    relations: [],
    get: 'inventory',
  },
};

// SQL selects identifiers only from the private engine snapshot. The native
// engine returns records and exact stock/money. No source connection is used.
export async function browseStore(commerce, database, payload, snapshot) {
  const Database = require('better-sqlite3');
  const db = new Database(database, { readonly: true, fileMustExist: true });
  snapshot.matched = {};
  snapshot.queries = { ...payload.queries };
  snapshot.sorts = { ...payload.sorts };
  snapshot.relation = payload.relation || null;
  try {
    for (const [table, spec] of Object.entries(DOMAINS)) {
      const field = table === 'inventory_items' ? 'inventory' : table;
      try {
        const query = String(payload.queries?.[field] || '').slice(0, 120);
        const sort = payload.sorts?.[field] === 1 ? 1 : 0;
        const conditions = [],
          values = [];
        if (query) {
          conditions.push(
            '(' + spec.search.map((column) => `${column} LIKE ? ESCAPE '\\'`).join(' OR ') + ')',
          );
          const escaped = '%' + query.replace(/[\\%_]/g, '\\$&') + '%';
          values.push(...spec.search.map(() => escaped));
        }
        const relation = payload.relation;
        if (relation?.field === field) {
          if (!spec.relations.includes(relation.column) || typeof relation.id !== 'string')
            throw new Error('Unsupported related-record query');
          conditions.push(`${relation.column} = ?`);
          values.push(relation.id);
        }
        const where = conditions.length ? ' WHERE ' + conditions.join(' AND ') : '';
        const total = db
          .prepare(`SELECT COUNT(*) AS count FROM ${table}${where}`)
          .get(...values).count;
        snapshot.matched[field] = total;
        if (field !== 'inventory' && !query && sort === 0 && relation?.field !== field) continue;
        const page = Math.max(
          0,
          Math.min(Number(payload.pages?.[field]) || 0, Math.max(0, Math.ceil(total / 50) - 1)),
        );
        snapshot.pages[field] = page;
        const identifiers = db
          .prepare(
            `SELECT ${field === 'inventory' ? 'sku, name' : 'id'} FROM ${table}${where} ORDER BY ${spec.sort[sort]} LIMIT 50 OFFSET ?`,
          )
          .all(...values, page * 50);
        snapshot[field] = await Promise.all(
          identifiers.map(async (row) => {
            if (field !== 'inventory') return commerce[spec.get].get(row.id);
            const stock = await commerce.inventory.getStock(row.sku);
            const variant = await commerce.products.getVariantBySku(row.sku);
            return {
              ...row,
              available: stock?.totalAvailable ?? null,
              onHand: stock?.totalOnHand ?? null,
              allocated: stock?.totalAllocated ?? null,
              priceExact: variant?.priceExact ?? null,
              productId: variant?.productId ?? null,
            };
          }),
        );
        snapshot[field] = snapshot[field].filter(Boolean);
        if (field === 'inventory')
          snapshot.counts.inventory = db
            .prepare('SELECT COUNT(*) AS count FROM inventory_items')
            .get().count;
      } catch (error) {
        snapshot.errors.push({ section: field + 'Browser', message: error.message });
        // Never label the old low-stock subset as a successful full inventory.
        snapshot[field] = [];
        snapshot.matched[field] = null;
      }
    }
  } finally {
    db.close();
  }
  return snapshot;
}
