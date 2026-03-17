// FFI crates legitimately need unsafe for `extern "C"` functions and raw
// pointers. We warn (rather than deny) and document every unsafe block.
#![warn(unsafe_code)]
#![cfg_attr(test, allow(unsafe_code))]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # stateset-ffi
//!
//! A stable, C-ABI-safe FFI surface for the StateSet iCommerce engine.
//!
//! This crate provides `#[repr(C)]` types and `extern "C"` functions that
//! can be consumed by **any** language with a C FFI: Python (via ctypes /
//! cffi), Swift, Kotlin/JNI, Go (via cgo), Ruby (via ffi gem), and plain C.
//!
//! ## Design Principles
//!
//! - **Minimal surface**: only the most commonly needed operations are exposed.
//! - **ABI stability**: all public types are `#[repr(C)]`; enums have explicit
//!   discriminant values; structs use fixed-size fields.
//! - **Thread safety**: the underlying `Commerce` engine is `Send + Sync`.
//!   Thread-local error storage is used for error messages.
//! - **No panics across FFI**: every function catches errors and returns an
//!   [`FfiErrorCode`] instead of unwinding.
//!
//! ## Quick Start (C)
//!
//! ```c
//! #include "stateset.h"
//!
//! FfiResult_CommerceHandle result = stateset_init(":memory:");
//! if (result.code != Ok) {
//!     fprintf(stderr, "init failed: %s\n", stateset_last_error_message());
//!     return 1;
//! }
//! CommerceHandle engine = result.value;
//!
//! // ... use engine ...
//!
//! stateset_destroy(engine);
//! ```
//!
//! ## ABI Versioning
//!
//! Call `stateset_abi_version` at load time and compare against the version
//! your bindings were generated for. See [`version`] module for details.

pub mod api;
pub mod convert;
pub mod error;
pub mod strings;
pub mod types;
pub mod version;

// Re-export the most commonly used items at the crate root.
pub use api::CommerceHandle;
pub use convert::{FromFfi, IntoFfi, TryIntoFfi};
pub use error::{FfiErrorCode, FfiResult};
pub use types::{
    FfiCustomer, FfiInventoryLevel, FfiMoney, FfiOrder, FfiOrderStatus, FfiProduct, FfiUuid,
};
pub use version::ABI_VERSION;
