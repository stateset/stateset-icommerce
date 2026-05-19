//! # `stateset-icp-client`
//!
//! Rust SDK for the **Intelligent Commerce Protocol (ICP-1.0)**.
//!
//! Mirrors the API of [`@stateset/icp-client`][npm] (`JavaScript`) and [`icp-client`][py]
//! (`Python`). Produces
//!
//! [npm]: https://www.npmjs.com/package/@stateset/icp-client
//! [py]: https://pypi.org/project/icp-client/
//! byte-identical wire bytes verified by the ICP conformance suite at
//! `icp-conformance/vectors/icp-1.0/`.
//!
//! ## Quickstart
//!
//! ```no_run
//! use stateset_icp_client::{Client, Identity};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // 1. Generate (or load) an Agent identity.
//! let identity = Identity::generate();
//!
//! // 2. Connect to a merchant handler.
//! let client = Client::new("https://shop.example.com", identity);
//!
//! // 3. Discover merchant capabilities (merchant_aid, settlers, verbs).
//! let caps = client.well_known()?;
//! let merchant = caps["merchant_aid"].as_str().unwrap();
//! let settler = caps["settler_allowlist"][0].as_str().unwrap();
//!
//! // 4. Submit signed Intents — the SDK handles canonicalization + signing.
//! let inventory = client.inventory(merchant, settler, &["WIDGET-001"])?;
//! println!("{} from merchant {}", inventory.payload, inventory.signature.kid);
//! # Ok(())
//! # }
//! ```

#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

mod canonical;
mod client;
mod error;
mod identity;
mod intents;
mod settlement;
mod types;
mod webhook;

pub use canonical::canonical_json;
pub use client::{Client, SignedResponse};
pub use error::Error;
pub use identity::{Identity, verify_ed25519};
pub use intents::{IntentEnvelope, build_intent_envelope};
pub use settlement::{VerifySettlementReceiptOptions, verify_settlement_receipt};
pub use types::{AID, Authority, IntentBase, LineItem, Money, PrincipalBinding, Signature};
pub use webhook::{HeaderPair, VerifyWebhookOptions, verify_webhook};
