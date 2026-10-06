//! Ruby binding for the StateSet embedded commerce engine.
//!
//! * [`dispatch`] — the real engine behind every Ruby call: JSON in, the
//!   engine's own types, JSON envelope out. Pure Rust, tested with
//!   `cargo test` (no Ruby needed).
//! * `runtime` (feature `runtime`) — the magnus glue that registers
//!   `StateSet::Native` and `StateSet::Crypto` with the Ruby VM.

pub mod dispatch;

#[cfg(feature = "runtime")]
mod runtime;
