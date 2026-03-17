# ADR-0005: Binding Generation From a Single Spec

- Status: Accepted
- Date: 2026-02-05

## Context

StateSet iCommerce ships 11 language bindings: Rust, Node.js, Python, Ruby, PHP, Java, Kotlin, Swift, C#/.NET, Go, and WASM. Hand-maintaining each binding would create drift, inconsistent behavior, and high maintenance cost as the API surface evolves (currently 41 accessor methods with 30+ domain APIs each).

We considered three approaches:

1. **Hand-written bindings** — Maximum flexibility per language, but maintenance cost scales as O(languages × API changes). Already impractical with 11 bindings.
2. **OpenAPI code generation** — Generate from an HTTP API spec. But iCommerce is an embedded library, not an HTTP service — OpenAPI doesn't capture in-process semantics.
3. **Single-spec generation from Rust** — Define the public binding surface once in the Rust crate and derive bindings via FFI + language-specific generators. Each binding can add idiomatic wrappers on top.

## Decision

Define the public binding surface once in a declarative generator spec and derive bindings from it. The generator spec is the source of truth for exposed types and operations, while each binding remains free to provide language-idiomatic wrappers.

### Binding Technologies

| Language | Binding Technology | Idiomatic Wrapper |
|----------|-------------------|-------------------|
| Node.js | NAPI-RS | ES modules, camelCase |
| Python | PyO3 | snake_case, type stubs |
| Ruby | Magnus | Ruby conventions |
| PHP | ext-php-rs | PHP extension |
| Go | cgo | Go error patterns |
| Java | JNI | try-with-resources |
| Kotlin | JNI | Named parameters, `use {}` |
| Swift | C FFI | `try` + `do/catch` |
| C# / .NET | P/Invoke | `using`, PascalCase |
| WASM | wasm-bindgen | TypeScript definitions |

### API Convention Mapping

The Rust API uses snake_case. Each binding adapts to its language's convention:

| Rust | Node.js | Python | Go | C# |
|------|---------|--------|----|----|
| `customers()` | `customers` | `customers` | `Customers()` | `Customers` |
| `create_customer()` | `create()` | `create()` | `Create()` | `Create()` |
| `list_by_status()` | `listByStatus()` | `list_by_status()` | `ListByStatus()` | `ListByStatus()` |

## Consequences

**Positive:**
- API parity across languages — a bug fixed in Rust is automatically fixed in all bindings
- Changes to the surface area require updates in a single place
- New domain APIs (e.g., `gift_cards()`) automatically appear in all 11 bindings
- Cross-language test vectors ensure byte-identical behavior (especially critical for VES cryptography)
- Type safety propagates: Rust's `OrderId` becomes a typed string in TypeScript, a validated parameter in Python

**Negative:**
- Some language-specific conveniences still require lightweight handwritten glue (e.g., Python's context manager `__enter__`/`__exit__`, Kotlin's coroutine support)
- The FFI boundary has marshalling overhead (negligible for commerce operations, measurable for high-frequency crypto operations)
- Platform support matrix is constrained by what the FFI layer can target (e.g., iOS requires a static library, WASM requires specific browser APIs)

## Testing Strategy

1. **Cross-language test vectors**: Critical operations (VES signing, canonical JSON) have test vectors that all bindings must reproduce byte-for-byte
2. **Binding-specific tests**: Each binding has language-idiomatic tests for its wrapper layer
3. **CI matrix**: Every binding is built and tested on its target platforms (Linux, macOS, Windows where applicable)
