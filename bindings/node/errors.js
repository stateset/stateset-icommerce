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

/**
 * Prototypes that must never be treated as native API surfaces.
 *
 * `adopt` sees whatever a native call hands back, and several exports return
 * Buffers (`merkleRoot`, `ed25519Sign`, `aesGcmEncrypt`,
 * `vesX402ComputeSigningHash`, …). Patching the prototype of one of those
 * replaced all 93 own methods of `Buffer.prototype` process-wide, for every
 * library in the host, after a single call into this binding. A commerce
 * binding has no business reaching into the realm's builtins, so it patches
 * only prototypes that came out of the native module.
 *
 * `isBuiltinPrototype` also catches builtins by identity, so this list is
 * belt-and-braces plus the two cases identity cannot reach: `%TypedArray%`,
 * which has no global name, and anything a host has shadowed on `globalThis`.
 */
const inertPrototypes = new WeakSet(
  [
    Object.prototype,
    Array.prototype,
    Function.prototype,
    Promise.prototype,
    Date.prototype,
    RegExp.prototype,
    Error.prototype,
    Map.prototype,
    Set.prototype,
    WeakMap.prototype,
    WeakSet.prototype,
    ArrayBuffer.prototype,
    DataView.prototype,
    // `%TypedArray%.prototype` — the shared base every typed array (and every
    // Buffer) inherits from. It has no global binding, so only this entry
    // keeps it out.
    Object.getPrototypeOf(Uint8Array.prototype),
    Uint8Array.prototype,
    Int8Array.prototype,
    Uint8ClampedArray.prototype,
    Int16Array.prototype,
    Uint16Array.prototype,
    Int32Array.prototype,
    Uint32Array.prototype,
    Float32Array.prototype,
    Float64Array.prototype,
    BigInt64Array.prototype,
    BigUint64Array.prototype,
    typeof Buffer === 'function' ? Buffer.prototype : null,
    typeof SharedArrayBuffer === 'function' ? SharedArrayBuffer.prototype : null,
  ].filter((prototype) => prototype !== null && typeof prototype === 'object'),
)

/**
 * Whether `prototype` belongs to a JavaScript builtin rather than to the
 * native module.
 *
 * A builtin's `prototype.constructor` is the global of the same name, which is
 * exactly what a napi class's constructor is not: nothing this module exports
 * is reachable as `globalThis[name]`. Anything that refuses to answer the
 * question is treated as a builtin too — being conservative here costs at most
 * some undecorated error messages, while being wrong the other way mutates the
 * host's realm.
 */
function isBuiltinPrototype(prototype) {
  if (inertPrototypes.has(prototype)) return true
  let constructor
  try {
    constructor = prototype.constructor
  } catch {
    return true
  }
  if (typeof constructor !== 'function') return false
  const name = constructor.name
  if (typeof name !== 'string' || name === '') return false
  try {
    return globalThis[name] === constructor
  } catch {
    return true
  }
}

/** napi's fixed text when an async task panics with a non-`&str` payload. */
const ASYNC_PANIC_TEXT = 'Panic in async function'

/**
 * Turn one thrown value into its decoded form. Idempotent, and a no-op for
 * anything that is not an enveloped native error.
 *
 * `fromAsync` says the value arrived as a promise rejection. That matters for
 * panic detection: `napi::tokio_runtime::execute_tokio_future` watches every
 * spawned task and rejects with `Status::GenericFailure` carrying the raw panic
 * payload, so an async rejection that is a `GenericFailure` *without* an
 * envelope is a contained panic — every deliberate error the binding raises
 * carries an envelope. A synchronous `#[napi]` fn has no such watchdog (an
 * unwind out of its `extern "C"` shim aborts), so the same shape arriving
 * synchronously is a napi-internal failure, not a panic, and is left alone.
 */
function decorate(error, fromAsync = false) {
  if (error === null || typeof error !== 'object' || error[DECORATED]) return error
  const raw = error.message
  // Fast bail: an envelope is always a JSON object.
  if (typeof raw !== 'string' || raw.charCodeAt(0) !== 0x7b) {
    if (
      typeof raw === 'string' &&
      error.code === 'GenericFailure' &&
      (fromAsync || raw === ASYNC_PANIC_TEXT)
    ) {
      return brand(error, {
        code: 'INTERNAL_PANIC',
        // Keep napi's payload verbatim — it is the panic message, or the fixed
        // text when the payload was a `String` rather than a `&'static str`.
        message: raw,
        details: { httpStatus: 500, contained: 'napi-async-watchdog' },
      })
    }
    return error
  }

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

  return brand(error, envelope)
}

/** Apply a decoded envelope to the error object, in place. */
function brand(error, envelope) {
  const raw = error.message
  error.napiStatus = error.code
  error.code = envelope.code
  error.message = envelope.message
  if (envelope.details !== undefined) error.details = envelope.details
  // The stack was captured with the envelope as its first line; rewrite that
  // line so a logged stack reads like the sentence it always did.
  // (a function replacement, so a `$&` in the message is not a substitution)
  if (typeof error.stack === 'string' && envelope.message !== raw) {
    error.stack = error.stack.replace(raw, () => envelope.message)
  }
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
    isBuiltinPrototype(prototype)
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
      throw decorate(error, true)
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
      // non-writable data property, and that is exactly how napi defines static
      // methods. `shadowStatics` below handles those; here we must hand back
      // the real function or the engine throws a TypeError.
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

/**
 * napi defines static class methods non-configurable and non-writable, so a
 * `Proxy` is forbidden from reporting wrapped versions of them (the engine
 * throws a TypeError rather than allowing the substitution). Without this, a
 * throwing static — `Tax.getUsStateInfo` and friends all return `Result` —
 * would hand the caller the raw JSON envelope as its message.
 *
 * The fix is a stand-in function object we own. Unlike a `class`, a plain
 * function's `prototype` is writable, so pointing it at the native prototype
 * keeps `instanceof` working for instances the native module hands back, while
 * the statics live as ordinary configurable own properties that *can* be
 * wrapped.
 */
function shadowStatics(proxied, target) {
  const staticKeys = Object.getOwnPropertyNames(target).filter((key) => {
    if (key === 'length' || key === 'name' || key === 'prototype') return false
    const descriptor = Object.getOwnPropertyDescriptor(target, key)
    return Boolean(descriptor) && typeof descriptor.value === 'function'
  })
  if (staticKeys.length === 0) return proxied

  const shim = function (...args) {
    return Reflect.construct(proxied, args, new.target === undefined ? shim : new.target)
  }
  shim.prototype = target.prototype
  Object.defineProperty(shim, 'name', { value: target.name, configurable: true })
  for (const key of staticKeys) {
    Object.defineProperty(shim, key, {
      value: wrapCallable(target[key]),
      writable: true,
      enumerable: true,
      configurable: true,
    })
  }
  return shim
}

/** Wrap every export of the native binding. */
function wrapNativeExports(native) {
  const wrapped = Object.create(null)
  for (const key of Object.keys(native)) {
    const value = native[key]
    const proxied = wrapExport(value)
    wrapped[key] = typeof value === 'function' ? shadowStatics(proxied, value) : proxied
  }
  return wrapped
}

module.exports = { decorate, wrapNativeExports }
