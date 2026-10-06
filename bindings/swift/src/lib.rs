//! Native library for the StateSet Swift binding.
//!
//! This crate contains no FFI code of its own. Every C function declared in
//! `Sources/StateSetC/stateset.h` (`stateset_json_open`, `stateset_json_call`,
//! `stateset_destroy`, `stateset_string_free`, `stateset_last_error_message`,
//! `stateset_abi_version`, and the `stateset_crypto_*` primitives) is defined
//! once in `stateset-ffi` and re-exported here so the library carries the
//! name `stateset_swift` that the module map links. The .NET binding loads the
//! same surface under `stateset_dotnet`.

pub use stateset_ffi;
