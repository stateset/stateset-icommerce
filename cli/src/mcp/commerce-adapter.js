// Adapt the embedded `Commerce` instance into a shape the MCP tool
// handlers can consume directly.
//
// The bound Rust→JS bindings expose API surfaces (customers, orders,
// inventory, …) as prototype getters: `commerce.customers` lazily
// constructs the `CustomersApi` on each access. Tool handlers expect
// each surface to be reachable both as a property AND as a callable
// (so they can invoke nested methods without re-binding `this`).
//
// This module owns three helpers, all pure (no closure or runtime deps):
//   - `createCallableApiAccessor(resolveValue)` — a Proxy factory that
//     makes any getter-backed API both indexable and invocable.
//   - `adaptCommerceForTools(commerce)` — walks the prototype chain,
//     hoists every method/getter into an own-property on a shallow
//     clone, and wraps getters in the callable accessor.
//   - `extendCommerceWithApis(commerce, apis)` — adds extra APIs (e.g.
//     A2A, treasury) onto a `Commerce` clone without mutating the
//     original prototype chain.
//
// Extracted from mcp-server.js. Single call site in
// `createStatesetMcpServer`:
//   `extendCommerceWithApis(adaptCommerceForTools(commerce), {...})`

/**
 * Wrap a getter-backed API surface in a Proxy that's both indexable
 * (returns nested methods bound to the API) AND callable
 * (returns the API itself when invoked).
 *
 * @template T
 * @param {() => T} resolveValue - lazily yields the API surface
 * @returns {T} a Proxy that mimics the API
 */
export function createCallableApiAccessor(resolveValue) {
  return new Proxy(
    function accessor() {
      return resolveValue();
    },
    {
      apply() {
        return resolveValue();
      },
      get(target, prop, receiver) {
        if (prop in target) {
          return Reflect.get(target, prop, receiver);
        }
        const api = resolveValue();
        const value = api?.[prop];
        return typeof value === 'function' ? value.bind(api) : value;
      },
      has(_target, prop) {
        const api = resolveValue();
        return prop in (api || {});
      },
      ownKeys() {
        const api = resolveValue();
        return Reflect.ownKeys(api || {});
      },
      getOwnPropertyDescriptor(_target, prop) {
        const api = resolveValue();
        const descriptor = Object.getOwnPropertyDescriptor(api || {}, prop);
        return descriptor ? { ...descriptor, configurable: true } : undefined;
      },
    },
  );
}

/**
 * Hoist every getter and method off a `Commerce` instance's prototype
 * chain onto a shallow clone, wrapping API getters in callable accessors.
 *
 * The MCP tool handlers iterate `Object.entries(commerce)` to discover
 * available APIs; without this hoist they'd see only the Commerce's
 * own-properties (e.g. `db`, `config`) and miss the API getters
 * (`customers`, `orders`, …) that live on the prototype.
 *
 * @template T
 * @param {T} commerce - the source Commerce instance
 * @returns {T} a clone with prototype methods/getters as own-properties
 */
export function adaptCommerceForTools(commerce) {
  if (!commerce || typeof commerce !== 'object') {
    return commerce;
  }

  const adapted = { ...commerce };
  const accessorCache = new Map();
  const seen = new Set(Object.keys(adapted));

  const getAccessor = (name) => {
    if (!accessorCache.has(name)) {
      accessorCache.set(
        name,
        createCallableApiAccessor(() => commerce[name]),
      );
    }
    return accessorCache.get(name);
  };

  for (
    let proto = Object.getPrototypeOf(commerce);
    proto && proto !== Object.prototype;
    proto = Object.getPrototypeOf(proto)
  ) {
    for (const [name, descriptor] of Object.entries(Object.getOwnPropertyDescriptors(proto))) {
      if (name === 'constructor' || seen.has(name)) {
        continue;
      }

      if (typeof descriptor.get === 'function') {
        Object.defineProperty(adapted, name, {
          enumerable: true,
          configurable: true,
          get() {
            return getAccessor(name);
          },
        });
        seen.add(name);
        continue;
      }

      if (typeof descriptor.value === 'function') {
        Object.defineProperty(adapted, name, {
          enumerable: true,
          configurable: true,
          writable: false,
          value: descriptor.value.bind(commerce),
        });
        seen.add(name);
      }
    }
  }

  return adapted;
}

/**
 * Decorate a Commerce instance with extra named APIs without
 * mutating the original. Returns a wrapper that delegates everything
 * else to `commerce` via prototype chain.
 *
 * @template T
 * @param {T} commerce - the source Commerce instance
 * @param {Record<string, unknown>} [apis] - additional named APIs
 * @returns {T & Record<string, unknown>}
 */
export function extendCommerceWithApis(commerce, apis = {}) {
  const base =
    commerce && (typeof commerce === 'object' || typeof commerce === 'function')
      ? commerce
      : Object.prototype;
  const wrapper = Object.create(base);

  for (const [name, value] of Object.entries(apis)) {
    Object.defineProperty(wrapper, name, {
      enumerable: true,
      configurable: true,
      writable: true,
      value,
    });
  }

  return wrapper;
}
