import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  inventoryExperimentSchema,
  commerceScenarioSchema,
  commerceComparisonSchema,
} from '../../src/simulation/schema.js';
import { checkSimulationInput } from '../../src/simulation/admission.js';
import { SIMULATION_LIMITS as limits } from '../../src/simulation/limits.js';

test('oversized arrays refuse before schema parsing reads their elements', () => {
  const products = Array(20000);
  Object.defineProperty(products, 0, {
    enumerable: true,
    get() {
      throw new Error('Unbounded element traversal reached the getter');
    },
  });
  let result;
  assert.doesNotThrow(() => {
    result = inventoryPlanSchema.safeParse({
      name: 'oversized',
      days: 1,
      startingCash: '1.00',
      products,
    });
  });
  assert.equal(result.success, false);
  assert.match(result.error.issues[0].message, /Simulation input/);
});

test('byte admission counts UTF-8 and JSON escapes at the inclusive 128 KiB boundary', () => {
  const values = [
    'a'.repeat(limits.maxInputBytes - 2),
    'é'.repeat((limits.maxInputBytes - 2) / 2),
    '\\'.repeat((limits.maxInputBytes - 2) / 2),
    '\ud800'.repeat((limits.maxInputBytes - 2) / 6),
    '😀'.repeat((limits.maxInputBytes - 4) / 4) + 'aa',
  ];
  for (const value of values) {
    assert.equal(checkSimulationInput(value).bytes, limits.maxInputBytes);
    assert.throws(() => checkSimulationInput(value + 'a'), /Simulation input exceeds 128 KiB/);
  }
  assert.throws(() => checkSimulationInput({ ['k'.repeat(limits.maxInputBytes)]: 0 }), /128 KiB/);
  assert.throws(() => checkSimulationInput('a'.repeat(limits.maxInputBytes * 4)), /128 KiB/);
});

test('depth and node admission terminate before recursive validation can expand the input', () => {
  let atLimit = null;
  for (let level = 1; level < limits.maxInputDepth; level += 1) atLimit = [atLimit];
  assert.equal(checkSimulationInput(atLimit).depth, limits.maxInputDepth);
  assert.throws(() => checkSimulationInput([atLimit]), /nesting levels/);
  let excessive = atLimit;
  for (let level = 0; level < 1000; level += 1) excessive = [excessive];
  assert.throws(() => checkSimulationInput(excessive), /nesting levels/);
  assert.equal(
    checkSimulationInput(Array(limits.maxInputNodes - 1).fill(0)).nodes,
    limits.maxInputNodes,
  );
  assert.throws(() => checkSimulationInput(Array(limits.maxInputNodes).fill(0)), /data nodes/);
  const wide = Object.fromEntries(
    Array.from({ length: limits.maxInputNodes }, (_, index) => [`k${index}`, 0]),
  );
  assert.throws(() => checkSimulationInput(wide), /data nodes/);
});

test('cycles refuse while repeated references remain ordinary counted data', () => {
  const circular = {};
  circular.self = circular;
  assert.throws(() => checkSimulationInput(circular), /cycles/);
  const shared = { a: 1 };
  assert.deepEqual(checkSimulationInput([shared, shared]), { bytes: 17, nodes: 5, depth: 3 });
  assert.deepEqual(shared, { a: 1 });
  assert.equal(checkSimulationInput(Object.assign(Object.create(null), shared)).bytes, 7);
  assert.equal(checkSimulationInput({ a: undefined }).bytes, 10);
  const sparse = Array(3);
  sparse[0] = undefined;
  sparse[2] = 1;
  assert.equal(checkSimulationInput(sparse).bytes, 13);
});

test('admission never invokes getters, proxies, iterators or serialization hooks', () => {
  let invoked = 0;
  const getter = Object.defineProperty({}, 'name', {
    enumerable: true,
    get() {
      invoked++;
      throw new Error('getter');
    },
  });
  const hidden = Object.defineProperty({}, 'name', {
    get() {
      invoked++;
      throw new Error('hidden getter');
    },
  });
  const element = Object.defineProperty([1], 0, {
    enumerable: true,
    get() {
      invoked++;
      throw new Error('element getter');
    },
  });
  const serializable = {
    toJSON() {
      invoked++;
      throw new Error('toJSON');
    },
  };
  const iterator = Object.assign([1], {
    [Symbol.iterator]() {
      invoked++;
      throw new Error('iterator');
    },
  });
  const proxy = new Proxy(
    {},
    {
      ownKeys() {
        invoked++;
        throw new Error('ownKeys');
      },
      getPrototypeOf() {
        invoked++;
        throw new Error('prototype');
      },
    },
  );
  const revoked = Proxy.revocable([], {});
  revoked.revoke();
  for (const value of [getter, hidden, element, serializable, iterator, proxy, revoked.proxy])
    assert.throws(() => checkSimulationInput(value), /Simulation input/);
  assert.equal(invoked, 0);
  const custom = Object.assign([1], { extra: 2 });
  assert.throws(() => checkSimulationInput(custom), /custom properties/);
  for (const value of [
    new Date(),
    new Map(),
    new Set(),
    Buffer.from('a'),
    { [Symbol('data')]: 1 },
    () => 1,
    Symbol('a'),
    1n,
    NaN,
    Infinity,
  ])
    assert.throws(() => checkSimulationInput(value), /Simulation input/);
});

test('every SDK and MCP execution schema enforces admission before its inner validation', () => {
  const excessive = 'x'.repeat(limits.maxInputBytes);
  for (const [schema, input] of [
    [commerceScenarioSchema, { name: excessive }],
    [commerceComparisonSchema, [{ name: excessive }]],
    [inventoryPlanSchema, { name: excessive }],
    [inventoryPlanComparisonSchema, [{ name: excessive }]],
    [inventoryExperimentSchema, { name: excessive }],
  ]) {
    const result = schema.safeParse(input);
    assert.equal(result.success, false);
    assert.equal(result.error.issues.length, 1);
    assert.match(result.error.issues[0].message, /Simulation input exceeds 128 KiB/);
  }
});

test('aggregate comparison size is checked even when individual scenarios fit', () => {
  const scenario = {
    name: 'bounded',
    inventory: [{ sku: 'A', unitPrice: '1.00', quantity: 1 }],
    steps: Array.from({ length: 50 }, (_, index) => ({
      type: 'place_order',
      id: `order-${index}`,
      items: Array.from({ length: 16 }, () => ({ sku: 'A', quantity: 1 })),
    })),
  };
  const parsed = commerceScenarioSchema.parse(scenario);
  assert.equal(checkSimulationInput(parsed).bytes < limits.maxInputBytes, true);
  const oversizedScenario = {
    ...scenario,
    inventory: [{ ...scenario.inventory[0], sku: 'A'.repeat(64) }],
    steps: scenario.steps.map((step) => ({
      ...step,
      items: step.items.map((item) => ({ ...item, sku: 'A'.repeat(64) })),
    })),
  };
  assert.equal(commerceScenarioSchema.safeParse(oversizedScenario).success, true);
  const combined = commerceComparisonSchema.safeParse([
    oversizedScenario,
    { ...oversizedScenario, name: 'second' },
  ]);
  assert.equal(combined.success, false);
  assert.match(combined.error.issues[0].message, /Simulation input exceeds 128 KiB/);
});
