import Foundation
import StateSetC

/// StateSet Embedded Commerce for Swift: the Rust commerce engine
/// (`stateset-embedded`) running in-process against a SQLite file.
///
/// Every call goes through the native library (`libstateset_swift`) and
/// persists to the database passed to the initializer; nothing is kept in
/// Swift memory. Use `":memory:"` for an ephemeral store.
///
/// ```swift
/// let commerce = try StateSetCommerce(dbPath: "store.db")
/// let customer = try commerce.customers.create(email: "alice@example.com", firstName: "Alice", lastName: "Smith")
/// let order = try commerce.orders.create(customerId: customer.id, items: [
///     CreateOrderItem(productId: productId, sku: "WIDGET-001", name: "Widget", quantity: 2, unitPrice: Decimal(string: "29.99")!)
/// ])
/// ```
///
/// Instances are safe to share across threads: the native handle is
/// reference-counted and `close()` waits for in-flight calls.
public final class StateSetCommerce: @unchecked Sendable {
    /// The `stateset-ffi` ABI major version this binding was written for.
    public static let expectedABIVersion: UInt32 = 1

    private let lock = NSLock()
    private var handle: StateSetHandle?

    /// Customers.
    public private(set) lazy var customers = CustomersAPI(commerce: self)
    /// Products and variants.
    public private(set) lazy var products = ProductsAPI(commerce: self)
    /// Inventory items, stock, adjustments and reservations.
    public private(set) lazy var inventory = InventoryAPI(commerce: self)
    /// Carts and checkout.
    public private(set) lazy var carts = CartsAPI(commerce: self)
    /// Orders.
    public private(set) lazy var orders = OrdersAPI(commerce: self)
    /// Payments and refunds.
    public private(set) lazy var payments = PaymentsAPI(commerce: self)
    /// Returns (RMAs).
    public private(set) lazy var returns = ReturnsAPI(commerce: self)
    /// Shipments.
    public private(set) lazy var shipments = ShipmentsAPI(commerce: self)

    /// Open (or create) a store at `dbPath` (`":memory:"` for an ephemeral one).
    public init(dbPath: String) throws {
        let abi = stateset_abi_version()
        guard abi == Self.expectedABIVersion else {
            throw StateSetError(code: .internalError, kind: "abi_mismatch",
                                message: "native library ABI \(abi) != binding ABI \(Self.expectedABIVersion)")
        }
        var h: StateSetHandle?
        let rc = dbPath.withCString { stateset_json_open($0, &h) }
        guard rc == 0, let opened = h else {
            let detail = stateset_last_error_message().map { String(cString: $0) } ?? "unknown error"
            throw StateSetError(code: StateSetError.Code(rawValue: rc) ?? .internalError,
                                kind: "open_failed",
                                message: "failed to open store '\(dbPath)': \(detail)")
        }
        handle = opened
    }

    deinit {
        close()
    }

    /// Close the store and release the native handle. Idempotent.
    public func close() {
        lock.lock()
        let h = handle
        handle = nil
        lock.unlock()
        if let h = h {
            stateset_destroy(h)
        }
    }

    /// Engine and JSON-API versions reported by the native library.
    public func version() throws -> [String: Any] {
        let data = try callRaw("meta.version", args: nil)
        let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        return (object?["result"] as? [String: Any]) ?? [:]
    }

    /// Every method name the native JSON surface accepts.
    public func nativeMethodNames() throws -> [String] {
        try call("meta.methods", nil)
    }

    // MARK: - Native calls

    private struct ResultEnvelope<T: Decodable>: Decodable {
        let result: T?
    }

    internal func call<T: Decodable>(_ method: String, _ args: [String: Any?]?) throws -> T {
        let data = try callRaw(method, args: args)
        guard let value = try Self.decoder.decode(ResultEnvelope<T>.self, from: data).result else {
            throw StateSetError(code: .notFound, kind: "not_found", message: "\(method) returned no result")
        }
        return value
    }

    /// Like `call`, but a `null` result (lookup found nothing) becomes `nil`.
    internal func callOptional<T: Decodable>(_ method: String, _ args: [String: Any?]?) throws -> T? {
        let data = try callRaw(method, args: args)
        return try Self.decoder.decode(ResultEnvelope<T>.self, from: data).result
    }

    /// Call a method whose result is irrelevant (e.g. delete).
    internal func callVoid(_ method: String, _ args: [String: Any?]?) throws {
        _ = try callRaw(method, args: args)
    }

    /// Returns the raw envelope JSON of a successful call; throws on failure.
    private func callRaw(_ method: String, args: [String: Any?]?) throws -> Data {
        lock.lock()
        let h = handle
        lock.unlock()
        guard let h = h else {
            throw StateSetError(code: .invalidArgument, kind: "closed", message: "the store has been closed")
        }

        let argsJSON: String?
        if let args = args {
            let object = JSONArgs.clean(args)
            let data = try JSONSerialization.data(withJSONObject: object, options: [])
            argsJSON = String(data: data, encoding: .utf8)
        } else {
            argsJSON = nil
        }

        let raw: UnsafeMutablePointer<CChar>? = method.withCString { m in
            if let a = argsJSON {
                return a.withCString { stateset_json_call(h, m, $0) }
            }
            return stateset_json_call(h, m, nil)
        }
        guard let ptr = raw else {
            let detail = stateset_last_error_message().map { String(cString: $0) } ?? "no envelope returned"
            throw StateSetError(code: .internalError, kind: "internal_error", message: "\(method): \(detail)")
        }
        let envelopeText = String(cString: ptr)
        stateset_string_free(ptr)

        let data = Data(envelopeText.utf8)
        let envelope = try Self.decoder.decode(Envelope.self, from: data)
        if envelope.ok {
            return data
        }
        let err = envelope.error
        throw StateSetError(code: StateSetError.Code(rawValue: err?.code ?? 3) ?? .internalError,
                            kind: err?.kind ?? "internal_error",
                            message: err?.message ?? "unknown error")
    }

    private struct Envelope: Decodable {
        struct Err: Decodable {
            let code: Int32
            let kind: String
            let message: String
        }
        let ok: Bool
        let error: Err?
    }

    internal static let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        d.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let text = try container.decode(String.self)
            guard let date = RFC3339.parse(text) else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "bad timestamp \(text)")
            }
            return date
        }
        return d
    }()
}

/// An error reported by the StateSet engine.
public struct StateSetError: Error, CustomStringConvertible, Equatable {
    /// Stable error codes (the values of `FfiErrorCode` in `crates/stateset-ffi`).
    public enum Code: Int32 {
        case notFound = 1
        case invalidArgument = 2
        case internalError = 3
        case databaseError = 4
        case serializationError = 5
        case nullPointer = 6
        case utf8Error = 7
        case bufferTooSmall = 8
    }

    /// The engine's error code.
    public let code: Code
    /// The engine's error kind, e.g. `"not_found"`.
    public let kind: String
    /// Human-readable message.
    public let message: String

    public var description: String { "StateSetError(\(kind)): \(message)" }
}

/// Builds argument objects for the native call: drops `nil`s and renders
/// `Decimal` as an exact string, so money never passes through a Double.
enum JSONArgs {
    static func clean(_ value: Any?) -> Any {
        switch value {
        case nil:
            return NSNull()
        case let d as Decimal:
            return decimalString(d)
        case let dict as [String: Any?]:
            var out: [String: Any] = [:]
            for (k, v) in dict {
                if let v = v, !(v is NSNull) { out[k] = clean(v) }
            }
            return out
        case let dict as [String: Any]:
            var out: [String: Any] = [:]
            for (k, v) in dict { out[k] = clean(v) }
            return out
        case let arr as [Any]:
            return arr.map { clean($0) }
        case let e as JSONArgConvertible:
            return clean(e.jsonArg)
        case let v?:
            return v
        }
    }

    static func decimalString(_ d: Decimal) -> String {
        var copy = d
        return NSDecimalString(&copy, Locale(identifier: "en_US_POSIX"))
    }
}

/// A value that renders itself as a JSON argument object.
protocol JSONArgConvertible {
    var jsonArg: Any { get }
}

enum RFC3339 {
    private static let withFraction: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()
    private static let plain: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        return f
    }()
    private static let lock = NSLock()

    /// Parses RFC 3339 timestamps, including chrono's nanosecond fractions.
    static func parse(_ text: String) -> Date? {
        var s = text
        // Trim fractional seconds to milliseconds for ISO8601DateFormatter.
        if let dot = s.firstIndex(of: ".") {
            var end = s.index(after: dot)
            while end < s.endIndex, s[end].isNumber { end = s.index(after: end) }
            let digits = s[s.index(after: dot)..<end]
            let ms = String(digits.prefix(3)).padding(toLength: 3, withPad: "0", startingAt: 0)
            s = String(s[..<dot]) + "." + ms + String(s[end...])
        }
        lock.lock(); defer { lock.unlock() }
        return withFraction.date(from: s) ?? plain.date(from: s)
    }
}
