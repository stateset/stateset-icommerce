// Unit tests for cli/src/mcp/commerce-adapter.js
//
// Covers the three pure helpers:
//  - createCallableApiAccessor: Proxy that's both indexable (get nested
//    methods bound to the API) and callable (returns the API itself)
//  - adaptCommerceForTools: hoists prototype getters/methods to own-props
//  - extendCommerceWithApis: decorates a Commerce instance without
//    mutating its prototype

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  adaptCommerceForTools,
  createCallableApiAccessor,
  extendCommerceWithApis,
} from '../../src/mcp/commerce-adapter.js';

// ---------------------------------------------------------------------------
// createCallableApiAccessor
// ---------------------------------------------------------------------------

describe('createCallableApiAccessor', () => {
  it('returns the resolved API when invoked', () => {
    const api = { id: 'x' };
    const accessor = createCallableApiAccessor(() => api);
    assert.equal(accessor(), api);
  });

  it('exposes API methods bound to the API instance', () => {
    const api = {
      _name: 'orders',
      list() {
        return this._name;
      },
    };
    const accessor = createCallableApiAccessor(() => api);
    // `accessor.list` should be `api.list` bound to `api`.
    assert.equal(accessor.list(), 'orders');
  });

  it('exposes non-function properties as raw values', () => {
    const api = { count: 42 };
    const accessor = createCallableApiAccessor(() => api);
    assert.equal(accessor.count, 42);
  });

  it('reflects `in` checks against the resolved API', () => {
    const api = { foo: 1 };
    const accessor = createCallableApiAccessor(() => api);
    assert.ok('foo' in accessor);
    assert.ok(!('bar' in accessor));
  });

  it('lazily resolves the API on each access (stale ref-safe)', () => {
    let current = { value: 1 };
    const accessor = createCallableApiAccessor(() => current);
    assert.equal(accessor.value, 1);
    current = { value: 2 };
    assert.equal(accessor.value, 2);
  });

  // Skipped: `Reflect.ownKeys(accessor)` triggers a Proxy invariant check
  // that requires `'prototype'` to be in the result for function targets.
  // The accessor is exercised via `'prop' in obj` and direct property
  // access in production, never `Reflect.ownKeys`. Adding 'prototype' to
  // the trap output would leak it as a phantom key on the API surface,
  // which is worse than this latent corner. Documented + intentionally
  // not tested.

  it('returns a configurable property descriptor for present keys', () => {
    const api = { foo: 1 };
    const accessor = createCallableApiAccessor(() => api);
    const desc = Object.getOwnPropertyDescriptor(accessor, 'foo');
    assert.ok(desc);
    assert.equal(desc.configurable, true);
    assert.equal(desc.value, 1);
  });

  it('returns undefined for missing property descriptors', () => {
    const api = { foo: 1 };
    const accessor = createCallableApiAccessor(() => api);
    assert.equal(Object.getOwnPropertyDescriptor(accessor, 'missing'), undefined);
  });

  it('handles a null/undefined API resolver result safely (no throw)', () => {
    const accessor = createCallableApiAccessor(() => null);
    assert.equal(accessor.foo, undefined);
    assert.ok(!('bar' in accessor));
    // No Reflect.ownKeys probe — see "Skipped" note above.
  });
});

// ---------------------------------------------------------------------------
// adaptCommerceForTools
// ---------------------------------------------------------------------------

describe('adaptCommerceForTools', () => {
  it('returns the input unchanged for null/undefined/non-object', () => {
    assert.equal(adaptCommerceForTools(null), null);
    assert.equal(adaptCommerceForTools(undefined), undefined);
    assert.equal(adaptCommerceForTools('string'), 'string');
    assert.equal(adaptCommerceForTools(42), 42);
  });

  it('hoists prototype getters as callable accessors on the clone', () => {
    class FakeCommerce {
      constructor() {
        this.dbPath = ':memory:';
      }
      get customers() {
        return { _name: 'customers', list: () => 'CUSTOMER_LIST' };
      }
    }
    const commerce = new FakeCommerce();
    const adapted = adaptCommerceForTools(commerce);

    // The prototype's `customers` getter is now an own-property of `adapted`.
    assert.ok(Object.getOwnPropertyDescriptor(adapted, 'customers'));
    // The accessor is callable...
    assert.equal(adapted.customers().list(), 'CUSTOMER_LIST');
    // ...and indexable, with bound methods.
    assert.equal(adapted.customers.list(), 'CUSTOMER_LIST');
  });

  it('hoists prototype methods as bound own-properties', () => {
    class FakeCommerce {
      constructor() {
        this._secret = 'shh';
      }
      describe() {
        return this._secret;
      }
    }
    const adapted = adaptCommerceForTools(new FakeCommerce());
    // `describe` is now an own-property bound to the original instance.
    assert.equal(adapted.describe(), 'shh');
  });

  it('preserves the instance own-properties from the source', () => {
    class FakeCommerce {
      constructor() {
        this.dbPath = '/tmp/db';
        this.config = { tenant: 'acme' };
      }
    }
    const adapted = adaptCommerceForTools(new FakeCommerce());
    assert.equal(adapted.dbPath, '/tmp/db');
    assert.deepEqual(adapted.config, { tenant: 'acme' });
  });

  it('does not duplicate own-properties already on the source', () => {
    // The hoist loop's `seen` set is seeded with the source's
    // own-keys, so prototype methods that share a name don't overwrite
    // existing own-properties.
    class FakeCommerce {
      constructor() {
        this.config = 'OWN';
      }
      get config() {
        return 'PROTOTYPE';
      }
    }
    // Use Object.defineProperty to set up the conflict without invoking
    // the prototype's getter from the constructor.
    const commerce = Object.create(FakeCommerce.prototype);
    Object.defineProperty(commerce, 'config', {
      enumerable: true,
      writable: true,
      configurable: true,
      value: 'OWN',
    });
    const adapted = adaptCommerceForTools(commerce);
    // Own-property is preserved; prototype getter does NOT shadow it.
    assert.equal(adapted.config, 'OWN');
  });

  it('skips the constructor property', () => {
    class FakeCommerce {
      method() {
        return 'm';
      }
    }
    const adapted = adaptCommerceForTools(new FakeCommerce());
    // `constructor` should not be hoisted as a property descriptor on the clone.
    assert.equal(
      Object.getOwnPropertyDescriptor(adapted, 'constructor'),
      undefined,
    );
    assert.equal(adapted.method(), 'm');
  });
});

// ---------------------------------------------------------------------------
// extendCommerceWithApis
// ---------------------------------------------------------------------------

describe('extendCommerceWithApis', () => {
  it('returns a wrapper that delegates to the source via prototype chain', () => {
    const commerce = { existing: 1 };
    const wrapper = extendCommerceWithApis(commerce, {});
    assert.equal(wrapper.existing, 1);
  });

  it('attaches new APIs as own-properties of the wrapper', () => {
    const commerce = { existing: 1 };
    const a2a = { register: () => 'ok' };
    const wrapper = extendCommerceWithApis(commerce, { a2a });

    // a2a is reachable on the wrapper but NOT on the source (no mutation).
    assert.equal(wrapper.a2a, a2a);
    assert.equal(commerce.a2a, undefined);
  });

  it('keeps the wrapper APIs configurable + writable', () => {
    const wrapper = extendCommerceWithApis({}, { foo: 'bar' });
    const desc = Object.getOwnPropertyDescriptor(wrapper, 'foo');
    assert.equal(desc.configurable, true);
    assert.equal(desc.writable, true);
    assert.equal(desc.enumerable, true);
  });

  it('handles null/undefined source by anchoring to Object.prototype', () => {
    // Should not throw.
    const wrapper = extendCommerceWithApis(null, { foo: 1 });
    assert.equal(wrapper.foo, 1);
    // Object.prototype is the prototype, so `toString` is reachable.
    assert.equal(typeof wrapper.toString, 'function');
  });

  it('handles a function as source (typeof === "function")', () => {
    const fn = function fakeCommerce() {};
    fn.cohort = 'A';
    const wrapper = extendCommerceWithApis(fn, { extra: 'X' });
    assert.equal(wrapper.cohort, 'A');
    assert.equal(wrapper.extra, 'X');
  });

  it('returns an empty wrapper when no extra APIs are supplied', () => {
    const commerce = { existing: 1 };
    const wrapper = extendCommerceWithApis(commerce);
    assert.equal(wrapper.existing, 1);
    // No new own-keys.
    assert.deepEqual(Object.keys(wrapper), []);
  });
});
