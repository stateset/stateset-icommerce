# stateset-ffi

`stateset-ffi` is the optional C-ABI oriented interop layer for StateSet iCommerce.

It exposes `#[repr(C)]` types and `extern "C"` functions around the embedded commerce engine and selected sync runtime helpers. Use it when you need a stable C-style boundary for custom hosts or generated bindings.

## What This Crate Is

- A focused C-ABI surface over `stateset-embedded`
- A place for explicit C-style integration and compatibility-sensitive exports
- A narrower interop layer than the full Rust or Node binding surface

## What This Crate Is Not

- The universal substrate for every binding in this repository
- The only way non-Rust runtimes connect to the engine

Several bindings in this repo link directly to `stateset-embedded` and `stateset-core` in their own crates. Treat `stateset-ffi` as an explicit interop option, not as the mandatory path through the binding layer.
