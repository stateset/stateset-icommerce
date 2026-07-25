# stateset-ffi

[![crates.io](https://img.shields.io/crates/v/stateset-ffi.svg)](https://crates.io/crates/stateset-ffi)
[![docs.rs](https://docs.rs/stateset-ffi/badge.svg)](https://docs.rs/stateset-ffi)

An optional C-ABI interop layer for StateSet iCommerce: `#[repr(C)]` types and
`extern "C"` functions over the embedded commerce engine and selected sync runtime
helpers.

Use it when you need a stable C-style boundary for a custom host or generated
bindings. If you're writing Rust, use
[`stateset-sdk`](https://crates.io/crates/stateset-sdk) instead.

## What This Crate Is

- A focused C-ABI surface over
  [`stateset-embedded`](https://crates.io/crates/stateset-embedded)
- A place for explicit C-style integration and compatibility-sensitive exports
- A narrower interop layer than the full Rust or Node binding surface

## What This Crate Is Not

- The universal substrate for every binding in this repository
- The only way non-Rust runtimes connect to the engine

Several bindings in this repo link directly to `stateset-embedded` and
`stateset-core` in their own crates. Treat `stateset-ffi` as an explicit interop
option, not as the mandatory path through the binding layer.

## Design Principles

- **Minimal surface** — only the most commonly needed operations are exposed.
- **ABI stability** — all public types are `#[repr(C)]`, enums have explicit
  discriminants, structs use fixed-size fields.
- **Thread safety** — the underlying engine is `Send + Sync`; error messages use
  thread-local storage.
- **No panics across FFI** — every function catches errors and returns an
  `FfiErrorCode` rather than unwinding.

## Usage (C)

```c
#include "stateset.h"

FfiResult_CommerceHandle result = stateset_init(":memory:");
if (result.code != Ok) {
    fprintf(stderr, "init failed: %s\n", stateset_last_error_message());
    return 1;
}
CommerceHandle engine = result.value;

// ... use engine ...

stateset_destroy(engine);
```

## ABI Versioning

Call `stateset_abi_version` at load time and compare it against the version your
bindings were generated for. A mismatch means regenerate, not proceed — the `version`
module documents the guarantees.

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `cbindgen` | Emit a C header via `cbindgen` during the build | No |

## Part of StateSet iCommerce

Wraps [`stateset-embedded`](https://crates.io/crates/stateset-embedded) and
[`stateset-sdk`](https://crates.io/crates/stateset-sdk). Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
