//! Panic containment probes (feature `test-panic` only).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]
// `#[napi]` free functions and their object types are reached only through
// napi's registration, which rustc cannot see from inside a private module.
#![allow(dead_code)]

use super::*;

// ============================================================================
// Panic containment probes (feature `test-panic` only)
// ============================================================================
//
// Off by default, so these symbols never reach a published binary and the
// committed `native-binding.js` / `index.d.ts` do not declare them.
// `npm run build:debug` turns the feature on for `test/panic-containment.js`.

/// Panic on purpose, synchronously, inside a [`guard`].
///
/// A sync `#[napi]` entry point has no safety net of its own: napi generates an
/// `extern "C"` shim, and an unwind escaping that aborts the process whatever
/// the panic strategy is. [`guard`] is what turns it into a JavaScript error
/// with `code: 'INTERNAL_PANIC'`.
#[cfg(feature = "test-panic")]
#[napi(js_name = "__testPanic")]
pub fn test_panic(message: Option<String>) -> Result<()> {
    guard(|| panic!("{}", message.unwrap_or_else(|| "deliberate test panic".to_owned())))
}

/// The async twin of [`test_panic`]: panics while the future is being polled,
/// inside a [`guard_async`], which reports the panic payload verbatim.
#[cfg(feature = "test-panic")]
#[napi(js_name = "__testPanicAsync")]
pub async fn test_panic_async(message: Option<String>) -> Result<()> {
    guard_async(async move {
        panic!("{}", message.unwrap_or_else(|| "deliberate test panic".to_owned()))
    })
    .await
}

/// Panic inside an async entry point that is **not** guarded, to prove the
/// fallback path.
///
/// `napi::tokio_runtime::execute_tokio_future` watches the spawned task and
/// rejects the promise with `Status::GenericFailure` if it panicked — so all 728
/// async entry points already fail soft once `panic = "unwind"` is in effect.
/// They just carry no code, which `decorate()` in `errors.js` supplies. Note
/// napi only forwards a `&'static str` payload; a formatted `String` becomes
/// the fixed text `"Panic in async function"`, which is why this probe panics
/// with a literal.
#[cfg(feature = "test-panic")]
#[napi(js_name = "__testPanicAsyncUnguarded")]
pub async fn test_panic_async_unguarded() -> Result<()> {
    tokio::task::yield_now().await;
    panic!("unguarded async panic")
}

/// Feed a value the `f64` narrowing cannot represent through
/// [`to_f64_checked`], so `test/money-exactness.js` can assert the failure
/// reaches JavaScript as `code: 'INTERNAL'` naming the field, rather than as a
/// silent `NaN`.
///
/// A non-finite `f64` is the only input that actually reaches the failure arm
/// today — `Decimal::to_f64` is total, `Decimal::MAX` included — so that is what
/// this probe sends. The field label is the caller's, to prove the message
/// carries it.
#[cfg(feature = "test-panic")]
#[napi(js_name = "__testMoneyNotRepresentable")]
pub fn test_money_not_representable(field: String) -> Result<f64> {
    guard(|| money_to_f64(f64::NAN, &field))
}
