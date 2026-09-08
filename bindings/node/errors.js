'use strict'

/**
 * Unpacks the coded-error envelope the native module puts in `err.message`.
 *
 * `bindings/node/src/errors.rs` throws every failure with a reason of the form
 *
 *     {"code":"NOT_FOUND","message":"Cart not found","details":{"httpStatus":404}}
 *
 * because napi has nowhere else to put a machine code: whatever it is handed
 * lands on `Error.prototype.message`, and `err.code` is reserved for the napi
 * status (`GenericFailure`, `InvalidArg`, …). This module unwraps that envelope
 * once, on the way out, so callers see a normal `Error` with:
 *
 * - `err.code`       — the stable code to branch on (`NOT_FOUND`, `CONFLICT`,
 *                      `VALIDATION`, `INSUFFICIENT_STOCK`, `POLICY_REJECTED`,
 *                      `DATABASE`, `EXTERNAL_SERVICE`, `INTERNAL`,
 *                      `INTERNAL_PANIC`);
 * - `err.message`    — the human sentence, byte-for-byte what it was before;
 * - `err.details`    — `{ httpStatus, invariant? }` for engine failures;
 * - `err.napiStatus` — the napi status the binding threw with.
 *
 * Errors that do not carry an envelope (napi's own argument-conversion errors,
 * for instance) pass through untouched, keeping their napi `code`.
 */

const DECORATED = Symbol('statesetErrorDecorated')

/** Prototypes whose methods have already been wrapped. */
const patchedPrototypes = new WeakSet()

/** Objects that must never be treated as native API surfaces. */
const inertPrototypes = new WeakSet([
  Object.prototype,
  Array.prototype,
  Function.prototype,
  Promise.prototype,
  Date.prototype,
  Error.prototype,
])

/**
 * Turn one thrown value into its decoded form. Idempotent, and a no-op for
 * anything that is not an enveloped native error.
 */
function decorate(error) {
  if (error === null || typeof error !== 'object' || error[DECORATED]) return error
  const raw = error.message
  // Fast bail: an envelope is always a JSON object.
  if (typeof raw !== 'string' || raw.charCodeAt(0) !== 0x7b) return error

  let envelope
  try {
    envelope = JSON.parse(raw)
  } catch {
    return error
  }
  if (
    envelope === null ||
    typeof envelope !== 'object' ||
    typeof envelope.code !== 'string' ||
    typeof envelope.message !== 'string'
  ) {
    return error
  }

  error.napiStatus = error.code
  error.code = envelope.code
  error.message = envelope.message
  if (envelope.details !== undefined) error.details = envelope.details
  // The stack was captured with the envelope as its first line; rewrite that
  // line so a logged stack reads like the sentence it always did.
  // (a function replacement, so a `$&` in the message is not a substitution)
  if (typeof error.stack === 'string') error.stack = error.stack.replace(raw, () => envelope.message)
  Object.defineProperty(error, DECORATED, { value: true, enumerable: false })
  return error
}

/**
 * Wrap the methods and getters a native class exposes on its prototype, so
 * failures from any of them are decoded. Idempotent per prototype.
 */
function patchPrototype(prototype) {
  if (
    prototype === null ||
    typeof prototype !== 'object' ||
    patchedPrototypes.has(prototype) ||
    inertPrototypes.has(prototype)
  ) {
    return
  }
  patchedPrototypes.add(prototype)

  for (const key of Object.getOwnPropertyNames(prototype)) {
    if (key === 'constructor') continue
    const descriptor = Object.getOwnPropertyDescriptor(prototype, key)
    if (!descriptor || !descriptor.configurable) continue

    if (typeof descriptor.value === 'function') {
      if (!descriptor.writable) continue
      descriptor.value = wrapCallable(descriptor.value)
    } else if (typeof descriptor.get === 'function') {
      descriptor.get = wrapCallable(descriptor.get)
    } else {
      continue
    }
    Object.defineProperty(prototype, key, descriptor)
  }
}

/**
 * A value on its way back to the caller. Native calls hand back further native
 * objects (`commerce.orders`, a subscription handle); patch those classes the
 * first time we see one, which covers classes the module never exports.
 */
function adopt(value) {
  if (value === null || typeof value !== 'object') return value
  patchPrototype(Object.getPrototypeOf(value))
  return value
}

/** Decode rejections as well as throws, without forcing sync calls async. */
function settle(result) {
  if (result !== null && typeof result === 'object' && typeof result.then === 'function') {
    return result.then(adopt, (error) => {
      throw decorate(error)
    })
  }
  return adopt(result)
}

/** Wrap a plain function or method (never a constructor). */
function wrapCallable(fn) {
  function wrapper(...args) {
    let result
    try {
      result = fn.apply(this, args)
    } catch (error) {
      throw decorate(error)
    }
    return settle(result)
  }
  Object.defineProperty(wrapper, 'name', { value: fn.name, configurable: true })
  Object.defineProperty(wrapper, 'length', { value: fn.length, configurable: true })
  return wrapper
}

/**
 * Wrap one export. A `Proxy` covers both shapes the native module exports —
 * classes and free functions — without changing identity: `instanceof` keeps
 * working in both directions, and instances handed back to native code are the
 * real ones.
 */
function wrapExport(value) {
  if (typeof value !== 'function') return value
  patchPrototype(value.prototype)

  const statics = new Map()
  return new Proxy(value, {
    apply(target, thisArg, args) {
      let result
      try {
        result = Reflect.apply(target, thisArg, args)
      } catch (error) {
        throw decorate(error)
      }
      return settle(result)
    },
    construct(target, args, newTarget) {
      try {
        return adopt(Reflect.construct(target, args, newTarget))
      } catch (error) {
        throw decorate(error)
      }
    },
    get(target, property, receiver) {
      const found = Reflect.get(target, property, receiver)
      // Static methods live on the class object itself, so wrap them as they
      // are read. Only own string keys — inherited machinery such as
      // `Symbol.hasInstance` must reach the caller untouched so `instanceof`
      // keeps working.
      if (
        typeof found !== 'function' ||
        typeof property !== 'string' ||
        property === 'prototype' ||
        !Object.prototype.hasOwnProperty.call(target, property)
      ) {
        return found
      }
      // A proxy may not report a different value for a non-configurable,
      // non-writable data property; napi defines static methods exactly that
      // way, so those few are handed back undecorated. Their errors still
      // carry the envelope — call `decorate(err)` if you need it unpacked.
      const descriptor = Object.getOwnPropertyDescriptor(target, property)
      if (descriptor && descriptor.configurable === false && descriptor.writable === false) {
        return found
      }
      let wrapped = statics.get(property)
      if (!wrapped) {
        wrapped = wrapCallable(found)
        statics.set(property, wrapped)
      }
      return wrapped
    },
  })
}

/** Wrap every export of the native binding. */
function wrapNativeExports(native) {
  const wrapped = Object.create(null)
  for (const key of Object.keys(native)) {
    wrapped[key] = wrapExport(native[key])
  }
  return wrapped
}

module.exports = { decorate, wrapNativeExports }
