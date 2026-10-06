//! Native library for the StateSet .NET binding.
//!
//! This crate contains no FFI code of its own. Every C export the C# side
//! P/Invokes (`stateset_json_open`, `stateset_json_call`, `stateset_destroy`,
//! `stateset_string_free`, `stateset_last_error_message`,
//! `stateset_abi_version`, and the `stateset_crypto_*` primitives) is defined
//! once in `stateset-ffi` and re-exported here so the shared library carries
//! the binding-specific name `stateset_dotnet`. The Swift binding links the
//! same surface under `stateset_swift`.

pub use stateset_ffi;
