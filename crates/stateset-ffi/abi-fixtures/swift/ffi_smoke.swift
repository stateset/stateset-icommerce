#if os(Linux)
import Glibc
#else
import Darwin
#endif

@frozen
struct FfiMoney {
    var amount_cents: Int64
    var currency: (UInt8, UInt8, UInt8)
}

@frozen
struct FfiResultCommerceHandle {
    var code: Int32
    var value: UnsafeMutableRawPointer?
}

@_silgen_name("stateset_abi_version")
func stateset_abi_version() -> UInt32

@_silgen_name("stateset_version")
func stateset_version() -> UnsafePointer<CChar>?

@_silgen_name("stateset_money_format")
func stateset_money_format(_ money: FfiMoney) -> UnsafeMutablePointer<CChar>?

@_silgen_name("stateset_string_free")
func stateset_string_free(_ ptr: UnsafeMutablePointer<CChar>?)

@_silgen_name("stateset_init")
func stateset_init(_ dbPath: UnsafePointer<CChar>?) -> FfiResultCommerceHandle

@_silgen_name("stateset_destroy")
func stateset_destroy(_ engine: UnsafeMutableRawPointer?)

func ensure(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() {
        fputs("swift ffi smoke failure: \(message)\n", stderr)
        exit(1)
    }
}

ensure(stateset_abi_version() == 1, "unexpected ABI version")

if let version = stateset_version() {
    ensure(strlen(version) > 0, "version string is empty")
} else {
    ensure(false, "version string pointer is null")
}

let money = FfiMoney(amount_cents: 1234, currency: (85, 83, 68))
if let formattedPtr = stateset_money_format(money) {
    let text = String(cString: formattedPtr)
    ensure(text == "$12.34", "unexpected money formatting output")
    stateset_string_free(formattedPtr)
} else {
    ensure(false, "stateset_money_format returned null")
}

let initResult = ":memory:".withCString { path in
    stateset_init(path)
}
ensure(initResult.code == 0, "stateset_init failed")
ensure(initResult.value != nil, "stateset_init returned null handle")
stateset_destroy(initResult.value)
