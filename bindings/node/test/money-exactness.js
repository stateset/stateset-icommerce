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
 *    The same holds for money that arrives as a bare `f64` **method argument**
 *    rather than as a struct field. Those are invisible to a struct census, so
 *    the fixture carries a second one (`methodArgs`) and the test below parses
 *    every `#[napi]` fn signature to keep it honest.
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

/**
 * Every `#[napi]` fn in `src/lib.rs` that takes at least one `f64` argument, as
 * `{ 'Owner.method': { params: { argName: 'rustType' }, line } }`.
 *
 * A money argument is money whether it is a struct field or a bare parameter,
 * so the census has to see both. `Owner` is the enclosing `impl` block — the
 * class a JavaScript caller reaches through `commerce.credit`, `commerce.tax`
 * and so on — because method names alone collide (three different `complete`s).
 */
function napiFloatMethods() {
  const lines = fs.readFileSync(path.join(ROOT, 'src', 'lib.rs'), 'utf8').split('\n');
  const methods = {};
  let owner = '(module)';

  for (let i = 0; i < lines.length; i += 1) {
    const implBlock = /^impl(?:<[^>]*>)? (\w+)/.exec(lines[i]);
    if (implBlock) owner = implBlock[1];
    if (!/^\s*#\[napi[([\]]/.test(lines[i])) continue;

    // `#[napi]` may be followed by more attributes (`#[allow(...)]`) before the fn.
    let start = i + 1;
    while (start < lines.length && /^\s*#\[/.test(lines[start])) start += 1;
    const signature = /^\s*pub (?:async )?fn (\w+)\s*\(/.exec(lines[start] ?? '');
    if (!signature) continue;

    const params = signatureParams(lines, start);
    if (!Object.values(params).some((type) => type === 'f64' || type === 'Option<f64>')) continue;

    const key = `${owner}.${camel(signature[1])}`;
    assert.ok(
      !(key in methods),
      `two #[napi] fns named ${key} take f64 arguments; the census key is ambiguous`,
    );
    methods[key] = { params, line: start + 1 };
  }

  return methods;
}

/** The `name: Type` pairs of the fn signature starting at `lines[start]`, camelCased. */
function signatureParams(lines, start) {
  let depth = 0;
  let body = '';
  for (let i = start; i < lines.length; i += 1) {
    for (const ch of i === start ? lines[i].slice(lines[i].indexOf('(')) : lines[i]) {
      if (ch === '(') {
        depth += 1;
        if (depth === 1) continue;
      } else if (ch === ')') {
        depth -= 1;
        if (depth === 0) return splitParams(body);
      }
      body += ch;
    }
    body += ' ';
  }
  return {};
}

function splitParams(body) {
  const params = {};
  let depth = 0;
  let current = '';
  const flush = () => {
    const param = /^(\w+)\s*:\s*(.+)$/.exec(current.replace(/\s+/g, ' ').trim());
    if (param) params[camel(param[1])] = param[2];
    current = '';
  };
  for (const ch of body) {
    if ('<([{'.includes(ch)) depth += 1;
    if ('>)]}'.includes(ch)) depth -= 1;
    if (ch === ',' && depth === 0) {
      flush();
      continue;
    }
    current += ch;
  }
  flush();
  return params;
}

/** `{ ClassName: { methodName: 'the whole declaration line' } }` read out of index.d.ts. */
function declaredClasses() {
  const src = fs.readFileSync(path.join(ROOT, 'index.d.ts'), 'utf8');
  const classes = {};
  const classRe = /^export declare class (\w+) \{\n([\s\S]*?)\n\}$/gm;
  for (let m = classRe.exec(src); m !== null; m = classRe.exec(src)) {
    const methods = {};
    const methodRe = /^ {2}(\w+)\((.*)\): /gm;
    for (let f = methodRe.exec(m[2]); f !== null; f = methodRe.exec(m[2])) {
      methods[f[1]] = f[2];
    }
    classes[m[1]] = methods;
  }
  return classes;
}

/** How many entries a `{ owner: [names] }` census section holds. */
function censusSize(section) {
  return Object.entries(section)
    .filter(([key]) => !key.startsWith('$'))
    .reduce((total, [, names]) => total + names.length, 0);
}

// The census tests are only as good as the regexes above: if `rustFloatFields`
// silently matched nothing — a rustfmt change to field indentation would do it —
// every "is this classified?" loop would pass vacuously. The fixture is the
// floor the parser has to clear: it lists every output float there is, so the
// parser must find at least that many. Deleting a field is a fixture edit
// either way (the census below rejects a fixture entry that is no longer an
// f64), so the floor moves with the source instead of drifting from it.
const FIXTURE_OUTPUT_FLOATS = censusSize(fixture.money) + censusSize(fixture.notMoney);
const FIXTURE_METHOD_ARG_FLOATS =
  censusSize(fixture.methodArgs.money) + censusSize(fixture.methodArgs.notMoney);

test('money census: the source parser actually sees the float fields', () => {
  const rust = rustFloatFields();
  const seen = Object.entries(rust)
    .filter(([struct]) => struct.endsWith('Output'))
    .reduce((total, [, fields]) => total + Object.keys(fields).length, 0);
  assert.ok(
    seen >= FIXTURE_OUTPUT_FLOATS,
    `fixture lists ${FIXTURE_OUTPUT_FLOATS} output floats, parser found ${seen}: update the ` +
      'fixture if fields were deliberately removed, otherwise rustFloatFields() has stopped ' +
      'matching src/lib.rs, which would make every other census assertion below pass vacuously',
  );
  assert.ok(Object.keys(declaredInterfaces()).length > 100, 'index.d.ts parser found no interfaces');
});

test('money census: the source parser actually sees the float method arguments', () => {
  const seen = Object.values(napiFloatMethods()).reduce(
    (total, { params }) =>
      total + Object.values(params).filter((t) => t === 'f64' || t === 'Option<f64>').length,
    0,
  );
  assert.ok(
    seen >= FIXTURE_METHOD_ARG_FLOATS,
    `fixture lists ${FIXTURE_METHOD_ARG_FLOATS} f64 method arguments, parser found ${seen}: ` +
      'update the fixture if arguments were deliberately removed, otherwise napiFloatMethods() ' +
      'has stopped matching src/lib.rs and the census below passes vacuously',
  );
  assert.ok(Object.keys(declaredClasses()).length > 40, 'index.d.ts parser found no classes');
});

test('money census: every money method argument has an exact sibling argument', () => {
  const methods = napiFloatMethods();
  const classes = declaredClasses();
  const { money, notMoney } = fixture.methodArgs;
  const { twinSuffix } = fixture;
  const failures = [];

  for (const [key, { params, line }] of Object.entries(methods)) {
    const classified = new Set([...(money[key] ?? []), ...(notMoney[key] ?? [])]);
    for (const [name, type] of Object.entries(params)) {
      if (type !== 'f64' && type !== 'Option<f64>') continue;
      if (!classified.has(name)) {
        failures.push(
          `${key}(${name}) — src/lib.rs:${line} — is an f64 argument but is not listed in ` +
            'test/fixtures/money-fields.json under `methodArgs`: add it under `money` (and ' +
            `give it a trailing \`${name}${twinSuffix}: Option<String>\` argument) or under ` +
            '`notMoney` if it is a quantity, a rate or anything else that is not currency',
        );
      }
    }
  }

  for (const [section, entries] of [
    ['money', money],
    ['notMoney', notMoney],
  ]) {
    for (const [key, names] of Object.entries(entries)) {
      if (key.startsWith('$')) continue;
      const method = methods[key];
      if (!method) {
        failures.push(
          `methodArgs.${section} names ${key}, which takes no f64 argument in src/lib.rs`,
        );
        continue;
      }
      for (const name of names) {
        const type = method.params[name];
        if (type !== 'f64' && type !== 'Option<f64>') {
          failures.push(
            `methodArgs.${section} lists ${key}(${name}), which is no longer an f64 argument`,
          );
        }
      }
    }
  }

  // The point of the whole census: money handed over as a bare float has to
  // have an exact form the caller can reach, exactly as a money struct field does.
  for (const [key, names] of Object.entries(money)) {
    if (key.startsWith('$')) continue;
    const method = methods[key];
    if (!method) continue;
    for (const name of names) {
      const sibling = `${name}${twinSuffix}`;
      if (method.params[sibling] !== 'Option<String>') {
        failures.push(
          `${key}(${name}) is money but has no \`${sibling}: Option<String>\` sibling argument ` +
            `(src/lib.rs:${method.line}); money must be sendable exactly, not only as an f64`,
        );
        continue;
      }
      const [owner, methodName] = key.split('.');
      const declaration = classes[owner]?.[methodName];
      assert.ok(declaration !== undefined, `index.d.ts declares no ${owner}.${methodName}`);
      assert.match(
        declaration,
        new RegExp(`\\b${sibling}\\?: string \\| undefined \\| null`),
        `index.d.ts ${owner}.${methodName} must take an optional ${sibling} string`,
      );
    }
  }

  assert.deepStrictEqual(failures, []);
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

test('credit.checkCredit takes an exact order amount', async (t) => {
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: `credit-${Date.now()}@example.com`,
    firstName: 'Credit',
    lastName: 'Check',
  });
  await commerce.credit.createCreditAccount({ customerId: customer.id, creditLimit: 1000 });

  await t.test('the exact string wins over the float', async () => {
    // A float of 5 would sail through a $1,000 limit; the exact $5,000 must not.
    const check = await commerce.credit.checkCredit(customer.id, 5, '5000');
    assert.strictEqual(check.approved, false);
    assert.match(check.reason, /required \$5000/);
  });

  await t.test('omitting it leaves the float argument working', async () => {
    const check = await commerce.credit.checkCredit(customer.id, 5);
    assert.strictEqual(check.approved, true);
  });

  await t.test('a malformed exact order amount is VALIDATION', async () => {
    await assert.rejects(
      () => commerce.credit.checkCredit(customer.id, 5, 'five thousand'),
      (error) => error.code === 'VALIDATION' && /order amount/.test(error.message),
    );
  });
});

test('credit.adjustCreditLimit takes an exact new limit', async (t) => {
  const commerce = new Commerce(':memory:');
  const customer = await commerce.customers.create({
    email: `limit-${Date.now()}@example.com`,
    firstName: 'Credit',
    lastName: 'Limit',
  });
  await commerce.credit.createCreditAccount({ customerId: customer.id, creditLimit: 1000 });

  await t.test('the exact string wins over the float', async () => {
    // The exact string is the last argument, after `reason`, so the positional
    // callers that predate it keep working unchanged.
    const account = await commerce.credit.adjustCreditLimit(
      customer.id,
      1,
      'exact raise',
      '2500.75',
    );
    assert.strictEqual(account.creditLimitExact, '2500.75');
  });

  await t.test('omitting it leaves the float argument working', async () => {
    const account = await commerce.credit.adjustCreditLimit(customer.id, 1500, 'float raise');
    assert.strictEqual(account.creditLimit, 1500);
    assert.strictEqual(account.creditLimitExact, '1500');
  });

  await t.test('a malformed exact new limit is VALIDATION', async () => {
    await assert.rejects(
      () => commerce.credit.adjustCreditLimit(customer.id, 1, 'bad raise', '2,500.75'),
      (error) => error.code === 'VALIDATION' && /new credit limit/.test(error.message),
    );
  });
});

test('costAccounting.updateAverageCost takes an exact unit cost', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('the exact string wins over the float', async () => {
    // `lastCost` is the unit cost as handed in, so it shows the resolved value
    // without the weighted average in the way.
    const cost = await commerce.costAccounting.updateAverageCost('EXACT-SKU', 1, 1, '19.99');
    assert.strictEqual(cost.lastCostExact, '19.99');
  });

  await t.test('omitting it leaves the float argument working', async () => {
    const cost = await commerce.costAccounting.updateAverageCost('FLOAT-SKU', 2, 12.5);
    assert.strictEqual(cost.lastCost, 12.5);
    assert.strictEqual(cost.lastCostExact, '12.5');
  });

  await t.test('a malformed exact unit cost is VALIDATION', async () => {
    await assert.rejects(
      () => commerce.costAccounting.updateAverageCost('BAD-SKU', 1, 1, 'nineteen'),
      (error) => error.code === 'VALIDATION' && /average cost unit cost/.test(error.message),
    );
  });

  await t.test('the quantity argument stays a float: it is not money', async () => {
    const cost = await commerce.costAccounting.updateAverageCost('QTY-SKU', 3, 2);
    assert.strictEqual(cost.lastCostExact, '2');
  });
});

test('currency.format takes an exact amount', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('the exact string wins over the float', async () => {
    assert.strictEqual(await commerce.currency.format(1, 'USD', '1234.56'), '$1234.56');
  });

  await t.test('omitting it leaves the float argument working', async () => {
    assert.strictEqual(await commerce.currency.format(12.5, 'USD'), '$12.5');
  });

  await t.test('the exact string keeps trailing zeros a float would drop', async () => {
    // A `number` cannot carry scale: 25.00 arrives as 25 and prints as "$25".
    assert.strictEqual(await commerce.currency.format(25.0, 'USD'), '$25');
    assert.strictEqual(await commerce.currency.format(0, 'USD', '25.00'), '$25.00');
  });

  await t.test('a malformed exact amount is VALIDATION', async () => {
    await assert.rejects(
      () => commerce.currency.format(1, 'USD', '$1234.56'),
      (error) => error.code === 'VALIDATION' && /currency format amount/.test(error.message),
    );
  });
});

test('promotions.recordUsage takes an exact discount amount', async (t) => {
  const commerce = new Commerce(':memory:');
  const promotion = await commerce.promotions.create({ name: 'Exact money promotion' });

  await t.test('the exact string wins over the float', async () => {
    const usage = await commerce.promotions.recordUsage(
      promotion.id,
      null,
      null,
      null,
      null,
      1,
      'USD',
      '42.42',
    );
    assert.strictEqual(usage.discountAmountExact, '42.42');
  });

  await t.test('omitting it leaves the float argument working', async () => {
    const usage = await commerce.promotions.recordUsage(
      promotion.id,
      null,
      null,
      null,
      null,
      7.5,
      'USD',
    );
    assert.strictEqual(usage.discountAmount, 7.5);
    assert.strictEqual(usage.discountAmountExact, '7.5');
  });

  await t.test('a malformed exact discount amount is VALIDATION', async () => {
    await assert.rejects(
      () =>
        commerce.promotions.recordUsage(promotion.id, null, null, null, null, 1, 'USD', 'forty two'),
      (error) => error.code === 'VALIDATION' && /promotion discount amount/.test(error.message),
    );
  });
});

test('tax takes exact money on the item calculation and on a new rate', async (t) => {
  const commerce = new Commerce(':memory:');
  // A jurisdiction of its own, so the rates these subtests create cannot stack
  // with each other or with the seeded ones and change what the tax comes to.
  const jurisdiction = await commerce.tax.createJurisdiction({
    name: 'Exactland',
    code: 'ZZ',
    level: 'country',
    countryCode: 'ZZ',
  });

  await t.test('createRate takes exact thresholds and an exact fixed amount', async () => {
    const rate = await commerce.tax.createRate({
      jurisdictionId: jurisdiction.id,
      rate: 0.1,
      name: 'Exact thresholds',
      effectiveFrom: '2020-01-01',
      thresholdMin: 0,
      thresholdMinExact: '100.05',
      thresholdMax: 0,
      thresholdMaxExact: '9999.95',
      fixedAmount: 0,
      fixedAmountExact: '1.25',
    });
    assert.strictEqual(rate.thresholdMinExact, '100.05');
    assert.strictEqual(rate.thresholdMaxExact, '9999.95');
    assert.strictEqual(rate.fixedAmountExact, '1.25');
  });

  await t.test('createRate leaves the float thresholds working when no exact is sent', async () => {
    const rate = await commerce.tax.createRate({
      jurisdictionId: jurisdiction.id,
      rate: 0.1,
      name: 'Float thresholds',
      effectiveFrom: '2020-01-01',
      thresholdMin: 50.5,
    });
    assert.strictEqual(rate.thresholdMin, 50.5);
    assert.strictEqual(rate.thresholdMinExact, '50.5');
    assert.strictEqual(rate.thresholdMax, undefined);
    assert.strictEqual(rate.thresholdMaxExact, undefined);
  });

  await t.test('a malformed exact threshold is VALIDATION', async () => {
    await assert.rejects(
      () =>
        commerce.tax.createRate({
          jurisdictionId: jurisdiction.id,
          rate: 0.1,
          name: 'Bad threshold',
          effectiveFrom: '2020-01-01',
          thresholdMinExact: 'one hundred',
        }),
      (error) => error.code === 'VALIDATION' && /tax threshold min/.test(error.message),
    );
  });

  // One flat ten percent, in a country of its own, so the arithmetic below has
  // exactly one rate in it.
  const taxed = await commerce.tax.createJurisdiction({
    name: 'Tenpercentia',
    code: 'YY',
    level: 'country',
    countryCode: 'YY',
  });
  await commerce.tax.createRate({
    jurisdictionId: taxed.id,
    rate: 0.1,
    name: 'Ten percent',
    effectiveFrom: '2020-01-01',
  });
  const address = { line1: '1 Exact Way', city: 'Ledger', postalCode: '00001', country: 'YY' };

  await t.test('calculateForItem takes an exact unit price', async () => {
    // Ten percent of the exact $100, not of the $1 float sent alongside it.
    assert.strictEqual(await commerce.tax.calculateForItem(1, 1, null, address, '100'), 10);
    assert.strictEqual(await commerce.tax.calculateForItem(100, 1, null, address), 10);
  });

  await t.test('a malformed exact unit price is VALIDATION', async () => {
    await assert.rejects(
      () => commerce.tax.calculateForItem(1, 1, null, address, 'one hundred'),
      (error) => error.code === 'VALIDATION' && /tax unit price/.test(error.message),
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
