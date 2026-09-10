/**
 * Money on the binding surface: exact, and never silently NaN.
 *
 * Two properties are asserted here.
 *
 * 1. **Every money output has an exact twin, and the twin is exact.** A `f64`
 *    cannot hold `19.99`, so a total that a JavaScript caller reads as a number
 *    is already wrong in the last places before it is displayed. Every money
 *    field on an output struct therefore carries a sibling `<field>Exact`
 *    string, rendered from the engine's `Decimal` without a float in the path.
 *    `test/fixtures/money-fields.json` is the census of which fields those are;
 *    the structural test below keeps it honest against `src/lib.rs` and the
 *    generated `index.d.ts`, so a new float money field cannot be added without
 *    either a twin or a deliberate "this is not money" entry.
 *
 * 2. **A money value that cannot be narrowed to `f64` throws instead of
 *    becoming `NaN`.** The binding used to hand JavaScript `NaN` there, which
 *    compares false against every threshold, so a report reading one showed a
 *    blank rather than an error. It now throws `code: 'INTERNAL'` naming the
 *    field. `__testMoneyNotRepresentable` is the probe for that path and is
 *    compiled only with the `test-panic` cargo feature (`npm run build:debug`),
 *    so the probe test skips against a shipping build.
 */

'use strict';

const assert = require('assert');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

const native = require('../index.js');
const { Commerce } = native;

const ROOT = path.resolve(__dirname, '..');
const fixture = JSON.parse(
  fs.readFileSync(path.join(__dirname, 'fixtures', 'money-fields.json'), 'utf8'),
);

function camel(snake) {
  const [head, ...rest] = snake.split('_');
  return head + rest.map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join('');
}

/** `{ StructName: { fieldName: 'f64' | 'Option<f64>' } }` read out of the Rust source. */
function rustFloatFields() {
  const src = fs.readFileSync(path.join(ROOT, 'src', 'lib.rs'), 'utf8');
  const structs = {};
  const structRe = /^pub struct (\w+) \{\n([\s\S]*?)\n\}$/gm;
  for (let m = structRe.exec(src); m !== null; m = structRe.exec(src)) {
    const fields = {};
    const fieldRe = /^ {4}pub (\w+): (f64|Option<f64>),$/gm;
    for (let f = fieldRe.exec(m[2]); f !== null; f = fieldRe.exec(m[2])) {
      fields[camel(f[1])] = f[2];
    }
    if (Object.keys(fields).length > 0) {
      structs[m[1]] = fields;
    }
  }
  return structs;
}

/** `{ InterfaceName: { fieldName: { type, optional } } }` read out of index.d.ts. */
function declaredInterfaces() {
  const src = fs.readFileSync(path.join(ROOT, 'index.d.ts'), 'utf8');
  const interfaces = {};
  const ifaceRe = /^export interface (\w+) \{\n([\s\S]*?)\n\}$/gm;
  for (let m = ifaceRe.exec(src); m !== null; m = ifaceRe.exec(src)) {
    const fields = {};
    const fieldRe = /^ {2}(\w+)(\??): (.+)$/gm;
    for (let f = fieldRe.exec(m[2]); f !== null; f = fieldRe.exec(m[2])) {
      fields[f[1]] = { type: f[3], optional: f[2] === '?' };
    }
    interfaces[m[1]] = fields;
  }
  return interfaces;
}

// The census tests are only as good as the regexes above: if `rustFloatFields`
// silently matched nothing — a rustfmt change to field indentation would do it —
// every "is this classified?" loop would pass vacuously. This is the floor the
// parser has to clear. It is a floor, not an equality, so adding an output float
// is not gated on editing this number; removing the parser's ability to see them
// is.
const MINIMUM_OUTPUT_FLOAT_FIELDS = 185;

test('money census: the source parser actually sees the float fields', () => {
  const rust = rustFloatFields();
  const seen = Object.entries(rust)
    .filter(([struct]) => struct.endsWith('Output'))
    .reduce((total, [, fields]) => total + Object.keys(fields).length, 0);
  assert.ok(
    seen >= MINIMUM_OUTPUT_FLOAT_FIELDS,
    `only ${seen} f64 fields found on *Output structs in src/lib.rs, expected at least ` +
      `${MINIMUM_OUTPUT_FLOAT_FIELDS} — rustFloatFields() has probably stopped matching, ` +
      'which would make every other census assertion below pass vacuously',
  );
  assert.ok(Object.keys(declaredInterfaces()).length > 100, 'index.d.ts parser found no interfaces');
});

test('money census: every float money output field has an exact twin', () => {
  const rust = rustFloatFields();
  const dts = declaredInterfaces();
  const { money, notMoney, twinSuffix } = fixture;
  const failures = [];

  for (const [struct, fields] of Object.entries(rust)) {
    if (!struct.endsWith('Output')) continue;
    const classified = new Set([...(money[struct] ?? []), ...(notMoney[struct] ?? [])]);
    for (const field of Object.keys(fields)) {
      if (!classified.has(field)) {
        failures.push(
          `${struct}.${field} is an f64 on an output struct but is not listed in ` +
            'test/fixtures/money-fields.json — add it under `money` (and give it a ' +
            `\`${field}${twinSuffix}\` twin) or under \`notMoney\` if it is not a currency amount`,
        );
      }
    }
    for (const field of classified) {
      if (!(field in fields)) {
        failures.push(`${struct}.${field} is listed in the fixture but is no longer an f64 field`);
      }
    }
  }

  for (const [struct, fields] of Object.entries(money)) {
    assert.ok(struct in rust, `fixture names ${struct}, which src/lib.rs does not declare`);
    const iface = dts[struct];
    assert.ok(iface, `index.d.ts declares no interface ${struct}`);
    for (const field of fields) {
      const twin = `${field}${twinSuffix}`;
      if (!(twin in iface)) {
        failures.push(`${struct}.${field} is money but index.d.ts has no ${twin}`);
        continue;
      }
      const optional = rust[struct][field] === 'Option<f64>';
      assert.strictEqual(
        iface[twin].type,
        'string',
        `${struct}.${twin} must be a string, not ${iface[twin].type}`,
      );
      assert.strictEqual(
        iface[twin].optional,
        optional,
        `${struct}.${twin} optionality must match ${struct}.${field}`,
      );
    }
  }

  assert.deepStrictEqual(failures, []);
});

test('money census: the float halves are marked deprecated in index.d.ts', () => {
  const src = fs.readFileSync(path.join(ROOT, 'index.d.ts'), 'utf8');
  const missing = [];
  for (const [struct, fields] of Object.entries(fixture.money)) {
    const iface = new RegExp(`^export interface ${struct} \\{\\n([\\s\\S]*?)\\n\\}$`, 'm').exec(src);
    assert.ok(iface, `index.d.ts declares no interface ${struct}`);
    for (const field of fields) {
      const declaration = new RegExp(
        `@deprecated Use the \`${field}${fixture.twinSuffix}\` twin[\\s\\S]{0,120}?\\n  ${field}\\??:`,
      );
      if (!declaration.test(iface[1])) {
        missing.push(`${struct}.${field}`);
      }
    }
  }
  assert.deepStrictEqual(missing, [], 'float money fields must carry the @deprecated JSDoc tag');
});

test('order money survives the round trip exactly', async (t) => {
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: `exact-${Date.now()}@example.com`,
    firstName: 'Exact',
    lastName: 'Money',
  });

  await t.test('0.1 + 0.2 is 0.30, not 0.30000000000000004', async () => {
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        { sku: 'DIME', name: 'Dime', quantity: 1, unitPrice: 0, unitPriceExact: '0.1' },
        { sku: 'DOUBLE', name: 'Double dime', quantity: 1, unitPrice: 0, unitPriceExact: '0.2' },
      ],
    });

    assert.strictEqual(order.items[0].unitPriceExact, '0.1');
    assert.strictEqual(order.items[1].unitPriceExact, '0.2');
    assert.strictEqual(order.totalAmountExact, '0.3');
    // The float half is what a caller reading `totalAmount` has always seen;
    // it is kept, and it is exactly why the exact half is worth having.
    assert.strictEqual(order.items[0].unitPrice + order.items[1].unitPrice, 0.30000000000000004);
  });

  await t.test('a price no f64 can hold is preserved by unitPriceExact', async () => {
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        { sku: 'CENTS', name: 'Cents', quantity: 3, unitPrice: 0, unitPriceExact: '19.99' },
      ],
    });

    assert.strictEqual(order.items[0].unitPriceExact, '19.99');
    assert.strictEqual(order.items[0].totalExact, '59.97');
    assert.strictEqual(order.totalAmountExact, '59.97');
    // 19.99 is not representable in binary floating point. The float half the
    // binding hands back prints as 19.99 only because JavaScript prints the
    // shortest string that round-trips; the value itself is not 19.99, which is
    // what the exact half is for.
    assert.strictEqual(order.items[0].unitPrice.toPrecision(17), '19.989999999999998');
  });

  await t.test('the exact string wins over the float when both are sent', async () => {
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [
        { sku: 'BOTH', name: 'Both', quantity: 1, unitPrice: 5, unitPriceExact: '7.25' },
      ],
    });

    assert.strictEqual(order.items[0].unitPriceExact, '7.25');
    assert.strictEqual(order.totalAmountExact, '7.25');
  });

  await t.test('omitting the exact string leaves the float path unchanged', async () => {
    const order = await commerce.orders.create({
      customerId: customer.id,
      items: [{ sku: 'FLOAT', name: 'Float', quantity: 2, unitPrice: 12.5 }],
    });

    assert.strictEqual(order.items[0].unitPrice, 12.5);
    assert.strictEqual(order.items[0].unitPriceExact, '12.5');
    // The exact half keeps the engine's scale rather than trimming it, so a
    // caller can tell 25.0 (a two-item line at 12.50) from a bare 25.
    assert.strictEqual(order.totalAmountExact, '25.0');
  });
});

test('cart money carries the exact twin through to checkout totals', async () => {
  const commerce = new Commerce(':memory:');
  const cart = await commerce.carts.create({});

  const item = await commerce.carts.addItem(cart.id, {
    sku: 'THIRD',
    name: 'A third of a dollar',
    quantity: 3,
    unitPrice: 0,
    unitPriceExact: '0.33',
  });
  assert.strictEqual(item.unitPriceExact, '0.33');
  assert.strictEqual(item.totalExact, '0.99');

  const updated = await commerce.carts.get(cart.id);
  assert.strictEqual(updated.subtotalExact, '0.99');
  assert.strictEqual(updated.grandTotalExact, '0.99');

  const shipped = await commerce.carts.setShipping(cart.id, {
    shippingAddress: {
      firstName: 'Exact',
      lastName: 'Money',
      line1: '1 Exact Way',
      city: 'Ledger',
      postalCode: '00001',
      country: 'US',
    },
    shippingAmount: 0,
    shippingAmountExact: '4.07',
  });
  assert.strictEqual(shipped.shippingAmountExact, '4.07');
  assert.strictEqual(shipped.grandTotalExact, '5.06');
});

test('a malformed exact money string is rejected as VALIDATION', async (t) => {
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: `malformed-${Date.now()}@example.com`,
    firstName: 'Malformed',
    lastName: 'Money',
  });

  // Anything the exact parser cannot read is the caller's mistake, so it must
  // land as VALIDATION — never as a silent fallback to the float half, which
  // would quietly charge a different price than the one that was sent.
  for (const bad of ['nineteen ninety nine', '19,99', '', '19.99USD', 'NaN', '1e5']) {
    await t.test(`rejects ${JSON.stringify(bad)}`, async () => {
      let thrown;
      try {
        await commerce.orders.create({
          customerId: customer.id,
          items: [
            { sku: 'BAD', name: 'Bad', quantity: 1, unitPrice: 1, unitPriceExact: bad },
          ],
        });
      } catch (error) {
        thrown = error;
      }
      assert.ok(thrown, `${JSON.stringify(bad)} must be rejected, not silently accepted`);
      assert.strictEqual(thrown.code, 'VALIDATION');
      assert.match(thrown.message, /order item unit price/);
    });
  }
});

test('carts.setTax takes an exact tax amount', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('the exact string wins over the float', async () => {
    const cart = await commerce.carts.create({});
    await commerce.carts.addItem(cart.id, {
      sku: 'TAXED',
      name: 'Taxed',
      quantity: 1,
      unitPrice: 0,
      unitPriceExact: '100.00',
    });

    const taxed = await commerce.carts.setTax(cart.id, 0, '8.25');
    assert.strictEqual(taxed.taxAmountExact, '8.25');
    assert.strictEqual(taxed.grandTotalExact, '108.25');
  });

  await t.test('omitting it leaves the float argument working', async () => {
    const cart = await commerce.carts.create({});
    const taxed = await commerce.carts.setTax(cart.id, 1.5);
    assert.strictEqual(taxed.taxAmount, 1.5);
    assert.strictEqual(taxed.taxAmountExact, '1.5');
  });

  await t.test('a malformed exact tax amount is VALIDATION', async () => {
    const cart = await commerce.carts.create({});
    await assert.rejects(
      () => commerce.carts.setTax(cart.id, 0, 'eight point two five'),
      (error) => error.code === 'VALIDATION' && /cart tax amount/.test(error.message),
    );
  });
});

test('an unrepresentable money value throws INTERNAL instead of returning NaN', async (t) => {
  if (typeof native.__testMoneyNotRepresentable !== 'function') {
    t.skip('binary built without the test-panic feature — no money probe compiled in');
    return;
  }

  let thrown;
  try {
    native.__testMoneyNotRepresentable('sales total revenue');
  } catch (error) {
    thrown = error;
  }

  assert.ok(thrown, 'an unrepresentable money value must throw, not return NaN');
  assert.strictEqual(thrown.code, 'INTERNAL');
  assert.match(thrown.message, /money value sales total revenue is not representable as f64/);
});
