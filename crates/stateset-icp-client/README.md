# stateset-icp-client

Rust SDK for the **Intelligent Commerce Protocol (ICP-1.0)**. Mirrors
[`@stateset/icp-client`][npm] (JavaScript) and [`icp-client`][py] (Python) and
produces byte-identical wire bytes — the conformance suite in
`icp-conformance/vectors/icp-1.0/` checks all three against the same vectors.

```rust
use stateset_icp_client::{Client, Identity, LineItem, Money, PrincipalIdentity};

let client = Client::new("https://shop.example.com", Identity::generate());
let caps = client.well_known()?;
let merchant = caps["merchant_aid"].as_str().unwrap();
let settler = caps["settler_allowlist"][0].as_str().unwrap();
let snapshot = client.inventory(merchant, settler, &["WIDGET-001"])?;
client.verify_signed_response(&snapshot)?;
```

## Delegation — the `PrincipalBinding`

An Intent may carry a **PrincipalBinding**: the principal's signed statement
that this Agent may act for it, over these verbs, up to this ceiling, until
this expiry. An enforcing handler (`ICP_TRUST_MODE=enforce`) verifies it
against the principal key its operator registered, and refuses everything
else.

Configure a client one of two ways:

```rust
use stateset_icp_client::{Client, Identity, PrincipalBindingParams, PrincipalIdentity};

// (a) Sign here — convenient for tests and single-tenant services.
let key = PrincipalIdentity::from_seed_hex(&std::env::var("ICP_PRINCIPAL_SEED_HEX").unwrap())?;
let client = Client::new("https://shop.example.com", Identity::generate())
    .with_principal_identity("did:web:my-store.example", key)?;

// (b) Sign elsewhere — the production shape: the Agent never holds the
//     principal's key, it just carries the finished binding.
let identity = Identity::generate();
let binding = PrincipalBindingParams::new("did:web:my-store.example", identity.aid())
    .verbs(["purchase.create"])
    .sign(&kms_held_principal_key)?;   // signed offline, or in a KMS
let agent = Client::new("https://shop.example.com", identity)
    .with_principal_binding(binding)?;
```

Hand the merchant `PrincipalIdentity::ed_pubkey_hex()`; it goes in their
`ICP_PRINCIPAL_KEYS_JSON`.

**With neither configured, Intents carry no `principal_binding` at all** — the
key is absent, not null — and the handler decides whether an undelegated Agent
may transact. An enforcing one answers `delegation.required`. That is the
honest outcome: this SDK used to ship a placeholder binding (`kid: "self"`,
`sig: "deadbeef"`) that looked like a delegation and proved nothing.

The canonical bytes a binding signs over are `canonical_json(binding)` with
the `signature` field removed — the rule the reference handler's
`checkDelegation` applies — so every other field is covered, and mutating one
after signing invalidates the signature.

The four settings that shape a binding this client signs — `with_verbs`,
`with_max_per_intent`, `with_max_per_payout`, `with_revocation_url` — are a
configuration error once `with_principal_binding` is set, rather than a silent
no-op. A binding signed elsewhere is sent verbatim, and its authority is
covered by the principal's signature: narrow it where it is signed.

**Expiry and time zones.** `PrincipalBindingParams::expires_at` takes any
RFC 3339 string and re-emits it in the millisecond `…Z` form every SDK
produces, so the same instant spelled three ways signs identical bytes. `Z`
and a numeric `±HH:MM` / `±HHMM` offset are read as written; a string with **no
offset at all** (`2026-01-01T00:00:00`) is read as **UTC**, matching the Python
SDK. JavaScript's `new Date()` reads that same string as *local* time, so pass
an explicit offset when the difference would matter — an expiry two SDKs
disagree about is a delegation that outlives itself somewhere.

## Tests

```bash
cargo test -p stateset-icp-client
```

`tests/principal_binding_vector.rs` asserts the binding against the same
fixture the JavaScript and Python suites read
(`packages/icp-client/test/fixtures/principal-binding-vector.json`).
`tests/handler_integration.rs` spawns the reference `icp-handler` and drives
every verb over the wire, in permissive **and** enforce mode; both skip
themselves when `node` is unavailable.

[npm]: https://www.npmjs.com/package/@stateset/icp-client
[py]: https://pypi.org/project/icp-client/
