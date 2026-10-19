import { createRequire } from 'node:module';
import { DOMAINS, TABLES, domainById, explorerRequestKey } from './domains.js';

const require = createRequire(import.meta.url);
const quote = (value) => '"' + String(value).replace(/"/g, '""') + '"';
const SENSITIVE =
  /(?:password|secret|token|private.?key|api.?key|signing.?key|credential|card.?number|cvv|cvc)/i;
const CELL_LIMIT = 4096;
const COLUMN_LIMIT = 64;

function specification(db, table) {
  if (!TABLES.includes(table)) throw new Error('Unsupported engine resource');
  if (!db.prepare("SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?").get(table))
    return null;
  const columns = db.prepare(`PRAGMA table_info(${quote(table)})`).all();
  // Never search, transmit or render known credential columns, including in links.
  const visible = columns.filter((c) => !SENSITIVE.test(c.name) && !/BLOB/i.test(c.type));
  const primary = visible
    .filter((c) => c.pk)
    .sort((a, b) => a.pk - b.pk)
    .map((c) => c.name);
  const order = primary.length ? primary : visible.map((c) => c.name);
  return { columns, visible, order, primary };
}

const RELATED_KEYS = {
  customer_id: 'customers',
  order_id: 'orders',
  product_id: 'products',
  cart_id: 'carts',
  payment_id: 'payments',
  supplier_id: 'suppliers',
  invoice_id: 'invoices',
  shipment_id: 'shipments',
  return_id: 'returns',
  warehouse_id: 'warehouses',
  bill_id: 'ap_bills',
  purchase_order_id: 'purchase_orders',
  promotion_id: 'promotions',
  subscription_id: 'subscriptions',
  lot_id: 'lots',
};

function relationships(db, table, cache) {
  const found = [];
  const add = (target, from, to, reverse = false) => {
    if (!TABLES.includes(target) || SENSITIVE.test(from) || SENSITIVE.test(to)) return;
    const targetSpec = cache.get(target);
    if (!targetSpec?.visible.some((c) => c.name === to)) return;
    if (found.some((l) => l.table === target && l.from === from && l.column === to)) return;
    const domain = DOMAINS.find((d) => d.tables.includes(target))?.id;
    found.push({ table: target, from, column: to, domain, reverse });
  };
  for (const target of TABLES) {
    if (!cache.get(target)) continue;
    const keys = db.prepare(`PRAGMA foreign_key_list(${quote(target)})`).all();
    // Composite keys cannot be safely followed with a single equality.
    const single = keys.filter((k) => keys.filter((other) => other.id === k.id).length === 1);
    for (const key of single) {
      const parentPrimary = cache.get(key.table)?.primary || [];
      const referenced = key.to || (parentPrimary.length === 1 ? parentPrimary[0] : null);
      if (target === table && referenced) add(key.table, key.from, referenced);
      if (key.table === table && referenced) add(target, referenced, key.from, true);
    }
    for (const column of cache.get(target).visible) {
      const referenced = RELATED_KEYS[column.name];
      if (!referenced) continue;
      if (target === table) add(referenced, column.name, 'id');
      if (referenced === table) add(target, 'id', column.name, true);
    }
  }
  return found;
}

// This is a persistence inspector, not a second commerce implementation. It
// only SELECTs whitelisted engine tables on the native engine's private copy.
// Amounts remain the original SQLite decimal TEXT; even integer identifiers
// are cast to text, so no JS Number can round an amount or a 64-bit identifier.
export function exploreStore(database, request) {
  const domain = domainById(request.domain);
  if (!domain || !domain.tables.includes(request.table))
    throw new Error('Unsupported engine resource');
  const Database = require('better-sqlite3');
  const db = new Database(database, { readonly: true, fileMustExist: true });
  const result = {
    requestKey: explorerRequestKey({ ...request, mode: 'records', page: 0 }),
    rows: [],
    links: [],
    matched: null,
    page: 0,
    warnings: [],
    unavailable: null,
  };
  try {
    const spec = specification(db, request.table);
    if (!spec) {
      result.unavailable =
        'This engine snapshot does not contain this resource. It may require an optional module or a newer engine.';
      return result;
    }
    if (!spec.visible.length) {
      result.unavailable = 'This resource has no inspectable columns.';
      return result;
    }
    const conditions = [],
      values = [];
    const query = String(request.query || '').slice(0, 120);
    if (query) {
      const escaped = '%' + query.replace(/[\\%_]/g, '\\$&') + '%';
      conditions.push(
        '(' +
          spec.visible
            .map((c) => `CAST(${quote(c.name)} AS TEXT) LIKE ? ESCAPE '\\'`)
            .join(' OR ') +
          ')',
      );
      values.push(...spec.visible.map(() => escaped));
    }
    if (request.relation) {
      const { column, value } = request.relation;
      if (
        !spec.visible.some((c) => c.name === column) ||
        typeof value !== 'string' ||
        value.length > CELL_LIMIT
      )
        throw new Error('Unsupported record relationship');
      conditions.push(`${quote(column)} = ?`);
      values.push(value);
    }
    const where = conditions.length ? ' WHERE ' + conditions.join(' AND ') : '';
    result.matched = db
      .prepare(`SELECT COUNT(*) AS count FROM ${quote(request.table)}${where}`)
      .get(...values).count;
    const requestedPage =
      Number.isSafeInteger(request.page) && request.page >= 0 ? request.page : 0;
    result.page = Math.min(requestedPage, Math.max(0, Math.ceil(result.matched / 50) - 1));
    result.requestKey = explorerRequestKey({ ...request, mode: 'records', page: result.page });
    const recent = ['created_at', 'order_date', 'updated_at'].find((c) =>
      spec.visible.some((v) => v.name === c),
    );
    const alphabetical = ['name', 'sku', 'email', 'code', 'account_number'].find((c) =>
      spec.visible.some((v) => v.name === c),
    );
    const leading = request.sort === 1 ? alphabetical : recent;
    const order = [
      ...(leading ? [`${quote(leading)}${request.sort === 1 ? ' COLLATE NOCASE' : ' DESC'}`] : []),
      ...spec.order.map(quote),
    ].join(', ');
    const visible = spec.visible.slice(0, COLUMN_LIMIT);
    const selection = visible
      .map((c) => `substr(CAST(${quote(c.name)} AS TEXT), 1, ${CELL_LIMIT}) AS ${quote(c.name)}`)
      .join(', ');
    result.rows = db
      .prepare(
        `SELECT ${selection} FROM ${quote(request.table)}${where} ORDER BY ${order} LIMIT 50 OFFSET ?`,
      )
      .all(...values, result.page * 50);
    if (spec.columns.length > spec.visible.length)
      result.warnings.push('Credential and binary columns are omitted from search and details.');
    if (spec.visible.length > COLUMN_LIMIT)
      result.warnings.push(`Only the first ${COLUMN_LIMIT} columns are shown.`);
    result.warnings.push(
      `Stored fields are text; long values are limited to ${CELL_LIMIT} characters. No derived totals are computed.`,
    );
    if (visible.some((c) => /REAL|FLOAT|DOUBLE/i.test(c.type)))
      result.warnings.push('Legacy REAL columns may already be approximate in storage.');
    const cache = new Map(
      TABLES.map((table) => [table, table === request.table ? spec : specification(db, table)]),
    );
    const relations = relationships(db, request.table, cache);
    result.links = result.rows.map((row) =>
      relations
        .filter(
          (link) =>
            row[link.from] !== null &&
            row[link.from] !== undefined &&
            row[link.from].length < CELL_LIMIT,
        )
        .map((link) => ({
          ...link,
          value: row[link.from],
          name: `${link.reverse ? 'Children' : 'Parent'}: ${link.table} · ${link.column}`,
        })),
    );
    return result;
  } catch (error) {
    result.unavailable = `Resource could not be read: ${error.message}`;
    result.rows = [];
    result.links = [];
    result.matched = null;
    return result;
  } finally {
    db.close();
  }
}
