#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet Protocol
//!
//! Canonical wire-format types for the StateSet sync protocol. This crate is
//! IO-free, DB-free, and WASM-compatible — it defines only the data structures
//! and pure functions needed for nodes to agree on event representation.
//!
//! ## What's Inside
//!
//! - **[`EventEnvelope`]** — a single domain event in wire format, with metadata,
//!   payload, and integrity hash.
//! - **[`SyncBatch`]** — a group of envelopes for node-to-node sync, with
//!   Merkle root, signatures, and inclusion proofs.
//! - **[`canonical`]** — RFC 8785 JCS canonical JSON, domain-separated hashing,
//!   and version newtypes.
//! - **[`merkle`]** — Merkle tree construction, proof generation, and verification.
//! - **[`ProtocolError`]** — unified error type for all protocol operations.
//!
//! ## Quick Start
//!
//! ```rust
//! use stateset_protocol::{EventEnvelope, SyncBatch, PayloadCodec};
//!
//! // Build an event envelope
//! let envelope = EventEnvelope::builder()
//!     .event_type("order.created")
//!     .entity_type("order")
//!     .entity_id("ord_42")
//!     .payload(br#"{"total": 100}"#.to_vec())
//!     .build()
//!     .unwrap();
//!
//! assert!(envelope.validate().is_ok());
//!
//! // Bundle into a sync batch
//! let batch = SyncBatch::new("node_alpha", vec![envelope]);
//! assert!(batch.verify_merkle_root());
//! assert!(batch.validate().is_ok());
//! ```
//!
//! ## Design Principles
//!
//! - **No IO**: no network, no filesystem, no database access.
//! - **Deterministic**: canonical serialization ensures byte-identical output.
//! - **Forward-compatible**: protocol and schema version fields allow evolution.
//! - **Type-safe**: newtypes prevent confusion between protocol and schema versions.

pub mod batch;
pub mod canonical;
pub mod envelope;
pub mod error;
pub mod merkle;

// Re-export primary types at crate root for convenience.
pub use batch::{BatchSignature, MerkleProof, SignatureAlgorithm, SyncBatch};
pub use canonical::{ProtocolVersion, SchemaVersion};
pub use envelope::{EventEnvelope, EventEnvelopeBuilder, PayloadCodec};
pub use error::ProtocolError;

// Ensure stateset-primitives is linked even though this crate does not
// (yet) reference its types directly. It is a declared dependency for
// forward-compatible wire types that will carry domain IDs.
use stateset_primitives as _;
