/**
 * Public entry point for @stateset/embedded.
 *
 * This file is HAND-WRITTEN. The platform-resolution shim that napi generates
 * lives in `native-binding.js`, which is why every build must pass
 * `--js native-binding.js`:
 *
 *     napi build --platform --js native-binding.js
 *
 * A bare `napi build` writes its generated shim to `index.js` and silently
 * overwrites this file, taking the error decoding with it. Both npm scripts
 * pass the flag; if you invoke napi by hand, pass it too.
 *
 * Everything the native module exports passes through `errors.js`, which turns
 * the coded envelope the binding throws into an ordinary `Error` carrying
 * `err.code`, `err.message`, `err.details` and `err.napiStatus`. See
 * `errors.js` for the contract.
 *
 * The re-exports below are written out one per line on purpose: Node's CJS
 * named-export detection is static, and `import { Commerce } from
 * '@stateset/embedded'` only works while they stay that way.
 * `test/error-codes.js` fails if this list drifts from the native binding.
 *
 * `__testPanic*` and `__testMoneyNotRepresentable` are compiled only with the
 * `test-panic` cargo feature (which `npm run build:debug` turns on); in any
 * published build they are `undefined`.
 */

const { wrapNativeExports } = require('./errors.js')
const { loadNativeBinding } = require('./native-loader.js')

const nativeBinding = wrapNativeExports(loadNativeBinding(() => require('./native-binding.js')))

// ---------------------------------------------------------------------------
// JavaScript-side surface layered over the native classes. Type declarations
// for it live in scripts/index-augment.d.ts (appended to index.d.ts on build).
// ---------------------------------------------------------------------------

{
  const proto = nativeBinding.Commerce.prototype

  // Sub-API getters (`commerce.orders`, …) hand back a fresh native object on
  // every access, so `commerce.orders !== commerce.orders`. Memoise per
  // instance; a getter that yields a primitive (`isClosed`) is a live value
  // and stays uncached.
  const subApis = new WeakMap()
  for (const key of Object.getOwnPropertyNames(proto)) {
    const descriptor = Object.getOwnPropertyDescriptor(proto, key)
    if (!descriptor || typeof descriptor.get !== 'function' || !descriptor.configurable) continue
    const get = descriptor.get
    descriptor.get = function () {
      let own = subApis.get(this)
      if (own && own.has(key)) return own.get(key)
      const value = get.call(this)
      if (value === null || typeof value !== 'object') return value
      if (!own) {
        own = new Map()
        subApis.set(this, own)
      }
      own.set(key, value)
      return value
    }
    Object.defineProperty(proto, key, descriptor)
  }

  // `await using commerce = await Commerce.open(path)`
  if (typeof Symbol.asyncDispose === 'symbol') {
    Object.defineProperty(proto, Symbol.asyncDispose, {
      value: function asyncDispose() {
        return this.close()
      },
      writable: true,
      configurable: true,
    })
  }
}

{
  const proto = nativeBinding.CommerceEventSubscription.prototype

  // The native class delivers events through an *unreferenced* threadsafe
  // callback (see `CommerceEventSubscription` in src/lib.rs for why a native
  // promise cannot do this). `recv()` and async iteration are built here on a
  // queue fed by that callback. `null` from the callback ends the stream.
  //
  // Whether the subscription keeps the process alive follows what a caller
  // would expect of any awaited I/O: it does while a `recv()` is outstanding
  // (so `await subscription.recv()` is never abandoned by an exiting loop)
  // and does not otherwise. `ref()` / `unref()` override that in either
  // direction, and `close()` always releases the hold.
  const nativeRef = proto.ref
  const nativeUnref = proto.unref
  const streams = new WeakMap()
  function state(subscription) {
    let own = streams.get(subscription)
    if (own) return own
    own = { queue: [], waiters: [], ended: false, started: false, mode: 'auto', held: false }
    streams.set(subscription, own)
    return own
  }
  function hold(subscription, own) {
    if (!own.started) return
    const want =
      own.mode === 'ref' ? !own.ended : own.mode === 'unref' ? false : own.waiters.length > 0 && !own.ended
    if (want === own.held) return
    own.held = want
    if (want) nativeRef.call(subscription)
    else nativeUnref.call(subscription)
  }
  function stream(subscription) {
    const own = state(subscription)
    if (own.started) return own
    own.started = true
    subscription.__start((event) => {
      if (event === null || event === undefined) {
        own.ended = true
        for (const resolve of own.waiters.splice(0)) resolve(null)
      } else {
        const resolve = own.waiters.shift()
        if (resolve) resolve(event)
        else own.queue.push(event)
      }
      hold(subscription, own)
    })
    hold(subscription, own)
    return own
  }

  Object.defineProperties(proto, {
    recv: {
      value: function recv() {
        const own = stream(this)
        if (own.queue.length > 0) return Promise.resolve(own.queue.shift())
        if (own.ended) return Promise.resolve(null)
        const pending = new Promise((resolve) => own.waiters.push(resolve))
        hold(this, own)
        return pending
      },
      writable: true,
      configurable: true,
    },
    ref: {
      value: function ref() {
        const own = state(this)
        own.mode = 'ref'
        hold(this, own)
      },
      writable: true,
      configurable: true,
    },
    unref: {
      value: function unref() {
        const own = state(this)
        own.mode = 'unref'
        hold(this, own)
      },
      writable: true,
      configurable: true,
    },
    [Symbol.asyncIterator]: {
      value: async function* events() {
        try {
          for (;;) {
            const event = await this.recv()
            if (event === null) return
            yield event
          }
        } finally {
          this.close()
        }
      },
      writable: true,
      configurable: true,
    },
  })
  if (typeof Symbol.dispose === 'symbol') {
    Object.defineProperty(proto, Symbol.dispose, {
      value: function dispose() {
        this.close()
      },
      writable: true,
      configurable: true,
    })
  }
}

{
  // Side stores (`commerce.openSideStore()`): the better-sqlite3-shaped
  // surface over the native `SideStore` / `SideStoreStatement`. Argument
  // sorting, `exec()` returning the store, `pragma()` validation and
  // `transaction()` follow better-sqlite3 11 (lib/methods/transaction.js and
  // src/util/binder.lzz) so a store written against it behaves the same. The
  // native side binds and executes on the engine's own SQLite connection.
  const storeProto = nativeBinding.SideStore.prototype
  const statementProto = nativeBinding.SideStoreStatement.prototype

  const isPlainObject = (value) => {
    const prototype = Object.getPrototypeOf(value)
    return prototype === Object.prototype || prototype === null
  }

  // better-sqlite3's argument rules: arrays spread into positional values, one
  // plain object supplies named values, anything else is positional.
  function sortArguments(args) {
    const positional = []
    let named = null
    for (const arg of args) {
      if (Array.isArray(arg)) {
        for (const value of arg) positional.push(value)
      } else if (arg !== null && typeof arg === 'object' && !ArrayBuffer.isView(arg) && isPlainObject(arg)) {
        if (named !== null) throw new TypeError('You cannot specify named parameters in two different objects')
        named = arg
      } else {
        positional.push(arg)
      }
    }
    return [positional, named]
  }

  const method = (fn) => ({ value: fn, writable: true, configurable: true })

  Object.defineProperties(statementProto, {
    run: method(function run(...args) {
      const [positional, named] = sortArguments(args)
      return this.__run(positional, named)
    }),
    get: method(function get(...args) {
      const [positional, named] = sortArguments(args)
      return this.__get(positional, named)
    }),
    all: method(function all(...args) {
      const [positional, named] = sortArguments(args)
      return this.__all(positional, named)
    }),
  })

  // One set of BEGIN/COMMIT/SAVEPOINT statements per side store.
  const controllers = new WeakMap()
  function controllerFor(store) {
    let controller = controllers.get(store)
    if (!controller) {
      const shared = {
        commit: store.prepare('COMMIT'),
        rollback: store.prepare('ROLLBACK'),
        savepoint: store.prepare('SAVEPOINT `\t_bs3.\t`'),
        release: store.prepare('RELEASE `\t_bs3.\t`'),
        rollbackTo: store.prepare('ROLLBACK TO `\t_bs3.\t`'),
      }
      controller = {
        default: { begin: store.prepare('BEGIN'), ...shared },
        deferred: { begin: store.prepare('BEGIN DEFERRED'), ...shared },
        immediate: { begin: store.prepare('BEGIN IMMEDIATE'), ...shared },
        exclusive: { begin: store.prepare('BEGIN EXCLUSIVE'), ...shared },
      }
      controllers.set(store, controller)
    }
    return controller
  }

  // A nested call runs in a savepoint; a throw rolls back to it (or the whole
  // transaction at the outermost level) and rethrows.
  const wrapTransaction = (fn, store, { begin, commit, rollback, savepoint, release, rollbackTo }) =>
    function sideStoreTransaction() {
      // Strict, so `this` reaches the body unboxed, as in better-sqlite3.
      'use strict'
      let before, after, undo
      if (store.inTransaction) {
        before = savepoint
        after = release
        undo = rollbackTo
      } else {
        before = begin
        after = commit
        undo = rollback
      }
      before.run()
      try {
        const result = fn.apply(this, arguments)
        if (result && typeof result.then === 'function') {
          throw new TypeError('Transaction function cannot return a promise')
        }
        after.run()
        return result
      } catch (error) {
        if (store.inTransaction) {
          undo.run()
          if (undo !== rollback) after.run()
        }
        throw error
      }
    }

  Object.defineProperties(storeProto, {
    exec: method(function exec(sql) {
      if (typeof sql !== 'string') throw new TypeError('Expected first argument to be a string')
      this.__exec(sql)
      return this
    }),
    pragma: method(function pragma(source, options) {
      if (options == null) options = {}
      if (typeof source !== 'string') throw new TypeError('Expected first argument to be a string')
      if (typeof options !== 'object') throw new TypeError('Expected second argument to be an options object')
      const simple = 'simple' in options ? options.simple : false
      if (typeof simple !== 'boolean') throw new TypeError('Expected the "simple" option to be a boolean')
      return this.__pragma(source, simple)
    }),
    transaction: method(function transaction(fn) {
      if (typeof fn !== 'function') throw new TypeError('Expected first argument to be a function')
      const controller = controllerFor(this)
      const properties = {
        default: { value: wrapTransaction(fn, this, controller.default) },
        deferred: { value: wrapTransaction(fn, this, controller.deferred) },
        immediate: { value: wrapTransaction(fn, this, controller.immediate) },
        exclusive: { value: wrapTransaction(fn, this, controller.exclusive) },
        database: { value: this, enumerable: true },
      }
      Object.defineProperties(properties.default.value, properties)
      Object.defineProperties(properties.deferred.value, properties)
      Object.defineProperties(properties.immediate.value, properties)
      Object.defineProperties(properties.exclusive.value, properties)
      return properties.default.value
    }),
  })
  if (typeof Symbol.dispose === 'symbol') {
    Object.defineProperty(storeProto, Symbol.dispose, method(function dispose() {
      this.close()
    }))
  }

  // One side store per engine for hosts whose stores should share a
  // connection. Keyed by the native instance, so it stays one store however
  // many wrapper objects a host builds around the engine (methods reached
  // through a wrapper are bound to the native instance).
  const sharedSideStores = new WeakMap()
  Object.defineProperty(
    nativeBinding.Commerce.prototype,
    'sharedSideStore',
    method(function sharedSideStore() {
      let store = sharedSideStores.get(this)
      if (!store || !store.open) {
        store = this.openSideStore()
        sharedSideStores.set(this, store)
      }
      return store
    }),
  )
}

module.exports.Commerce = nativeBinding.Commerce
module.exports.SideStore = nativeBinding.SideStore
module.exports.SideStoreStatement = nativeBinding.SideStoreStatement
module.exports.Events = nativeBinding.Events
module.exports.CommerceEventSubscription = nativeBinding.CommerceEventSubscription
module.exports.Customers = nativeBinding.Customers
module.exports.Orders = nativeBinding.Orders
module.exports.Products = nativeBinding.Products
module.exports.CustomObjects = nativeBinding.CustomObjects
module.exports.Inventory = nativeBinding.Inventory
module.exports.Returns = nativeBinding.Returns
module.exports.Payments = nativeBinding.Payments
module.exports.Shipments = nativeBinding.Shipments
module.exports.Warranties = nativeBinding.Warranties
module.exports.PurchaseOrders = nativeBinding.PurchaseOrders
module.exports.Invoices = nativeBinding.Invoices
module.exports.Bom = nativeBinding.Bom
module.exports.WorkOrders = nativeBinding.WorkOrders
module.exports.Carts = nativeBinding.Carts
module.exports.Analytics = nativeBinding.Analytics
module.exports.CurrencyOperations = nativeBinding.CurrencyOperations
module.exports.Subscriptions = nativeBinding.Subscriptions
module.exports.Promotions = nativeBinding.Promotions
module.exports.Tax = nativeBinding.Tax
module.exports.Quality = nativeBinding.Quality
module.exports.Lots = nativeBinding.Lots
module.exports.Serials = nativeBinding.Serials
module.exports.Warehouse = nativeBinding.Warehouse
module.exports.Receiving = nativeBinding.Receiving
module.exports.Fulfillment = nativeBinding.Fulfillment
module.exports.AccountsPayable = nativeBinding.AccountsPayable
module.exports.AccountsReceivable = nativeBinding.AccountsReceivable
module.exports.CostAccounting = nativeBinding.CostAccounting
module.exports.Credit = nativeBinding.Credit
module.exports.Backorders = nativeBinding.Backorders
module.exports.GeneralLedger = nativeBinding.GeneralLedger
module.exports.vesX402ComputeSigningHash = nativeBinding.vesX402ComputeSigningHash
module.exports.X402 = nativeBinding.X402
module.exports.VectorSearch = nativeBinding.VectorSearch
module.exports.jcsCanonicalize = nativeBinding.jcsCanonicalize
module.exports.domainHash = nativeBinding.domainHash
module.exports.ed25519Sign = nativeBinding.ed25519Sign
module.exports.ed25519Verify = nativeBinding.ed25519Verify
module.exports.vesHybridGenerateSigningKeypair = nativeBinding.vesHybridGenerateSigningKeypair
module.exports.vesHybridSignEventHash = nativeBinding.vesHybridSignEventHash
module.exports.vesHybridVerifyEventSignature = nativeBinding.vesHybridVerifyEventSignature
module.exports.vesTestVectorMlDsaPublicKey = nativeBinding.vesTestVectorMlDsaPublicKey
module.exports.vesHybridGenerateRecipientKeypair = nativeBinding.vesHybridGenerateRecipientKeypair
module.exports.vesTestVectorMlKemPublicKey = nativeBinding.vesTestVectorMlKemPublicKey
module.exports.vesHybridEncryptPayload = nativeBinding.vesHybridEncryptPayload
module.exports.vesHybridDecryptPayload = nativeBinding.vesHybridDecryptPayload
module.exports.vesStrictGenerateSigningKeypair = nativeBinding.vesStrictGenerateSigningKeypair
module.exports.vesStrictSignEventHash = nativeBinding.vesStrictSignEventHash
module.exports.vesStrictVerifyEventSignature = nativeBinding.vesStrictVerifyEventSignature
module.exports.vesStrictGenerateRecipientKeypair = nativeBinding.vesStrictGenerateRecipientKeypair
module.exports.vesStrictEncryptPayload = nativeBinding.vesStrictEncryptPayload
module.exports.vesStrictDecryptPayload = nativeBinding.vesStrictDecryptPayload
module.exports.vesHybridGenerateSigningPop = nativeBinding.vesHybridGenerateSigningPop
module.exports.vesHybridVerifySigningPop = nativeBinding.vesHybridVerifySigningPop
module.exports.vesStrictGenerateSigningPop = nativeBinding.vesStrictGenerateSigningPop
module.exports.vesStrictVerifySigningPop = nativeBinding.vesStrictVerifySigningPop
module.exports.aesGcmEncrypt = nativeBinding.aesGcmEncrypt
module.exports.aesGcmDecrypt = nativeBinding.aesGcmDecrypt
module.exports.merkleRoot = nativeBinding.merkleRoot
module.exports.GiftCards = nativeBinding.GiftCards
module.exports.StoreCredits = nativeBinding.StoreCredits
module.exports.Reviews = nativeBinding.Reviews
module.exports.Wishlists = nativeBinding.Wishlists
module.exports.Segments = nativeBinding.Segments
module.exports.Loyalty = nativeBinding.Loyalty
module.exports.FixedAssets = nativeBinding.FixedAssets
module.exports.RevenueRecognition = nativeBinding.RevenueRecognition
module.exports.CycleCounts = nativeBinding.CycleCounts
module.exports.EdiDocuments = nativeBinding.EdiDocuments
module.exports.Prepayments = nativeBinding.Prepayments
module.exports.VendorCredits = nativeBinding.VendorCredits
module.exports.PriceSchedules = nativeBinding.PriceSchedules
module.exports.PriceLevels = nativeBinding.PriceLevels
module.exports.TransferOrders = nativeBinding.TransferOrders
module.exports.ProductionBatches = nativeBinding.ProductionBatches
module.exports.SupplierSkus = nativeBinding.SupplierSkus
module.exports.InboundShipments = nativeBinding.InboundShipments
module.exports.ActivityLogs = nativeBinding.ActivityLogs
module.exports.Channels = nativeBinding.Channels
module.exports.Companies = nativeBinding.Companies
module.exports.UnitsOfMeasure = nativeBinding.UnitsOfMeasure
module.exports.ShippingZones = nativeBinding.ShippingZones
module.exports.StockSnapshots = nativeBinding.StockSnapshots
module.exports.PrintStations = nativeBinding.PrintStations
module.exports.IntegrationMappings = nativeBinding.IntegrationMappings
module.exports.IntegrationFieldMappings = nativeBinding.IntegrationFieldMappings
module.exports.PaymentObligations = nativeBinding.PaymentObligations
module.exports.Purgatory = nativeBinding.Purgatory
module.exports.TopologySnapshots = nativeBinding.TopologySnapshots
module.exports.VendorReturns = nativeBinding.VendorReturns
module.exports.Fraud = nativeBinding.Fraud
module.exports.SearchConfigs = nativeBinding.SearchConfigs
module.exports.Erc8004 = nativeBinding.Erc8004
module.exports.Maintenance = nativeBinding.Maintenance
module.exports.__testPanic = nativeBinding.__testPanic
module.exports.__testPanicAsync = nativeBinding.__testPanicAsync
module.exports.__testPanicAsyncUnguarded = nativeBinding.__testPanicAsyncUnguarded
module.exports.__testMoneyNotRepresentable = nativeBinding.__testMoneyNotRepresentable
