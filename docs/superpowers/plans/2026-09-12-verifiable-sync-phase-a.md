# Verifiable Sync Phase A — Honest Receive Path Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every pulled VES event verify against its author's signing key, and quarantine the ones that don't, so the cryptography protects agents from each other rather than only the sequencer from agents.

**Architecture:** The sequencer gains a read endpoint that serves an agent's registered signing keys with a sequencer signature over the whole response (`VES_KEYDIR_V1`). The CLI fetches it, verifies that signature, caches keys with a TTL, and pins `(agent_id, key_id) → public_key` on first use. The pull path then verifies every envelope; anything that fails goes to a quarantine table instead of `_ves_pulled_events`, and the cursor still advances. The fabricated `applied` counter is deleted.

**Tech Stack:** Rust 1.90 / axum / sqlx / ed25519-dalek (sequencer); Node ≥ 20.20.0 ES modules / better-sqlite3 / `node --test` (CLI).

**Spec:** `docs/superpowers/specs/2026-09-12-verifiable-sync-phase-ab-design.md`

## Global Constraints

- Sequencer MSRV is **1.90**, declared as `rust-version` in `Cargo.toml`. CI runs `cargo fmt`, `clippy -D warnings`, tests against PostgreSQL, `cargo audit` and `cargo deny`.
- CLI requires **Node >= 20.20.0**. The default shell node here is 18.20.8 — run `nvm use 22.18.0` before any CLI test command.
- CLI tests use the **built-in `node --test` runner**, never vitest. ESLint config lives at `cli/eslint.config.js` and must be run from the `cli/` directory.
- CLI has **no `.ts` files**; it uses JSDoc plus `jsconfig.json` for type checking.
- Domain separators follow the existing `VES_<THING>_V1` byte-string convention in `sequencer/src/crypto/hash.rs`. The new one is exactly `b"VES_KEYDIR_V1"`.
- New CLI sync tables use the existing `_ves_` prefix: `_ves_peer_keys`, `_ves_peer_key_pins`, `_ves_quarantined_events`.
- Verification is **unconditional**. Do not add a config flag that disables it.
- `bufferToHex()` in `cli/src/sync/crypto.js` returns `0x`-prefixed hex.
- Two repos: `/home/dom/stateset-icommerce` (CLI) and `/home/dom/icommerce-app/stateset-sequencer` (sequencer). Tasks 1–3 are in the sequencer repo; Tasks 4–8 are in the CLI repo. Commit separately in each.

---

### Task 1: Key directory domain separator and hash

**Files:**
- Modify: `sequencer/src/crypto/hash.rs` (add constant beside `DOMAIN_RECEIPT` at line 56, and a hash function beside `compute_receipt_hash` at line 416)
- Test: `sequencer/src/crypto/tests.rs`

**Interfaces:**
- Consumes: `hash_canonical_json`-style domain prefixing already used for `DOMAIN_PAYLOAD_PLAIN`.
- Produces: `pub const DOMAIN_KEYDIR: &[u8] = b"VES_KEYDIR_V1";` and
  `pub fn compute_key_directory_hash(canonical_json: &[u8]) -> Hash256`.

- [ ] **Step 1: Write the failing test**

In `sequencer/src/crypto/tests.rs`, beside the existing `DOMAIN_RECEIPT` assertion at line 788:

```rust
#[test]
fn keydir_domain_tag_is_stable() {
    assert_eq!(DOMAIN_KEYDIR, b"VES_KEYDIR_V1");
}

#[test]
fn key_directory_hash_is_domain_separated() {
    let body = br#"{"agentId":"a"}"#;

    // The domain prefix must actually participate: hashing the same bytes
    // without it must not collide.
    let with_domain = compute_key_directory_hash(body);
    let plain = sha256(body);
    assert_ne!(with_domain, plain);

    // And it must be deterministic.
    assert_eq!(with_domain, compute_key_directory_hash(body));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /home/dom/icommerce-app/stateset-sequencer
cargo test -p stateset-sequencer --lib crypto::tests::keydir -- --nocapture
```

Expected: FAIL — `cannot find value DOMAIN_KEYDIR in this scope`.

- [ ] **Step 3: Write minimal implementation**

In `sequencer/src/crypto/hash.rs`, after `DOMAIN_RECEIPT` (line 56):

```rust
/// Domain separator for signed agent key-directory responses.
pub const DOMAIN_KEYDIR: &[u8] = b"VES_KEYDIR_V1";
```

And after `compute_receipt_hash`:

```rust
/// Hash a canonical-JSON key directory body under the keydir domain.
///
/// ```text
/// keydir_hash = SHA256(b"VES_KEYDIR_V1" || JCS(body))
/// ```
///
/// The caller supplies already-canonicalized JSON so the JS client can
/// reproduce the preimage byte-for-byte with its own `canonicalizeJson`.
pub fn compute_key_directory_hash(canonical_json: &[u8]) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_KEYDIR);
    hasher.update(canonical_json);
    hasher.finalize().into()
}
```

Export `DOMAIN_KEYDIR` and `compute_key_directory_hash` from `src/crypto/mod.rs` alongside the existing `DOMAIN_RECEIPT` / `compute_receipt_hash` exports.

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p stateset-sequencer --lib crypto::tests::keydir -- --nocapture
```

Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/crypto/hash.rs src/crypto/mod.rs src/crypto/tests.rs
git commit -m "feat(crypto): add VES_KEYDIR_V1 domain and key directory hash"
```

---

### Task 2: Expose the sequencer signing config

**Files:**
- Modify: `sequencer/src/infra/postgres/ves_sequencer.rs` (beside `security_profile()` at line 442)

**Interfaces:**
- Consumes: the private `signing_config: Option<SequencerSigningConfig>` field set by `with_signing_config` (line 422).
- Produces: `pub fn signing_config(&self) -> Option<&SequencerSigningConfig>` — Task 3 calls this to sign the directory, and returns 503 when it is `None`.

- [ ] **Step 1: Write the failing test**

In the `#[cfg(test)] mod tests` block of `sequencer/src/infra/postgres/ves_sequencer.rs`:

```rust
#[test]
fn signing_config_accessor_reflects_configuration() {
    let registry = std::sync::Arc::new(crate::auth::InMemoryAgentKeyRegistry::new());
    let sequencer = VesSequencer::new_for_test(registry);
    assert!(
        sequencer.signing_config().is_none(),
        "a sequencer with no signing key must report None"
    );
}
```

If `new_for_test` does not exist, construct the struct the way the surrounding tests in this file already do and keep the assertion identical.

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p stateset-sequencer --lib ves_sequencer::tests::signing_config_accessor
```

Expected: FAIL — `no method named signing_config`.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Access the receipt signing configuration, if one was supplied.
///
/// Returns `None` when `VES_SEQUENCER_SIGNING_KEY` is unset. Callers that
/// must produce a signature are expected to fail closed rather than emit
/// unsigned output.
pub const fn signing_config(&self) -> Option<&crate::crypto::pqc_signing::SequencerSigningConfig> {
    self.signing_config.as_ref()
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo test -p stateset-sequencer --lib ves_sequencer::tests::signing_config_accessor
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/infra/postgres/ves_sequencer.rs
git commit -m "feat(sequencer): expose signing config accessor"
```

---

### Task 3: Signed key directory endpoint

**Files:**
- Modify: `sequencer/src/api/handlers/agent_keys.rs` (add handler)
- Modify: `sequencer/src/api/handlers/mod.rs` (re-export)
- Modify: `sequencer/src/api/mod.rs:118` (add route beside `/v1/agents/keys`)
- Modify: `sequencer/src/api/types.rs` (request/response types)
- Test: `sequencer/tests/agent_key_directory_test.rs` (create)

**Interfaces:**
- Consumes: `AgentKeyRegistry::list_agent_keys(&tenant_id, &agent_id) -> Vec<(u32, AgentKeyEntry)>` (`src/auth/agent_keys.rs:265`); `compute_key_directory_hash` (Task 1); `signing_config()` (Task 2).
- Produces: `GET /api/v1/agents/:agent_id/signing-keys?tenant_id=<uuid>` returning the JSON in the spec, with camelCase keys. Task 5 consumes this shape.

- [ ] **Step 1: Write the failing test**

Create `sequencer/tests/agent_key_directory_test.rs`:

```rust
//! The signed agent key directory: authorization, contents, and signature.

mod common;

use common::{register_agent_key, spawn_test_server, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn directory_is_readable_by_a_peer_in_the_same_tenant() {
    let server = spawn_test_server().await;
    let author = Uuid::new_v4();
    register_agent_key(&server, server.tenant_id, author, 1).await;

    // A *different* agent in the same tenant must be able to read it —
    // that is the entire point of the endpoint.
    let peer_key = server.create_agent_api_key(server.tenant_id, Uuid::new_v4()).await;

    let response = server
        .get(&format!(
            "/api/v1/agents/{author}/signing-keys?tenant_id={}",
            server.tenant_id
        ))
        .header("Authorization", format!("ApiKey {peer_key}"))
        .send()
        .await;

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await;
    assert_eq!(body["agentId"], author.to_string());
    assert_eq!(body["keys"].as_array().unwrap().len(), 1);
    assert_eq!(body["keys"][0]["keyId"], 1);
    assert!(
        body["directorySignature"].as_str().unwrap().starts_with("0x"),
        "the response must be signed"
    );
}

#[tokio::test]
async fn directory_refuses_cross_tenant_reads() {
    let server = spawn_test_server().await;
    let author = Uuid::new_v4();
    register_agent_key(&server, server.tenant_id, author, 1).await;

    let other_tenant = Uuid::new_v4();
    let outsider_key = server.create_agent_api_key(other_tenant, Uuid::new_v4()).await;

    let response = server
        .get(&format!(
            "/api/v1/agents/{author}/signing-keys?tenant_id={}",
            server.tenant_id
        ))
        .header("Authorization", format!("ApiKey {outsider_key}"))
        .send()
        .await;

    assert_eq!(
        response.status(),
        403,
        "an agent from another tenant must not read this tenant's keys"
    );
}

#[tokio::test]
async fn directory_includes_revoked_and_expired_keys_with_markers() {
    let server = spawn_test_server().await;
    let author = Uuid::new_v4();
    register_agent_key(&server, server.tenant_id, author, 1).await;
    register_agent_key(&server, server.tenant_id, author, 2).await;
    server.revoke_agent_key(server.tenant_id, author, 1).await;

    let body = server.get_json_as_admin(&format!(
        "/api/v1/agents/{author}/signing-keys?tenant_id={}",
        server.tenant_id
    )).await;

    let keys = body["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 2, "history must be served, not just active keys");
    let revoked = keys.iter().find(|k| k["keyId"] == 1).unwrap();
    assert!(
        !revoked["revokedAt"].is_null(),
        "a revoked key must still be served, marked revoked, so historical \
         events signed by it can be verified"
    );
}

#[tokio::test]
async fn directory_is_unavailable_without_a_signing_key() {
    let server = spawn_test_server_without_signing_key().await;
    let author = Uuid::new_v4();

    let response = server
        .get_as_admin(&format!(
            "/api/v1/agents/{author}/signing-keys?tenant_id={}",
            server.tenant_id
        ))
        .await;

    assert_eq!(
        response.status(),
        503,
        "an unsigned key directory must not be served at all"
    );
}
```

Add `spawn_test_server_without_signing_key`, `register_agent_key`, `revoke_agent_key`, `create_agent_api_key`, `get_json_as_admin` and `get_as_admin` to `sequencer/tests/common/mod.rs` if absent, following the helpers already used by `tests/api_integration_test.rs`.

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /home/dom/icommerce-app/stateset-sequencer
export DATABASE_URL="postgres://sequencer:sequencer@localhost:5433/stateset_sequencer"
cargo test --test agent_key_directory_test
```

Expected: FAIL — all four tests 404, because the route does not exist.

- [ ] **Step 3: Write minimal implementation**

In `sequencer/src/api/types.rs`:

```rust
/// Query for `GET /api/v1/agents/:agent_id/signing-keys`.
#[derive(Debug, Deserialize)]
pub struct AgentSigningKeysQuery {
    pub tenant_id: Uuid,
}

/// One registered signing key, as served by the directory.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSigningKeyEntry {
    pub key_id: u32,
    pub algorithm: String,
    pub public_key: String,
    pub public_key_bundle: Option<serde_json::Value>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Signed key-directory response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSigningKeysResponse {
    pub agent_id: Uuid,
    pub tenant_id: Uuid,
    pub keys: Vec<AgentSigningKeyEntry>,
    pub signed_at: DateTime<Utc>,
    pub directory_signature: String,
}
```

In `sequencer/src/api/handlers/agent_keys.rs`:

```rust
/// GET /api/v1/agents/:agent_id/signing-keys - Signed peer key directory.
///
/// Readable by any authenticated agent within the same tenant. Serves the
/// agent's full key history — including revoked and expired keys, each with
/// its validity window — so that peers can verify events signed by a key that
/// has since rotated. The client, not the server, decides whether a given
/// event falls inside its key's window.
#[instrument(skip(state, auth), fields(tenant_id = %query.tenant_id, agent_id = %agent_id))]
pub async fn list_agent_signing_keys(
    State(state): State<AppState>,
    Extension(AuthContextExt(auth)): Extension<AuthContextExt>,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<AgentSigningKeysQuery>,
) -> Result<Json<AgentSigningKeysResponse>, (StatusCode, String)> {
    // Tenant membership, not self-or-admin: peers must read each other's keys.
    if auth.tenant_id != query.tenant_id && !auth.is_admin() {
        return Err((StatusCode::FORBIDDEN, "Access denied".to_string()));
    }

    // Fail closed: an unsigned key directory invites clients to trust key
    // material with no provenance, which is worse than having no directory.
    let signing_config = state.ves_sequencer.signing_config().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Key directory unavailable: sequencer signing key not configured".to_string(),
    ))?;

    let entries = state
        .agent_key_registry
        .list_agent_keys(&query.tenant_id, &agent_id)
        .await
        .map_err(internal_error)?;

    let mut keys: Vec<AgentSigningKeyEntry> = entries
        .into_iter()
        .map(|(key_id, entry)| AgentSigningKeyEntry {
            key_id,
            algorithm: format!("{:?}", entry.key_algorithm).to_lowercase(),
            public_key: public_key_to_hex(&entry.public_key),
            public_key_bundle: entry
                .public_key_bundle
                .as_ref()
                .map(|b| serde_json::to_value(b).unwrap_or(serde_json::Value::Null)),
            valid_from: entry.valid_from,
            valid_to: entry.valid_to,
            revoked_at: entry.revoked_at,
        })
        .collect();
    // Deterministic order so the signed preimage is reproducible.
    keys.sort_by_key(|k| k.key_id);

    let signed_at = Utc::now();
    let body = serde_json::json!({
        "agentId": agent_id,
        "tenantId": query.tenant_id,
        "keys": keys,
        "signedAt": signed_at,
    });
    let canonical = canonicalize_json(&body).map_err(internal_error)?;
    let hash = compute_key_directory_hash(canonical.as_bytes());
    let (_scheme, signature, _bundle) = signing_config
        .sign_receipt(&hash)
        .map_err(|e| internal_error(e))?;

    Ok(Json(AgentSigningKeysResponse {
        agent_id,
        tenant_id: query.tenant_id,
        keys,
        signed_at,
        directory_signature: format!("0x{}", hex::encode(signature)),
    }))
}
```

Use the crate's existing JCS helper for `canonicalize_json` — the same one behind `hash_canonical_json` in `src/crypto/hash.rs`. Re-export the handler from `src/api/handlers/mod.rs`, then register the route in `src/api/mod.rs` beside line 118:

```rust
.route(
    "/v1/agents/:agent_id/signing-keys",
    get(handlers::list_agent_signing_keys),
)
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --test agent_key_directory_test
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

Expected: 4 passed; clippy and fmt clean.

- [ ] **Step 5: Commit**

```bash
git add src/api/ tests/agent_key_directory_test.rs tests/common/
git commit -m "feat(api): serve a sequencer-signed agent key directory"
```

---

### Task 4: Quarantine and peer key tables

**Files:**
- Modify: `cli/src/sync/outbox.js` (schema block near line 171; storage methods near line 1252)
- Test: `cli/test/unit/sync-quarantine.test.js` (create)

**Interfaces:**
- Consumes: the existing `_ves_pulled_events` schema and `storePulledEvents(events)` (`outbox.js:1252`).
- Produces on the `Outbox` class:
  - `storeQuarantinedEvents(events, reason)` — `reason` is one of `signature_invalid`, `key_unresolved`, `key_outside_validity_window`, `key_revoked`, `peer_key_conflict`
  - `getQuarantinedEvents(limit = 1000)`
  - `deleteQuarantinedEvent(eventId)`
  - `getPeerKeys(agentId)` / `upsertPeerKeys(agentId, keys, fetchedAt)`
  - `getPeerKeyPin(agentId, keyId)` / `pinPeerKey(agentId, keyId, publicKey)`

- [ ] **Step 1: Write the failing test**

Create `cli/test/unit/sync-quarantine.test.js`:

```js
/**
 * Unit tests for the quarantine and peer-key tables in sync/outbox.js.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';

function sampleEvent(overrides = {}) {
  return {
    sequenceNumber: 1,
    eventId: '11111111-1111-1111-1111-111111111111',
    tenantId: '22222222-2222-2222-2222-222222222222',
    storeId: '33333333-3333-3333-3333-333333333333',
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'order.created',
    vesVersion: 1,
    payload: { total: 99.99 },
    payloadKind: 0,
    payloadPlainHash: '0x00',
    payloadCipherHash: '0x00',
    agentKeyId: 1,
    agentSignature: '0xdead',
    baseVersion: 0,
    createdAt: '2026-09-12T00:00:00.000Z',
    sequencedAt: '2026-09-12T00:00:01.000Z',
    sourceAgent: '44444444-4444-4444-4444-444444444444',
    ...overrides,
  };
}

describe('quarantine storage', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('stores quarantined events separately from pulled events', () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

    assert.equal(outbox.getPulledEvents().length, 0, 'quarantined events must never reach application reads');
    const quarantined = outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(quarantined[0].reason, 'signature_invalid');
    assert.equal(quarantined[0].eventId, '11111111-1111-1111-1111-111111111111');
  });

  it('removes a quarantined event once it has been promoted', () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');
    outbox.deleteQuarantinedEvent('11111111-1111-1111-1111-111111111111');
    assert.equal(outbox.getQuarantinedEvents().length, 0);
  });
});

describe('peer key storage', () => {
  let db;
  let outbox;
  const agentId = '44444444-4444-4444-4444-444444444444';

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('round-trips cached peer keys', () => {
    outbox.upsertPeerKeys(
      agentId,
      [{ keyId: 1, algorithm: 'ed25519', publicKey: '0xaa', publicKeyBundle: null, validFrom: null, validTo: null, revokedAt: null }],
      '2026-09-12T00:00:00.000Z',
    );

    const keys = outbox.getPeerKeys(agentId);
    assert.equal(keys.length, 1);
    assert.equal(keys[0].keyId, 1);
    assert.equal(keys[0].publicKey, '0xaa');
    assert.equal(keys[0].fetchedAt, '2026-09-12T00:00:00.000Z');
  });

  it('records a pin and reads it back', () => {
    outbox.pinPeerKey(agentId, 1, '0xaa');
    assert.equal(outbox.getPeerKeyPin(agentId, 1).publicKey, '0xaa');
    assert.equal(outbox.getPeerKeyPin(agentId, 2), null);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /home/dom/stateset-icommerce && nvm use 22.18.0
node --test cli/test/unit/sync-quarantine.test.js
```

Expected: FAIL — `outbox.storeQuarantinedEvents is not a function`.

- [ ] **Step 3: Write minimal implementation**

Add to the schema block in `cli/src/sync/outbox.js` after `_ves_pulled_events` (line 171-204):

```sql
CREATE TABLE IF NOT EXISTS _ves_quarantined_events (
    event_id TEXT PRIMARY KEY,
    sequence_number INTEGER NOT NULL,
    command_id TEXT,
    tenant_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    event_type TEXT NOT NULL,

    ves_version INTEGER NOT NULL DEFAULT 1,
    payload TEXT NOT NULL,
    payload_kind INTEGER NOT NULL DEFAULT 0,
    payload_encrypted TEXT,
    payload_plain_hash TEXT NOT NULL,
    payload_cipher_hash TEXT NOT NULL,

    agent_key_id INTEGER NOT NULL,
    agent_signature TEXT NOT NULL,
    agent_signature_scheme INTEGER NOT NULL DEFAULT 0,
    agent_signature_bundle TEXT,

    base_version INTEGER,
    created_at TEXT NOT NULL,
    sequenced_at TEXT NOT NULL,
    source_agent TEXT NOT NULL,

    reason TEXT NOT NULL,
    quarantined_at TEXT NOT NULL DEFAULT (datetime('now')),

    CHECK(json_valid(payload))
);

CREATE INDEX IF NOT EXISTS idx_ves_quarantined_source
    ON _ves_quarantined_events (source_agent, reason);

CREATE TABLE IF NOT EXISTS _ves_peer_keys (
    agent_id TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    algorithm TEXT NOT NULL,
    public_key TEXT NOT NULL,
    public_key_bundle TEXT,
    valid_from TEXT,
    valid_to TEXT,
    revoked_at TEXT,
    fetched_at TEXT NOT NULL,
    PRIMARY KEY (agent_id, key_id)
);

CREATE TABLE IF NOT EXISTS _ves_peer_key_pins (
    agent_id TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    public_key TEXT NOT NULL,
    pinned_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (agent_id, key_id)
);
```

Add the methods to the `Outbox` class beside `storePulledEvents`:

```js
  /**
   * Store events that failed verification. These are never returned by
   * getPulledEvents, so application reads cannot see unverified state.
   * @param {Array<Object>} events
   * @param {'signature_invalid'|'key_unresolved'|'key_outside_validity_window'|'key_revoked'|'peer_key_conflict'} reason
   */
  storeQuarantinedEvents(events, reason) {
    this.initialize();

    const stmt = this.db.prepare(`
      INSERT OR REPLACE INTO _ves_quarantined_events (
        event_id, sequence_number, command_id, tenant_id, store_id,
        entity_type, entity_id, event_type,
        ves_version, payload, payload_kind, payload_encrypted,
        payload_plain_hash, payload_cipher_hash,
        agent_key_id, agent_signature, agent_signature_scheme, agent_signature_bundle,
        base_version, created_at, sequenced_at, source_agent, reason
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);

    const transaction = this.db.transaction(() => {
      for (const event of events) {
        stmt.run(
          event.eventId,
          event.sequenceNumber,
          event.commandId || null,
          event.tenantId,
          event.storeId,
          event.entityType,
          event.entityId,
          event.eventType,
          event.vesVersion || 1,
          JSON.stringify(event.payload),
          event.payloadKind || 0,
          event.payloadEncrypted ? JSON.stringify(event.payloadEncrypted) : null,
          event.payloadPlainHash,
          event.payloadCipherHash,
          event.agentKeyId,
          event.agentSignature,
          event.agentSignatureScheme || 0,
          event.agentSignatureBundle ? JSON.stringify(event.agentSignatureBundle) : null,
          event.baseVersion ?? null,
          event.createdAt,
          event.sequencedAt,
          event.sourceAgent,
          reason,
        );
      }
    });

    transaction();
  }

  /**
   * @param {number} [limit]
   * @returns {Array<Object>}
   */
  getQuarantinedEvents(limit = 1000) {
    this.initialize();
    return this.db
      .prepare(
        `SELECT * FROM _ves_quarantined_events
         ORDER BY sequence_number ASC LIMIT ?`,
      )
      .all(limit)
      .map((row) => ({
        eventId: row.event_id,
        sequenceNumber: row.sequence_number,
        commandId: row.command_id,
        tenantId: row.tenant_id,
        storeId: row.store_id,
        entityType: row.entity_type,
        entityId: row.entity_id,
        eventType: row.event_type,
        vesVersion: row.ves_version,
        payload: JSON.parse(row.payload),
        payloadKind: row.payload_kind,
        payloadEncrypted: row.payload_encrypted ? JSON.parse(row.payload_encrypted) : null,
        payloadPlainHash: row.payload_plain_hash,
        payloadCipherHash: row.payload_cipher_hash,
        agentKeyId: row.agent_key_id,
        agentSignature: row.agent_signature,
        agentSignatureScheme: row.agent_signature_scheme,
        agentSignatureBundle: row.agent_signature_bundle
          ? JSON.parse(row.agent_signature_bundle)
          : null,
        baseVersion: row.base_version,
        createdAt: row.created_at,
        sequencedAt: row.sequenced_at,
        sourceAgent: row.source_agent,
        reason: row.reason,
        quarantinedAt: row.quarantined_at,
      }));
  }

  /**
   * @param {string} eventId
   */
  deleteQuarantinedEvent(eventId) {
    this.initialize();
    this.db.prepare('DELETE FROM _ves_quarantined_events WHERE event_id = ?').run(eventId);
  }

  /**
   * @param {string} agentId
   * @returns {Array<Object>}
   */
  getPeerKeys(agentId) {
    this.initialize();
    return this.db
      .prepare('SELECT * FROM _ves_peer_keys WHERE agent_id = ? ORDER BY key_id ASC')
      .all(agentId)
      .map((row) => ({
        keyId: row.key_id,
        algorithm: row.algorithm,
        publicKey: row.public_key,
        publicKeyBundle: row.public_key_bundle ? JSON.parse(row.public_key_bundle) : null,
        validFrom: row.valid_from,
        validTo: row.valid_to,
        revokedAt: row.revoked_at,
        fetchedAt: row.fetched_at,
      }));
  }

  /**
   * @param {string} agentId
   * @param {Array<Object>} keys
   * @param {string} fetchedAt - ISO timestamp
   */
  upsertPeerKeys(agentId, keys, fetchedAt) {
    this.initialize();
    const stmt = this.db.prepare(`
      INSERT OR REPLACE INTO _ves_peer_keys (
        agent_id, key_id, algorithm, public_key, public_key_bundle,
        valid_from, valid_to, revoked_at, fetched_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `);
    const transaction = this.db.transaction(() => {
      for (const key of keys) {
        stmt.run(
          agentId,
          key.keyId,
          key.algorithm,
          key.publicKey,
          key.publicKeyBundle ? JSON.stringify(key.publicKeyBundle) : null,
          key.validFrom || null,
          key.validTo || null,
          key.revokedAt || null,
          fetchedAt,
        );
      }
    });
    transaction();
  }

  /**
   * @param {string} agentId
   * @param {number} keyId
   * @returns {{publicKey: string, pinnedAt: string}|null}
   */
  getPeerKeyPin(agentId, keyId) {
    this.initialize();
    const row = this.db
      .prepare('SELECT public_key, pinned_at FROM _ves_peer_key_pins WHERE agent_id = ? AND key_id = ?')
      .get(agentId, keyId);
    return row ? { publicKey: row.public_key, pinnedAt: row.pinned_at } : null;
  }

  /**
   * @param {string} agentId
   * @param {number} keyId
   * @param {string} publicKey
   */
  pinPeerKey(agentId, keyId, publicKey) {
    this.initialize();
    this.db
      .prepare('INSERT OR IGNORE INTO _ves_peer_key_pins (agent_id, key_id, public_key) VALUES (?, ?, ?)')
      .run(agentId, keyId, publicKey);
  }
```

- [ ] **Step 4: Run test to verify it passes**

```bash
node --test cli/test/unit/sync-quarantine.test.js
```

Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add cli/src/sync/outbox.js cli/test/unit/sync-quarantine.test.js
git commit -m "feat(sync): add quarantine and peer key tables"
```

---

### Task 5: Client fetch with directory signature verification

**Files:**
- Modify: `cli/src/sync/client.js:979` (replace the dead `getAgentKeys`)
- Test: `cli/test/unit/sync-client.test.js` (append)

**Interfaces:**
- Consumes: the Task 3 endpoint; `canonicalizeJson` and `DOMAIN` from `cli/src/sync/crypto.js` (lines 499, 350).
- Produces: `async getAgentSigningKeys(agentId)` returning
  `{ agentId, tenantId, keys: [{keyId, algorithm, publicKey, publicKeyBundle, validFrom, validTo, revokedAt}], signedAt }`.
  Throws `Error('Key directory signature invalid')` when the signature does not verify.

**Note:** `getAgentKeys` at line 979 calls `GET /api/v1/agents/keys`, which is registered POST-only on the sequencer and therefore 405s. It is dead code. Replace it rather than adding a second method.

- [ ] **Step 1: Write the failing test**

Append to `cli/test/unit/sync-client.test.js`:

```js
describe('getAgentSigningKeys', () => {
  it('verifies the directory signature before returning keys', async () => {
    const signingKey = crypto.generateKeyPairSync('ed25519');
    const sequencerPublicKey = signingKey.publicKey
      .export({ type: 'spki', format: 'der' })
      .subarray(-32);

    const body = {
      agentId: '44444444-4444-4444-4444-444444444444',
      tenantId: '22222222-2222-2222-2222-222222222222',
      keys: [
        {
          keyId: 1,
          algorithm: 'ed25519',
          publicKey: '0xaa',
          publicKeyBundle: null,
          validFrom: null,
          validTo: null,
          revokedAt: null,
        },
      ],
      signedAt: '2026-09-12T00:00:00.000Z',
    };
    const preimage = Buffer.concat([
      Buffer.from('VES_KEYDIR_V1'),
      Buffer.from(canonicalizeJson(body)),
    ]);
    const hash = crypto.createHash('sha256').update(preimage).digest();
    const signature = crypto.sign(null, hash, signingKey.privateKey);

    const client = createSequencerClient({
      sequencerUrl: 'http://localhost:8080',
      tenantId: body.tenantId,
      apiKey: 'test',
      sequencerPublicKey,
      allowInsecureTransport: true,
    });
    client._request = async () => ({ ...body, directorySignature: `0x${signature.toString('hex')}` });

    const result = await client.getAgentSigningKeys(body.agentId);
    assert.equal(result.keys.length, 1);
    assert.equal(result.keys[0].keyId, 1);
  });

  it('rejects a tampered directory response', async () => {
    const signingKey = crypto.generateKeyPairSync('ed25519');
    const sequencerPublicKey = signingKey.publicKey
      .export({ type: 'spki', format: 'der' })
      .subarray(-32);

    const body = {
      agentId: '44444444-4444-4444-4444-444444444444',
      tenantId: '22222222-2222-2222-2222-222222222222',
      keys: [{ keyId: 1, algorithm: 'ed25519', publicKey: '0xaa', publicKeyBundle: null, validFrom: null, validTo: null, revokedAt: null }],
      signedAt: '2026-09-12T00:00:00.000Z',
    };
    const preimage = Buffer.concat([Buffer.from('VES_KEYDIR_V1'), Buffer.from(canonicalizeJson(body))]);
    const hash = crypto.createHash('sha256').update(preimage).digest();
    const signature = crypto.sign(null, hash, signingKey.privateKey);

    const client = createSequencerClient({
      sequencerUrl: 'http://localhost:8080',
      tenantId: body.tenantId,
      apiKey: 'test',
      sequencerPublicKey,
      allowInsecureTransport: true,
    });
    // An attacker swaps in their own key but cannot re-sign.
    client._request = async () => ({
      ...body,
      keys: [{ ...body.keys[0], publicKey: '0xbb' }],
      directorySignature: `0x${signature.toString('hex')}`,
    });

    await assert.rejects(
      () => client.getAgentSigningKeys(body.agentId),
      /Key directory signature invalid/,
    );
  });
});
```

Add `canonicalizeJson` to the existing `crypto.js` import block at the top of the file.

- [ ] **Step 2: Run test to verify it fails**

```bash
node --test cli/test/unit/sync-client.test.js
```

Expected: FAIL — `client.getAgentSigningKeys is not a function`.

- [ ] **Step 3: Write minimal implementation**

Replace `getAgentKeys` in `cli/src/sync/client.js` with:

```js
  /**
   * Fetch an agent's signed key directory.
   *
   * The response carries a sequencer signature over the whole body. An invalid
   * signature is a hard refusal — that is a MITM, not a bad key.
   *
   * @param {string} agentId
   * @returns {Promise<{agentId: string, tenantId: string, keys: Array<Object>, signedAt: string}>}
   */
  async getAgentSigningKeys(agentId) {
    const params = new URLSearchParams({ tenant_id: this.config.tenantId });
    const response = await this._request(
      'GET',
      `/api/v1/agents/${agentId}/signing-keys?${params}`,
    );

    const sequencerPublicKey =
      this.config.sequencerPublicKey ?? this.config.sequencer?.publicKey;
    if (!sequencerPublicKey) {
      throw new Error(
        'sequencerPublicKey is required to verify the key directory signature',
      );
    }

    const { directorySignature, ...body } = response;
    if (!directorySignature) {
      throw new Error('Key directory signature invalid: response is unsigned');
    }

    const preimage = Buffer.concat([
      DOMAIN.KEYDIR,
      Buffer.from(canonicalizeJson(body)),
    ]);
    const hash = createHash('sha256').update(preimage).digest();
    const valid = verifyEventSignature(
      hash,
      hexToBuffer(directorySignature),
      normalizeVerificationPublicKeyBundle(sequencerPublicKey)?.ed25519PublicKey ??
        sequencerPublicKey,
    );
    if (!valid) {
      throw new Error('Key directory signature invalid');
    }

    return body;
  }
```

Add `KEYDIR: Buffer.from('VES_KEYDIR_V1')` to the `DOMAIN` object in `cli/src/sync/crypto.js:350`, and add `canonicalizeJson`, `verifyEventSignature` and `hexToBuffer` to `client.js`'s imports from `./crypto.js` if not already present. Import `createHash` from `node:crypto`.

- [ ] **Step 4: Run test to verify it passes**

```bash
node --test cli/test/unit/sync-client.test.js
```

Expected: PASS, including both new tests.

- [ ] **Step 5: Commit**

```bash
git add cli/src/sync/client.js cli/src/sync/crypto.js cli/test/unit/sync-client.test.js
git commit -m "feat(sync): fetch and verify the signed agent key directory"
```

---

### Task 6: PeerKeyDirectory with TTL cache and pinning

**Files:**
- Create: `cli/src/sync/key-directory.js`
- Test: `cli/test/unit/sync-key-directory.test.js` (create)

**Interfaces:**
- Consumes: `Outbox.getPeerKeys/upsertPeerKeys/getPeerKeyPin/pinPeerKey` (Task 4); `SequencerClient.getAgentSigningKeys` (Task 5).
- Produces:
  ```js
  class PeerKeyDirectory {
    constructor(outbox, client, config)
    async resolve(agentId, keyId, atTime) // → {publicKey, publicKeyBundle} | {error: reason}
    async refresh(agentId)                // → keys, or throws
  }
  export function createPeerKeyDirectory(outbox, client, config)
  ```
  `resolve` returns `{ error }` where `error` is one of the five quarantine reasons. Task 7 maps that straight to the quarantine reason.

- [ ] **Step 1: Write the failing test**

Create `cli/test/unit/sync-key-directory.test.js`:

```js
/**
 * Unit tests for sync/key-directory.js — peer key resolution, TTL, pinning.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { createPeerKeyDirectory } from '../../src/sync/key-directory.js';

const AGENT = '44444444-4444-4444-4444-444444444444';

function stubClient(keys) {
  return {
    calls: 0,
    async getAgentSigningKeys() {
      this.calls += 1;
      return { agentId: AGENT, keys, signedAt: new Date().toISOString() };
    },
  };
}

function key(overrides = {}) {
  return {
    keyId: 1,
    algorithm: 'ed25519',
    publicKey: '0xaa',
    publicKeyBundle: null,
    validFrom: null,
    validTo: null,
    revokedAt: null,
    ...overrides,
  };
}

describe('PeerKeyDirectory', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('resolves a key and pins it on first use', async () => {
    const client = stubClient([key()]);
    const dir = createPeerKeyDirectory(outbox, client, {});

    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.publicKey, '0xaa');
    assert.equal(outbox.getPeerKeyPin(AGENT, 1).publicKey, '0xaa');
  });

  it('serves from cache within the TTL', async () => {
    const client = stubClient([key()]);
    const dir = createPeerKeyDirectory(outbox, client, { peerKeyTtlSeconds: 300 });

    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(client.calls, 1, 'a second resolve inside the TTL must not refetch');
  });

  it('refuses a changed key for an already-pinned key_id', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    // Same key_id, different key material — the sequencer is lying or compromised.
    const attacker = createPeerKeyDirectory(outbox, stubClient([key({ publicKey: '0xbb' })]), {
      peerKeyTtlSeconds: 0,
    });
    const resolved = await attacker.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'peer_key_conflict');
  });

  it('accepts a new key_id as rotation', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    const rotated = createPeerKeyDirectory(
      outbox,
      stubClient([key(), key({ keyId: 2, publicKey: '0xbb' })]),
      { peerKeyTtlSeconds: 0 },
    );
    const resolved = await rotated.resolve(AGENT, 2, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.publicKey, '0xbb');
    assert.equal(outbox.getPeerKeyPin(AGENT, 2).publicKey, '0xbb');
  });

  it('rejects an event created outside the key validity window', async () => {
    const dir = createPeerKeyDirectory(
      outbox,
      stubClient([key({ validFrom: '2026-09-01T00:00:00.000Z', validTo: '2026-09-10T00:00:00.000Z' })]),
      {},
    );
    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_outside_validity_window');
  });

  it('rejects an event signed after the key was revoked', async () => {
    const dir = createPeerKeyDirectory(
      outbox,
      stubClient([key({ revokedAt: '2026-09-11T00:00:00.000Z' })]),
      {},
    );
    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_revoked');
  });

  it('reports key_unresolved when the directory has no such key', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    const resolved = await dir.resolve(AGENT, 9, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_unresolved');
  });

  it('serves stale cache when refresh fails, then gives up past the stale limit', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), { peerKeyTtlSeconds: 0 });
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    const failing = createPeerKeyDirectory(
      outbox,
      { async getAgentSigningKeys() { throw new Error('unreachable'); } },
      { peerKeyTtlSeconds: 0, peerKeyMaxStaleSeconds: 86400 },
    );
    const stillOk = await failing.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(stillOk.publicKey, '0xaa', 'a directory outage must not stop verification');

    const tooStale = createPeerKeyDirectory(
      outbox,
      { async getAgentSigningKeys() { throw new Error('unreachable'); } },
      { peerKeyTtlSeconds: 0, peerKeyMaxStaleSeconds: 0 },
    );
    assert.equal((await tooStale.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z')).error, 'key_unresolved');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
node --test cli/test/unit/sync-key-directory.test.js
```

Expected: FAIL — cannot find module `../../src/sync/key-directory.js`.

- [ ] **Step 3: Write minimal implementation**

Create `cli/src/sync/key-directory.js`:

```js
/**
 * Peer Key Directory
 *
 * Resolves another agent's signing key so the receive path can verify what
 * that agent wrote. Keys come from the sequencer's signed directory, are
 * cached with a TTL, and are pinned on first use: a changed public key for an
 * already-pinned key_id is refused, because a sequencer that can silently swap
 * key material can forge any agent's events.
 */

const DEFAULT_TTL_SECONDS = 300;
const DEFAULT_MAX_STALE_SECONDS = 86400;

export class PeerKeyDirectory {
  /**
   * @param {import('./outbox.js').Outbox} outbox
   * @param {{getAgentSigningKeys: (agentId: string) => Promise<Object>}} client
   * @param {Object} [config]
   */
  constructor(outbox, client, config = {}) {
    this.outbox = outbox;
    this.client = client;
    this.ttlSeconds = config.peerKeyTtlSeconds ?? DEFAULT_TTL_SECONDS;
    this.maxStaleSeconds = config.peerKeyMaxStaleSeconds ?? DEFAULT_MAX_STALE_SECONDS;
  }

  /**
   * Resolve the key an event claims to be signed by.
   *
   * @param {string} agentId
   * @param {number} keyId
   * @param {string} atTime - the event's createdAt, ISO 8601
   * @returns {Promise<{publicKey: string, publicKeyBundle: Object|null}|{error: string}>}
   */
  async resolve(agentId, keyId, atTime) {
    let keys = this.outbox.getPeerKeys(agentId);
    const fetchedAt = keys.length ? Date.parse(keys[0].fetchedAt) : 0;
    const ageSeconds = (Date.now() - fetchedAt) / 1000;

    if (!keys.length || ageSeconds > this.ttlSeconds) {
      try {
        keys = await this.refresh(agentId);
      } catch {
        // A directory outage must not stop verification of keys we already
        // hold — but we will not trust them forever.
        if (!keys.length || ageSeconds > this.maxStaleSeconds) {
          return { error: 'key_unresolved' };
        }
      }
    }

    const key = keys.find((k) => k.keyId === keyId);
    if (!key) return { error: 'key_unresolved' };

    const pin = this.outbox.getPeerKeyPin(agentId, keyId);
    if (pin && pin.publicKey !== key.publicKey) {
      return { error: 'peer_key_conflict' };
    }

    const at = Date.parse(atTime);
    if (key.revokedAt && at >= Date.parse(key.revokedAt)) {
      return { error: 'key_revoked' };
    }
    if (key.validFrom && at < Date.parse(key.validFrom)) {
      return { error: 'key_outside_validity_window' };
    }
    if (key.validTo && at > Date.parse(key.validTo)) {
      return { error: 'key_outside_validity_window' };
    }

    if (!pin) this.outbox.pinPeerKey(agentId, keyId, key.publicKey);

    return { publicKey: key.publicKey, publicKeyBundle: key.publicKeyBundle };
  }

  /**
   * Fetch and cache the directory for one agent.
   * @param {string} agentId
   * @returns {Promise<Array<Object>>}
   */
  async refresh(agentId) {
    const directory = await this.client.getAgentSigningKeys(agentId);
    const keys = directory.keys || [];
    this.outbox.upsertPeerKeys(agentId, keys, new Date().toISOString());
    return this.outbox.getPeerKeys(agentId);
  }
}

/**
 * @param {import('./outbox.js').Outbox} outbox
 * @param {Object} client
 * @param {Object} [config]
 * @returns {PeerKeyDirectory}
 */
export function createPeerKeyDirectory(outbox, client, config = {}) {
  return new PeerKeyDirectory(outbox, client, config);
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
node --test cli/test/unit/sync-key-directory.test.js
```

Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add cli/src/sync/key-directory.js cli/test/unit/sync-key-directory.test.js
git commit -m "feat(sync): add peer key directory with TTL cache and pinning"
```

---

### Task 7: Verify on pull, quarantine failures, honest counters

**Files:**
- Modify: `cli/src/sync/engine.js:382-500` (the `pull` method)
- Test: `cli/test/unit/sync-engine-verification.test.js` (create)

**Interfaces:**
- Consumes: `createPeerKeyDirectory` (Task 6); `client.verifyEventSignature(envelope, publicKey)` (`client.js:792`); `outbox.storeQuarantinedEvents` (Task 4).
- Produces: the `pull` event payload `{ pulled, verified, quarantined, stored, conflicts }`. **No `applied` field.**

- [ ] **Step 1: Write the failing test**

Create `cli/test/unit/sync-engine-verification.test.js`:

```js
/**
 * The pull path must verify every event against its author's key and
 * quarantine what it cannot verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { SyncEngine } from '../../src/sync/engine.js';

const TENANT = '22222222-2222-2222-2222-222222222222';
const STORE = '33333333-3333-3333-3333-333333333333';
const AGENT = '44444444-4444-4444-4444-444444444444';

function pulledEvent(overrides = {}) {
  return {
    sequenceNumber: 1,
    sequencedAt: '2026-09-12T00:00:01.000Z',
    envelope: {
      eventId: '11111111-1111-1111-1111-111111111111',
      tenantId: TENANT,
      storeId: STORE,
      entityType: 'order',
      entityId: 'ORD-1',
      eventType: 'order.created',
      vesVersion: 1,
      payload: { total: 99.99 },
      payloadKind: 0,
      payloadPlainHash: '0x00',
      payloadCipherHash: '0x00',
      agentKeyId: 1,
      agentSignature: '0xdead',
      baseVersion: 0,
      createdAt: '2026-09-12T00:00:00.000Z',
      sourceAgent: AGENT,
      ...overrides,
    },
  };
}

function buildEngine(db, { verifies = true, resolves = true } = {}) {
  const outbox = createOutbox(db, {});
  const engine = new SyncEngine({
    identity: { tenantId: TENANT, storeId: STORE, agentId: 'self' },
  });
  engine.outbox = outbox;
  engine.client = {
    async pull() {
      return { events: [pulledEvent()], nextSequence: 2, headSequence: 1 };
    },
    verifyEventSignature: () => verifies,
  };
  engine.keyDirectory = {
    async resolve() {
      return resolves ? { publicKey: '0xaa', publicKeyBundle: null } : { error: 'key_unresolved' };
    },
  };
  return { engine, outbox };
}

describe('pull verification', () => {
  let db;

  beforeEach(() => {
    db = new Database(':memory:');
  });

  afterEach(() => db.close());

  it('stores an event whose signature verifies', async () => {
    const { engine, outbox } = buildEngine(db);
    const result = await engine.pull();

    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
    assert.equal(result.verified, 1);
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 1);
  });

  it('quarantines an event whose signature does not verify', async () => {
    const { engine, outbox } = buildEngine(db, { verifies: false });
    const result = await engine.pull();

    assert.equal(outbox.getPulledEvents().length, 0, 'a forged event must never reach application reads');
    const quarantined = outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(quarantined[0].reason, 'signature_invalid');
    assert.equal(result.quarantined, 1);
  });

  it('quarantines an event whose key cannot be resolved', async () => {
    const { engine, outbox } = buildEngine(db, { resolves: false });
    await engine.pull();

    assert.equal(outbox.getQuarantinedEvents()[0].reason, 'key_unresolved');
  });

  it('advances the cursor even when every event quarantines', async () => {
    const { engine, outbox } = buildEngine(db, { verifies: false });
    await engine.pull();

    assert.equal(
      outbox.getSyncState().lastPulledSequence,
      2,
      'one bad peer must not wedge this agent’s sync',
    );
  });

  it('never reports an applied count', async () => {
    const { engine } = buildEngine(db);
    const result = await engine.pull();

    assert.equal(
      'applied' in result,
      false,
      'nothing is applied to local state until Phase D; a field claiming otherwise is the bug',
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
node --test cli/test/unit/sync-engine-verification.test.js
```

Expected: FAIL — the forged event is stored, and `applied` is present.

- [ ] **Step 3: Write minimal implementation**

In `cli/src/sync/engine.js`, import the directory and construct it where the client is constructed:

```js
import { createPeerKeyDirectory } from './key-directory.js';
```

```js
this.keyDirectory = createPeerKeyDirectory(this.outbox, this.client, this.config);
```

Replace the body of `pull()` between fetching `result` and updating sync state (currently lines 410-490) with:

```js
      // Verify every event against its author's key before it can be read.
      // Self-authored echoes are verified too: it costs nothing and keeps a
      // continuous check on our own signing path.
      const verified = [];
      const quarantined = [];

      for (const event of result.events) {
        const envelope = event.envelope;
        const resolution = await this.keyDirectory.resolve(
          envelope.sourceAgent,
          envelope.agentKeyId,
          envelope.createdAt,
        );

        const record = {
          sequenceNumber: event.sequenceNumber,
          eventId: envelope.eventId,
          commandId: envelope.commandId,
          tenantId: envelope.tenantId,
          storeId: envelope.storeId,
          entityType: envelope.entityType,
          entityId: envelope.entityId,
          eventType: envelope.eventType,
          payload: envelope.payload,
          vesVersion: envelope.vesVersion || 1,
          payloadKind: envelope.payloadKind || 0,
          payloadEncrypted: envelope.payloadEncrypted,
          payloadPlainHash: envelope.payloadPlainHash,
          payloadCipherHash: envelope.payloadCipherHash,
          agentKeyId: envelope.agentKeyId,
          agentSignature: envelope.agentSignature,
          agentSignatureScheme: envelope.agentSignatureScheme || 0,
          agentSignatureBundle: envelope.agentSignatureBundle || null,
          baseVersion: envelope.baseVersion,
          createdAt: envelope.createdAt,
          sequencedAt: event.sequencedAt,
          sourceAgent: envelope.sourceAgent,
        };

        if (resolution.error) {
          quarantined.push({ record, reason: resolution.error });
          continue;
        }

        const ok = this.client.verifyEventSignature(
          envelope,
          resolution.publicKeyBundle ?? resolution.publicKey,
        );
        if (ok) {
          verified.push(record);
        } else {
          quarantined.push({ record, reason: 'signature_invalid' });
        }
      }

      if (verified.length) {
        this.outbox.storePulledEvents(verified);
      }
      for (const reason of new Set(quarantined.map((q) => q.reason))) {
        this.outbox.storeQuarantinedEvents(
          quarantined.filter((q) => q.reason === reason).map((q) => q.record),
          reason,
        );
        this.emit('receive-verification-failed', {
          reason,
          count: quarantined.filter((q) => q.reason === reason).length,
        });
      }

      // The cursor advances regardless: one bad event must not wedge sync.
      this.outbox.updateSyncState({
        lastPulledSequence: result.nextSequence,
        headSequence: result.headSequence,
        lastSyncAt: new Date(),
      });

      const conflicts = this.detectConflicts().length;
      const summary = {
        pulled: result.events.length,
        verified: verified.length,
        quarantined: quarantined.length,
        stored: verified.length,
        conflicts,
      };
      this.emit('pull', summary);
      return summary;
```

Apply the same `{ pulled: 0, verified: 0, quarantined: 0, stored: 0, conflicts: 0 }` shape to the early-return and error branches currently at lines 394-407 and 490-499. Delete every `applied:` key in the file.

- [ ] **Step 4: Run tests to verify they pass**

```bash
node --test cli/test/unit/sync-engine-verification.test.js
node --test cli/test/unit/
cd cli && npx eslint src/ bin/
```

Expected: all pass; eslint clean.

- [ ] **Step 5: Commit**

```bash
git add cli/src/sync/engine.js cli/test/unit/sync-engine-verification.test.js
git commit -m "feat(sync): verify peer signatures on pull and quarantine failures"
```

---

### Task 8: `sync doctor`

**Files:**
- Modify: `cli/src/commands/sync.js`
- Modify: `cli/bin/stateset-sync.js` (register the subcommand)
- Test: `cli/test/unit/sync-doctor.test.js` (create)

**Interfaces:**
- Consumes: `outbox.getQuarantinedEvents/deleteQuarantinedEvent/storePulledEvents/getPeerKeyPin` (Task 4); `PeerKeyDirectory.resolve` (Task 6); `client.verifyEventSignature` (Task 5 context).
- Produces: `async function syncDoctor({ outbox, client, keyDirectory, promote = false })` returning `{ quarantined: [{reason, count}], pins: [{agentId, keyId, pinnedAt}], promoted: number }`.

- [ ] **Step 1: Write the failing test**

Create `cli/test/unit/sync-doctor.test.js`:

```js
/**
 * `sync doctor` — inspect quarantine, inspect pins, promote events that now verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { syncDoctor } from '../../src/commands/sync.js';

const AGENT = '44444444-4444-4444-4444-444444444444';

function sampleEvent() {
  return {
    sequenceNumber: 1,
    eventId: '11111111-1111-1111-1111-111111111111',
    tenantId: '22222222-2222-2222-2222-222222222222',
    storeId: '33333333-3333-3333-3333-333333333333',
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'order.created',
    vesVersion: 1,
    payload: { total: 1 },
    payloadKind: 0,
    payloadPlainHash: '0x00',
    payloadCipherHash: '0x00',
    agentKeyId: 1,
    agentSignature: '0xdead',
    baseVersion: 0,
    createdAt: '2026-09-12T00:00:00.000Z',
    sequencedAt: '2026-09-12T00:00:01.000Z',
    sourceAgent: AGENT,
  };
}

describe('syncDoctor', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('summarizes quarantined events by reason', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.deepEqual(report.quarantined, [{ reason: 'key_unresolved', count: 1 }]);
    assert.equal(report.promoted, 0);
  });

  it('promotes events that verify once the key arrives', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: { async resolve() { return { publicKey: '0xaa', publicKeyBundle: null }; } },
      promote: true,
    });

    assert.equal(report.promoted, 1);
    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
  });

  it('leaves events that still do not verify in quarantine', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => false },
      keyDirectory: { async resolve() { return { publicKey: '0xaa', publicKeyBundle: null }; } },
      promote: true,
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
  });

  it('lists current pins', async () => {
    outbox.pinPeerKey(AGENT, 1, '0xaa');
    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.equal(report.pins.length, 1);
    assert.equal(report.pins[0].agentId, AGENT);
    assert.equal(report.pins[0].keyId, 1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

```bash
node --test cli/test/unit/sync-doctor.test.js
```

Expected: FAIL — `syncDoctor` is not exported from `../../src/commands/sync.js`.

- [ ] **Step 3: Write minimal implementation**

Add to `cli/src/sync/outbox.js` (needed by the report):

```js
  /**
   * @returns {Array<{agentId: string, keyId: number, publicKey: string, pinnedAt: string}>}
   */
  getPeerKeyPins() {
    this.initialize();
    return this.db
      .prepare('SELECT agent_id, key_id, public_key, pinned_at FROM _ves_peer_key_pins ORDER BY agent_id, key_id')
      .all()
      .map((row) => ({
        agentId: row.agent_id,
        keyId: row.key_id,
        publicKey: row.public_key,
        pinnedAt: row.pinned_at,
      }));
  }
```

Add to `cli/src/commands/sync.js`:

```js
/**
 * Inspect the receive path: what is quarantined, what keys are pinned, and
 * which quarantined events now verify.
 *
 * The common benign case is an agent that pushed events before its key
 * reached the directory. Re-verification is the designed recovery.
 *
 * @param {Object} params
 * @param {import('../sync/outbox.js').Outbox} params.outbox
 * @param {Object} params.client
 * @param {Object} params.keyDirectory
 * @param {boolean} [params.promote] - re-verify and promote what now passes
 * @returns {Promise<{quarantined: Array<{reason: string, count: number}>, pins: Array<Object>, promoted: number}>}
 */
export async function syncDoctor({ outbox, client, keyDirectory, promote = false }) {
  const events = outbox.getQuarantinedEvents();

  let promoted = 0;
  if (promote) {
    for (const event of events) {
      const resolution = await keyDirectory.resolve(
        event.sourceAgent,
        event.agentKeyId,
        event.createdAt,
      );
      if (resolution.error) continue;

      const ok = client.verifyEventSignature(
        event,
        resolution.publicKeyBundle ?? resolution.publicKey,
      );
      if (!ok) continue;

      outbox.storePulledEvents([event]);
      outbox.deleteQuarantinedEvent(event.eventId);
      promoted += 1;
    }
  }

  const remaining = outbox.getQuarantinedEvents();
  const byReason = new Map();
  for (const event of remaining) {
    byReason.set(event.reason, (byReason.get(event.reason) || 0) + 1);
  }

  return {
    quarantined: [...byReason.entries()].map(([reason, count]) => ({ reason, count })),
    pins: outbox.getPeerKeyPins(),
    promoted,
  };
}
```

Register a `doctor` subcommand in `cli/bin/stateset-sync.js` following the file's existing subcommand pattern, accepting `--promote`, and printing the three sections of the report.

- [ ] **Step 4: Run tests to verify they pass**

```bash
node --test cli/test/unit/sync-doctor.test.js
node --test cli/test/unit/
cd cli && npx eslint src/ bin/
```

Expected: all pass; eslint clean.

- [ ] **Step 5: Commit**

```bash
git add cli/src/commands/sync.js cli/src/sync/outbox.js cli/bin/stateset-sync.js cli/test/unit/sync-doctor.test.js
git commit -m "feat(cli): add sync doctor for quarantine and pin inspection"
```

---

## Deployment note

The sequencer endpoint (Tasks 1–3) must be deployed **before** any CLI carrying Tasks 5–7, or every agent quarantines everything on upgrade with `key_unresolved`. Ship the sequencer first, confirm `GET /api/v1/agents/:agent_id/signing-keys` responds, then roll the CLI.

Existing deployments have no pins, so the first post-upgrade pull pins whatever the directory serves — trust-on-first-use at the population level. Operators wanting stronger initial assurance should register keys and inspect `stateset sync doctor` output before the first pull.
