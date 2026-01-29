# Embedded SDK Bindings

## Packages by Language

- Rust: `stateset-embedded`
- Node.js: `@stateset/embedded`
- Python: `stateset-embedded`
- Ruby: `stateset_embedded`
- Java: JNI binding
- Kotlin: JVM binding
- Swift: FFI binding
- C#: P/Invoke binding
- Go: cgo binding
- WASM: `@stateset/embedded-wasm`

## Example Files

- Rust: `/home/dom/stateset-icommerce/examples/basic_usage.rs`
- Node: `/home/dom/stateset-icommerce/examples/node/basic_usage.js`
- Python: `/home/dom/stateset-icommerce/examples/python/basic_usage.py`
- Ruby: `/home/dom/stateset-icommerce/examples/ruby/basic_usage.rb`
- Java: `/home/dom/stateset-icommerce/examples/java/BasicUsage.java`
- Kotlin: `/home/dom/stateset-icommerce/examples/kotlin/BasicUsage.kt`
- Swift: `/home/dom/stateset-icommerce/examples/swift/BasicUsage.swift`
- C#: `/home/dom/stateset-icommerce/examples/dotnet/BasicUsage.cs`
- Go: `/home/dom/stateset-icommerce/examples/go/basic_usage.go`
- WASM: see `/home/dom/stateset-icommerce/bindings/wasm`

## Standard Usage Pattern

1. Initialize `Commerce` with a SQLite file path.
2. Create customers, products, inventory, and orders.
3. Use module APIs (customers, orders, inventory, returns, etc).
