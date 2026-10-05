// `structuredClone` is available in the supported Node 20 runtime, but the
// Rust protocol integration test may be run with an older system `node`.
// Keep the reference handler's state isolation intact in that environment as
// well instead of failing the first request at runtime.
export function clone(value) {
  if (typeof globalThis.structuredClone === 'function') return globalThis.structuredClone(value);
  return cloneFallback(value, new WeakMap());
}

function cloneFallback(value, seen) {
  if (value === null || typeof value !== 'object') return value;
  if (value instanceof Date) return new Date(value.getTime());

  const prior = seen.get(value);
  if (prior) return prior;

  if (Array.isArray(value)) {
    const result = [];
    seen.set(value, result);
    for (const item of value) result.push(cloneFallback(item, seen));
    return result;
  }

  if (value instanceof Map) {
    const result = new Map();
    seen.set(value, result);
    for (const [key, item] of value) result.set(cloneFallback(key, seen), cloneFallback(item, seen));
    return result;
  }

  if (value instanceof Set) {
    const result = new Set();
    seen.set(value, result);
    for (const item of value) result.add(cloneFallback(item, seen));
    return result;
  }

  const result = Object.create(Object.getPrototypeOf(value));
  seen.set(value, result);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor?.value !== undefined) descriptor.value = cloneFallback(descriptor.value, seen);
    Object.defineProperty(result, key, descriptor);
  }
  return result;
}
