import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const DEFAULT_COMPATIBLE_API_NAMES = ['a2a', 'x402'];

/** @typedef {new (...args: any[]) => unknown} CommerceConstructor */
/** @typedef {{ [key: string | symbol]: any }} AnyRecord */

/** @type {CommerceConstructor | null} */
let commerceCtor = null;

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {unknown} value
 * @returns {boolean}
 */
function isObjectLike(value) {
  return value !== null && (typeof value === 'object' || typeof value === 'function');
}

/**
 * @param {() => any} resolveValue
 * @returns {(...args: any[]) => any}
 */
function createCallableApiAccessor(resolveValue) {
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
 * @param {AnyRecord} commerce
 * @param {string[]} [extraApiNames]
 * @returns {Set<string>}
 */
function collectCompatibleApiNames(commerce, extraApiNames = []) {
  const names = new Set(DEFAULT_COMPATIBLE_API_NAMES);
  for (const name of extraApiNames) {
    if (typeof name === 'string' && name.length > 0) {
      names.add(name);
    }
  }

  for (
    let proto = Object.getPrototypeOf(commerce);
    proto && proto !== Object.prototype;
    proto = Object.getPrototypeOf(proto)
  ) {
    for (const [name, descriptor] of Object.entries(Object.getOwnPropertyDescriptors(proto))) {
      if (name === 'constructor') continue;
      if (typeof descriptor.get === 'function') {
        names.add(name);
      }
    }
  }

  return names;
}

/**
 * @returns {CommerceConstructor}
 */
export function getCommerceCtor() {
  if (commerceCtor) return commerceCtor;

  /** @type {any} */
  let mod;
  try {
    mod = require('@stateset/embedded');
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to load @stateset/embedded. ${message}`);
  }

  const resolvedCtor = mod.Commerce || mod.default?.Commerce || mod.default;
  if (!resolvedCtor) {
    throw new Error('Failed to resolve Commerce export from @stateset/embedded.');
  }

  commerceCtor = resolvedCtor;
  return resolvedCtor;
}

/**
 * @param {unknown} commerce
 * @param {string} name
 * @returns {boolean}
 */
export function hasCommerceApi(commerce, name) {
  try {
    return resolveCommerceApi(commerce, name) !== null;
  } catch {
    return false;
  }
}

/**
 * @template T
 * @param {T} commerce
 * @param {string[]} [extraApiNames]
 * @returns {T}
 */
export function adaptCommerceApis(commerce, extraApiNames = []) {
  if (!isObjectLike(commerce)) {
    return commerce;
  }

  const target = /** @type {AnyRecord} */ (commerce);
  const compatibleApiNames = collectCompatibleApiNames(target, extraApiNames);
  const accessorCache = new Map();

  /** @param {string} name */
  const getAccessor = (name) => {
    if (!accessorCache.has(name)) {
      accessorCache.set(
        name,
        createCallableApiAccessor(() => resolveCommerceApi(target, name)),
      );
    }
    return accessorCache.get(name);
  };

  return /** @type {T} */ (
    new Proxy(target, {
      get(target, prop, receiver) {
        if (
          typeof prop === 'string' &&
          compatibleApiNames.has(prop) &&
          hasCommerceApi(target, prop)
        ) {
          return getAccessor(prop);
        }

        const value = Reflect.get(target, prop, receiver);
        return typeof value === 'function' ? value.bind(target) : value;
      },
      has(target, prop) {
        if (
          typeof prop === 'string' &&
          compatibleApiNames.has(prop) &&
          hasCommerceApi(target, prop)
        ) {
          return true;
        }
        return Reflect.has(target, prop);
      },
      ownKeys(target) {
        const keys = new Set(Reflect.ownKeys(target));
        for (const name of compatibleApiNames) {
          if (hasCommerceApi(target, name)) {
            keys.add(name);
          }
        }
        return [...keys];
      },
      getOwnPropertyDescriptor(target, prop) {
        if (
          typeof prop === 'string' &&
          compatibleApiNames.has(prop) &&
          hasCommerceApi(target, prop)
        ) {
          return {
            configurable: true,
            enumerable: true,
            writable: false,
            value: getAccessor(prop),
          };
        }
        return Reflect.getOwnPropertyDescriptor(target, prop);
      },
    })
  );
}

/**
 * @param {...any} args
 * @returns {unknown}
 */
export function createCommerce(...args) {
  const Commerce = getCommerceCtor();
  return adaptCommerceApis(new Commerce(...args));
}

/**
 * @param {unknown} commerce
 * @param {string} name
 * @returns {any}
 */
export function resolveCommerceApi(commerce, name) {
  if (!commerce || (typeof commerce !== 'object' && typeof commerce !== 'function')) {
    throw new Error(`Commerce instance is required to resolve ${name}`);
  }

  const api = /** @type {AnyRecord} */ (commerce)[name];
  if (api === undefined || api === null) {
    throw new Error(`commerce.${name} API is unavailable`);
  }

  if (typeof api !== 'function') {
    return api;
  }

  try {
    const resolved = api.call(commerce);
    if (resolved !== undefined && resolved !== null) {
      return resolved;
    }
  } catch (error) {
    throw new Error(`Failed to resolve commerce.${name} API: ${messageFromError(error)}`);
  }

  throw new Error(`commerce.${name} API resolved to an empty value`);
}

export const Commerce = /** @type {CommerceConstructor} */ (
  /** @type {unknown} */ (
    new Proxy(function CommerceProxy() {}, {
      construct(_target, args) {
        const CommerceCtor = getCommerceCtor();
        return /** @type {object} */ (adaptCommerceApis(Reflect.construct(CommerceCtor, args)));
      },
      apply(_target, _thisArg, args) {
        const CommerceCtor = getCommerceCtor();
        return adaptCommerceApis(Reflect.construct(CommerceCtor, args));
      },
      get(_target, prop, receiver) {
        const CommerceCtor = getCommerceCtor();
        const value = Reflect.get(CommerceCtor, prop, receiver);
        return typeof value === 'function' ? value.bind(CommerceCtor) : value;
      },
      has(_target, prop) {
        return prop in getCommerceCtor();
      },
      ownKeys() {
        return Reflect.ownKeys(getCommerceCtor());
      },
      getOwnPropertyDescriptor(_target, prop) {
        const descriptor = Object.getOwnPropertyDescriptor(getCommerceCtor(), prop);
        return descriptor ? { ...descriptor, configurable: true } : undefined;
      },
    })
  )
);

export default Commerce;
