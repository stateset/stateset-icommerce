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

const nativeBinding = wrapNativeExports(require('./native-binding.js'))

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

module.exports.Commerce = nativeBinding.Commerce
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
