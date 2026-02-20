/**
 * Shopify CSV Export Parser
 *
 * Parses Shopify's CSV export format using Node.js built-in readline.
 * Handles quoted fields, multi-row product variants, and streaming batches.
 * No external dependencies.
 */

import { createReadStream } from 'fs';
import { createInterface } from 'readline';

// ---------------------------------------------------------------------------
// RFC 4180 CSV line parser
// ---------------------------------------------------------------------------

/**
 * Parse a single CSV line respecting quoted fields.
 * @param {string} line
 * @returns {string[]}
 */
export function parseCsvLine(line) {
  const fields = [];
  let current = '';
  let inQuotes = false;
  let i = 0;

  while (i < line.length) {
    const ch = line[i];

    if (inQuotes) {
      if (ch === '"') {
        if (i + 1 < line.length && line[i + 1] === '"') {
          current += '"';
          i += 2;
        } else {
          inQuotes = false;
          i++;
        }
      } else {
        current += ch;
        i++;
      }
    } else {
      if (ch === '"') {
        inQuotes = true;
        i++;
      } else if (ch === ',') {
        fields.push(current);
        current = '';
        i++;
      } else {
        current += ch;
        i++;
      }
    }
  }

  fields.push(current);
  return fields;
}

/**
 * Parse a CSV file into an array of objects (header row → keys).
 * Yields batches for memory efficiency.
 *
 * @param {string} filePath
 * @param {number} [batchSize=50]
 * @returns {AsyncGenerator<{records: Object[], page: number, hasMore: boolean}>}
 */
export async function* parseCsvFile(filePath, batchSize = 50) {
  const rl = createInterface({
    input: createReadStream(filePath, 'utf-8'),
    crlfDelay: Infinity,
  });

  let headers = null;
  let batch = [];
  let page = 0;

  for await (const line of rl) {
    if (!headers) {
      headers = parseCsvLine(line);
      continue;
    }

    const values = parseCsvLine(line);
    const record = {};
    for (let i = 0; i < headers.length; i++) {
      record[headers[i]] = values[i] || '';
    }
    batch.push(record);

    if (batch.length >= batchSize) {
      page++;
      yield { records: batch, page, hasMore: true };
      batch = [];
    }
  }

  if (batch.length > 0) {
    page++;
    yield { records: batch, page, hasMore: false };
  } else if (page > 0) {
    // Re-emit last batch with hasMore=false (already yielded with hasMore=true)
  }
}

// ---------------------------------------------------------------------------
// Shopify-specific CSV parsers
// ---------------------------------------------------------------------------

/**
 * Parse a Shopify customer CSV export.
 * Shopify customer CSV columns: First Name, Last Name, Email, Phone, Accepts Marketing, Tags, Note
 *
 * @param {string} filePath
 * @param {number} [batchSize=50]
 * @returns {AsyncGenerator<{records: Object[], page: number, hasMore: boolean}>}
 */
export async function* parseCustomerCsv(filePath, batchSize = 50) {
  for await (const batch of parseCsvFile(filePath, batchSize)) {
    const records = batch.records.map((row) => ({
      id: row['Id'] || row['id'] || row['Customer Id'] || null,
      email: row['Email'] || row['email'] || '',
      first_name: row['First Name'] || row['first_name'] || '',
      last_name: row['Last Name'] || row['last_name'] || '',
      phone: row['Phone'] || row['phone'] || null,
      state: 'enabled',
      accepts_marketing:
        (row['Accepts Marketing'] || row['accepts_marketing'] || '').toLowerCase() === 'yes' ||
        (row['Accepts Marketing'] || row['accepts_marketing'] || '').toLowerCase() === 'true',
      tags: row['Tags'] || row['tags'] || '',
      note: row['Note'] || row['note'] || '',
    }));
    yield { records, page: batch.page, hasMore: batch.hasMore };
  }
}

/**
 * Parse a Shopify product CSV export.
 * Products with multiple variants span multiple rows (grouped by Handle).
 *
 * @param {string} filePath
 * @param {number} [batchSize=50]
 * @returns {AsyncGenerator<{records: Object[], page: number, hasMore: boolean}>}
 */
export async function* parseProductCsv(filePath, batchSize = 50) {
  const products = new Map();
  let page = 0;
  let batch = [];

  for await (const csvBatch of parseCsvFile(filePath, 1000)) {
    for (const row of csvBatch.records) {
      const handle = row['Handle'] || row['handle'] || '';
      if (!handle) continue;

      if (!products.has(handle)) {
        products.set(handle, {
          id: row['Id'] || row['id'] || handle,
          title: row['Title'] || row['title'] || '',
          body_html: row['Body (HTML)'] || row['body_html'] || '',
          handle,
          status: (row['Status'] || row['status'] || 'active').toLowerCase(),
          product_type: row['Type'] || row['product_type'] || '',
          vendor: row['Vendor'] || row['vendor'] || '',
          tags: row['Tags'] || row['tags'] || '',
          variants: [],
        });
      }

      const product = products.get(handle);
      const variantTitle = row['Option1 Value'] || row['Variant Title'] || 'Default';
      product.variants.push({
        id: row['Variant Id'] || row['variant_id'] || null,
        title: variantTitle,
        sku: row['Variant SKU'] || row['sku'] || '',
        price: row['Variant Price'] || row['price'] || '0',
        compare_at_price: row['Variant Compare At Price'] || null,
        weight: row['Variant Grams'] || null,
        weight_unit: 'g',
        barcode: row['Variant Barcode'] || null,
        inventory_item_id: null,
      });
    }
  }

  // Yield products in batches
  for (const product of products.values()) {
    batch.push(product);

    if (batch.length >= batchSize) {
      page++;
      yield { records: batch, page, hasMore: true };
      batch = [];
    }
  }

  if (batch.length > 0) {
    page++;
    yield { records: batch, page, hasMore: false };
  }
}

/**
 * Parse a Shopify order CSV export.
 *
 * @param {string} filePath
 * @param {number} [batchSize=50]
 * @returns {AsyncGenerator<{records: Object[], page: number, hasMore: boolean}>}
 */
export async function* parseOrderCsv(filePath, batchSize = 50) {
  const orders = new Map();
  let page = 0;
  let batch = [];

  for await (const csvBatch of parseCsvFile(filePath, 1000)) {
    for (const row of csvBatch.records) {
      const orderName = row['Name'] || row['name'] || '';
      if (!orderName) continue;

      if (!orders.has(orderName)) {
        orders.set(orderName, {
          id: row['Id'] || row['id'] || orderName,
          order_number: orderName.replace(/^#/, ''),
          email: row['Email'] || '',
          customer: { id: row['Customer Id'] || null },
          financial_status: (row['Financial Status'] || 'pending').toLowerCase(),
          fulfillment_status: (row['Fulfillment Status'] || '').toLowerCase() || null,
          currency: row['Currency'] || 'USD',
          total_price: row['Total'] || '0',
          shipping_address: {
            address1: row['Shipping Address1'] || row['Shipping Street'] || '',
            address2: row['Shipping Address2'] || '',
            city: row['Shipping City'] || '',
            province: row['Shipping Province'] || '',
            zip: row['Shipping Zip'] || '',
            country: row['Shipping Country'] || '',
          },
          line_items: [],
        });
      }

      const order = orders.get(orderName);
      if (row['Lineitem name'] || row['lineitem_name']) {
        order.line_items.push({
          id: row['Lineitem Id'] || null,
          name: row['Lineitem name'] || row['lineitem_name'] || '',
          sku: row['Lineitem sku'] || row['lineitem_sku'] || '',
          quantity: parseInt(row['Lineitem quantity'] || row['lineitem_quantity'] || '1', 10),
          price: row['Lineitem price'] || row['lineitem_price'] || '0',
          variant_id: row['Lineitem variant_id'] || null,
          product_id: row['Lineitem product_id'] || null,
        });
      }
    }
  }

  // Yield orders in batches
  for (const order of orders.values()) {
    batch.push(order);

    if (batch.length >= batchSize) {
      page++;
      yield { records: batch, page, hasMore: true };
      batch = [];
    }
  }

  if (batch.length > 0) {
    page++;
    yield { records: batch, page, hasMore: false };
  }
}
