import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import path from 'path';
import { fileURLToPath } from 'url';
import fs from 'fs';
import os from 'os';

import {
  parseCsvLine,
  parseCsvFile,
  parseCustomerCsv,
  parseProductCsv,
  parseOrderCsv,
} from '../../src/adapters/shopify/csv-parser.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures', 'shopify');

// ---------------------------------------------------------------------------
// parseCsvLine
// ---------------------------------------------------------------------------

describe('parseCsvLine', () => {
  it('parses simple unquoted fields', () => {
    const result = parseCsvLine('a,b,c');
    assert.deepStrictEqual(result, ['a', 'b', 'c']);
  });

  it('parses quoted fields', () => {
    const result = parseCsvLine('"hello","world"');
    assert.deepStrictEqual(result, ['hello', 'world']);
  });

  it('handles commas inside quoted fields', () => {
    const result = parseCsvLine('"one, two",three');
    assert.deepStrictEqual(result, ['one, two', 'three']);
  });

  it('handles escaped double quotes inside quoted fields', () => {
    const result = parseCsvLine('"say ""hello""",end');
    assert.deepStrictEqual(result, ['say "hello"', 'end']);
  });

  it('handles empty fields', () => {
    const result = parseCsvLine('a,,c,');
    assert.deepStrictEqual(result, ['a', '', 'c', '']);
  });

  it('handles single field', () => {
    const result = parseCsvLine('only');
    assert.deepStrictEqual(result, ['only']);
  });

  it('handles empty string', () => {
    const result = parseCsvLine('');
    assert.deepStrictEqual(result, ['']);
  });
});

// ---------------------------------------------------------------------------
// parseCsvFile
// ---------------------------------------------------------------------------

describe('parseCsvFile', () => {
  it('parses a CSV file into objects keyed by headers', async () => {
    const tmpFile = path.join(os.tmpdir(), `csv-test-${Date.now()}.csv`);
    fs.writeFileSync(tmpFile, 'Name,Age\nAlice,30\nBob,25\n');

    const batches = [];
    for await (const batch of parseCsvFile(tmpFile, 10)) {
      batches.push(batch);
    }

    fs.unlinkSync(tmpFile);

    assert.equal(batches.length, 1);
    assert.equal(batches[0].records.length, 2);
    assert.equal(batches[0].records[0].Name, 'Alice');
    assert.equal(batches[0].records[0].Age, '30');
    assert.equal(batches[0].records[1].Name, 'Bob');
  });

  it('yields multiple batches for large files', async () => {
    const tmpFile = path.join(os.tmpdir(), `csv-batch-${Date.now()}.csv`);
    let content = 'Id,Value\n';
    for (let i = 0; i < 10; i++) {
      content += `${i},val${i}\n`;
    }
    fs.writeFileSync(tmpFile, content);

    const batches = [];
    for await (const batch of parseCsvFile(tmpFile, 3)) {
      batches.push(batch);
    }

    fs.unlinkSync(tmpFile);

    assert.equal(batches.length, 4); // 3+3+3+1
    assert.equal(batches[0].records.length, 3);
    assert.equal(batches[0].page, 1);
    assert.equal(batches[3].records.length, 1);
  });

  it('handles empty file (header only)', async () => {
    const tmpFile = path.join(os.tmpdir(), `csv-empty-${Date.now()}.csv`);
    fs.writeFileSync(tmpFile, 'Id,Value\n');

    const batches = [];
    for await (const batch of parseCsvFile(tmpFile, 10)) {
      batches.push(batch);
    }

    fs.unlinkSync(tmpFile);
    assert.equal(batches.length, 0);
  });
});

// ---------------------------------------------------------------------------
// parseCustomerCsv
// ---------------------------------------------------------------------------

describe('parseCustomerCsv', () => {
  it('parses Shopify customer CSV fixture', async () => {
    const filePath = path.join(fixturesDir, 'customers.csv');
    const batches = [];
    for await (const batch of parseCustomerCsv(filePath, 100)) {
      batches.push(batch);
    }

    assert.equal(batches.length, 1);
    const customers = batches[0].records;
    assert.equal(customers.length, 5);

    assert.equal(customers[0].email, 'alice@example.com');
    assert.equal(customers[0].first_name, 'Alice');
    assert.equal(customers[0].last_name, 'Johnson');
    assert.equal(customers[0].phone, '+15551234567');
    assert.equal(customers[0].accepts_marketing, true);
    assert.equal(customers[0].tags, 'vip,wholesale');
  });

  it('handles missing phone as null', async () => {
    const filePath = path.join(fixturesDir, 'customers.csv');
    const batches = [];
    for await (const batch of parseCustomerCsv(filePath, 100)) {
      batches.push(batch);
    }
    const bob = batches[0].records[1];
    assert.equal(bob.phone, null);
    assert.equal(bob.accepts_marketing, false);
  });

  it('preserves customer IDs', async () => {
    const filePath = path.join(fixturesDir, 'customers.csv');
    const batches = [];
    for await (const batch of parseCustomerCsv(filePath, 100)) {
      batches.push(batch);
    }
    assert.equal(batches[0].records[0].id, '1001');
  });
});

// ---------------------------------------------------------------------------
// parseProductCsv
// ---------------------------------------------------------------------------

describe('parseProductCsv', () => {
  it('groups multi-row products by handle', async () => {
    const filePath = path.join(fixturesDir, 'products.csv');
    const batches = [];
    for await (const batch of parseProductCsv(filePath, 100)) {
      batches.push(batch);
    }

    assert.equal(batches.length, 1);
    const products = batches[0].records;
    assert.equal(products.length, 3); // classic-widget, premium-gadget, deluxe-thingamajig

    // classic-widget has 2 variants
    const widget = products.find((p) => p.handle === 'classic-widget');
    assert.ok(widget);
    assert.equal(widget.variants.length, 2);
    assert.equal(widget.variants[0].sku, 'WIDGET-SM');
    assert.equal(widget.variants[1].sku, 'WIDGET-LG');
  });

  it('preserves product fields', async () => {
    const filePath = path.join(fixturesDir, 'products.csv');
    const batches = [];
    for await (const batch of parseProductCsv(filePath, 100)) {
      batches.push(batch);
    }

    const gadget = batches[0].records.find((p) => p.handle === 'premium-gadget');
    assert.ok(gadget);
    assert.equal(gadget.title, 'Premium Gadget');
    assert.equal(gadget.vendor, 'GadgetLab');
    assert.equal(gadget.product_type, 'Gadgets');
    assert.equal(gadget.variants.length, 1);
    assert.equal(gadget.variants[0].sku, 'GADGET-01');
  });

  it('handles variant prices as strings', async () => {
    const filePath = path.join(fixturesDir, 'products.csv');
    const batches = [];
    for await (const batch of parseProductCsv(filePath, 100)) {
      batches.push(batch);
    }

    const widget = batches[0].records.find((p) => p.handle === 'classic-widget');
    assert.equal(widget.variants[0].price, '19.99');
    assert.equal(widget.variants[0].compare_at_price, '24.99');
  });

  it('batches products correctly', async () => {
    const filePath = path.join(fixturesDir, 'products.csv');
    const batches = [];
    for await (const batch of parseProductCsv(filePath, 2)) {
      batches.push(batch);
    }

    assert.equal(batches.length, 2); // 2 + 1
    assert.equal(batches[0].records.length, 2);
    assert.equal(batches[1].records.length, 1);
  });
});

// ---------------------------------------------------------------------------
// parseOrderCsv — using a temp file since the fixture is JSON-based orders
// ---------------------------------------------------------------------------

describe('parseOrderCsv', () => {
  let tmpFile;

  it('parses Shopify order CSV with line items', async () => {
    tmpFile = path.join(os.tmpdir(), `order-csv-${Date.now()}.csv`);
    const csv = [
      'Name,Email,Financial Status,Fulfillment Status,Currency,Total,Customer Id,Shipping Street,Shipping City,Shipping Province,Shipping Zip,Shipping Country,Lineitem name,Lineitem sku,Lineitem quantity,Lineitem price',
      '#1001,alice@example.com,paid,,USD,49.98,1001,123 Main St,Anytown,CA,90210,US,Widget Small,WIDGET-SM,2,19.99',
      '#1001,,,,,,,,,,,,Part,PART-001,2,5.00',
      '#1002,bob@example.com,pending,partial,USD,49.99,1002,456 Oak Ave,Springfield,IL,62701,US,Gadget,GADGET-01,1,49.99',
    ].join('\n');
    fs.writeFileSync(tmpFile, csv);

    const batches = [];
    for await (const batch of parseOrderCsv(tmpFile, 100)) {
      batches.push(batch);
    }

    assert.equal(batches.length, 1);
    const orders = batches[0].records;
    assert.equal(orders.length, 2);

    // First order has 2 line items
    const order1 = orders.find((o) => o.order_number === '1001');
    assert.ok(order1);
    assert.equal(order1.line_items.length, 2);
    assert.equal(order1.line_items[0].sku, 'WIDGET-SM');
    assert.equal(order1.line_items[0].quantity, 2);
    assert.equal(order1.line_items[1].sku, 'PART-001');

    // Second order
    const order2 = orders.find((o) => o.order_number === '1002');
    assert.ok(order2);
    assert.equal(order2.line_items.length, 1);
    assert.equal(order2.financial_status, 'pending');
    assert.equal(order2.fulfillment_status, 'partial');

    fs.unlinkSync(tmpFile);
  });

  it('preserves shipping address', async () => {
    tmpFile = path.join(os.tmpdir(), `order-csv-addr-${Date.now()}.csv`);
    const csv = [
      'Name,Email,Financial Status,Fulfillment Status,Currency,Total,Customer Id,Shipping Address1,Shipping City,Shipping Province,Shipping Zip,Shipping Country,Lineitem name,Lineitem sku,Lineitem quantity,Lineitem price',
      '#1001,a@b.com,paid,,USD,10,1001,123 St,NYC,NY,10001,US,Item,SKU-1,1,10',
    ].join('\n');
    fs.writeFileSync(tmpFile, csv);

    const batches = [];
    for await (const batch of parseOrderCsv(tmpFile, 100)) {
      batches.push(batch);
    }

    const order = batches[0].records[0];
    assert.equal(order.shipping_address.address1, '123 St');
    assert.equal(order.shipping_address.city, 'NYC');
    assert.equal(order.shipping_address.country, 'US');

    fs.unlinkSync(tmpFile);
  });
});
