import { strict as assert } from 'node:assert';
import { test } from 'node:test';

import {
  KNOWN_UNBOUND_ACCESSORS,
  checkBindingAccessorParity,
  moduleToGetter,
  parseBindingGetters,
  parseEmbeddedAccessorModules,
} from '../../src/coverage/binding-accessor-parity.js';

test('parses a non-trivial embedded accessor surface', () => {
  const modules = parseEmbeddedAccessorModules();
  assert.ok(
    modules.length >= 40,
    `expected >= 40 embedded accessor modules, parsed ${modules.length} — the lib.rs parser is likely broken`,
  );
});

test('parses a non-trivial binding getter surface', () => {
  const getters = parseBindingGetters();
  assert.ok(
    getters.length >= 40,
    `expected >= 40 Commerce getters, parsed ${getters.length} — the index.d.ts parser is likely broken`,
  );
  assert.ok(getters.includes('orders'), 'expected an `orders` getter in index.d.ts');
});

test('moduleToGetter converts snake_case to camelCase', () => {
  assert.equal(moduleToGetter('accounts_payable'), 'accountsPayable');
  assert.equal(moduleToGetter('x402'), 'x402');
  assert.equal(moduleToGetter('warehouse'), 'warehouse');
});

test('every embedded accessor domain has a binding getter or a documented exception', () => {
  const { problems } = checkBindingAccessorParity();
  assert.deepEqual(
    problems,
    [],
    `binding accessor parity gate failed:\n- ${problems.join('\n- ')}`,
  );
});

test('exception list fails closed against synthetic drift', () => {
  // A new embedded accessor without a getter must be reported.
  const drift = checkBindingAccessorParity({
    embeddedModules: [...parseEmbeddedAccessorModules(), 'brand_new_domain'],
  });
  assert.deepEqual(drift.missing, ['brand_new_domain']);

  // An exception whose getter now exists must be reported as stale.
  const bound = checkBindingAccessorParity({
    bindingGetters: [...parseBindingGetters(), moduleToGetter(KNOWN_UNBOUND_ACCESSORS[0])],
  });
  assert.deepEqual(bound.staleExceptions, [KNOWN_UNBOUND_ACCESSORS[0]]);
});
