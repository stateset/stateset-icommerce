//! magnus glue: registers `StateSet::Native` (the engine handle) and
//! `StateSet::Crypto` (cross-binding crypto primitives) with the Ruby VM.
//!
//! All commerce behaviour lives in [`crate::dispatch`]; this file only moves
//! strings across the boundary. `Native#call` never raises for engine
//! failures — it returns the JSON envelope and the Ruby layer
//! (`lib/stateset_embedded/native.rb`) raises the matching
//! `StateSet::Error` subclass. Panics are caught inside the dispatcher, so
//! none unwind into the Ruby VM.

use magnus::{Error, IntoValue, RArray, RString, Ruby, Value, function, method, prelude::*};

use crate::dispatch::{Engine, error_envelope};

// =============================================================================
// Engine handle
// =============================================================================

#[magnus::wrap(class = "StateSet::Native", free_immediately, size)]
struct Native {
    engine: Engine,
}

impl Native {
    /// `StateSet::Native.open(path)` -> a `Native`, or a JSON error envelope
    /// `String` when the store cannot be opened.
    fn open(ruby: &Ruby, path: String) -> Value {
        match Engine::open(&path) {
            Ok(engine) => Self { engine }.into_value_with(ruby),
            Err(err) => error_envelope(err).into_value_with(ruby),
        }
    }

    /// `native.call(op, args_json)` -> JSON envelope `String`.
    fn call(&self, op: String, args_json: String) -> String {
        self.engine.call(&op, &args_json)
    }
}

// =============================================================================
// Cross-binding crypto primitives
// =============================================================================
//
// Thin wrappers over `stateset-crypto` so the Ruby binding verifies the
// language-neutral corpus at `bindings/test-vectors/v1.json`.

fn invalid_json(ruby: &Ruby, e: &serde_json::Error) -> Error {
    Error::new(ruby.exception_arg_error(), format!("invalid JSON: {e}"))
}

fn crypto_jcs_canonicalize(ruby: &Ruby, json_str: String) -> Result<RString, Error> {
    let value: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| invalid_json(ruby, &e))?;
    let canonical = stateset_crypto::canonicalize::canonicalize_json_bytes(&value)
        .map_err(|e| Error::new(ruby.exception_runtime_error(), format!("canonicalize: {e}")))?;
    Ok(ruby.str_from_slice(&canonical))
}

fn fixed_bytes<const N: usize>(ruby: &Ruby, what: &str, s: RString) -> Result<[u8; N], Error> {
    // SAFETY: the slice is copied before any Ruby code can run and mutate
    // or move the string.
    let bytes = unsafe { s.as_slice() };
    <[u8; N]>::try_from(bytes).map_err(|_| {
        Error::new(
            ruby.exception_arg_error(),
            format!("{what} must be exactly {N} bytes, got {}", bytes.len()),
        )
    })
}

fn crypto_payload_plain_hash(
    ruby: &Ruby,
    json_str: String,
    salt: Option<RString>,
) -> Result<RString, Error> {
    let value: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| invalid_json(ruby, &e))?;
    let salt = salt.map(|s| fixed_bytes::<16>(ruby, "salt", s)).transpose()?;
    let digest =
        stateset_crypto::hash::compute_payload_plain_hash(&value, salt.as_ref()).map_err(|e| {
            Error::new(ruby.exception_runtime_error(), format!("payload_plain_hash: {e}"))
        })?;
    Ok(ruby.str_from_slice(&digest))
}

fn crypto_merkle_root(ruby: &Ruby, leaves: RArray) -> Result<RString, Error> {
    let mut typed: Vec<[u8; 32]> = Vec::with_capacity(leaves.len());
    for (i, leaf) in leaves.into_iter().enumerate() {
        let leaf = RString::try_convert(leaf)?;
        typed.push(fixed_bytes::<32>(ruby, &format!("leaf {i}"), leaf)?);
    }
    let root = stateset_crypto::merkle::compute_merkle_root(&typed);
    Ok(ruby.str_from_slice(&root))
}

// =============================================================================
// Registration
// =============================================================================

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("StateSet")?;

    let native = module.define_class("Native", ruby.class_object())?;
    native.undef_default_alloc_func();
    native.define_singleton_method("open", function!(Native::open, 1))?;
    native.define_method("call", method!(Native::call, 2))?;

    let crypto = module.define_module("Crypto")?;
    crypto.define_singleton_method("jcs_canonicalize", function!(crypto_jcs_canonicalize, 1))?;
    crypto
        .define_singleton_method("payload_plain_hash", function!(crypto_payload_plain_hash, 2))?;
    crypto.define_singleton_method("merkle_root", function!(crypto_merkle_root, 1))?;

    Ok(())
}
