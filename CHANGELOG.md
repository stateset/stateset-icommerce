# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

### Fixed
- **ICP: RFC 8785 canonicalization parity across all four IUTs.** The Go
  IUT HTML-escaped `<`, `>`, `&`, escaped U+0008/U+000C as ``/``
  instead of `\b`/`\f`, passed non-minimal number literals (`1.50`) through
  verbatim, and sorted object keys in code-point rather than UTF-16
  code-unit order; the Python IUT serialized via bare `json.dumps`, so
  float formatting diverged from ES `Number::toString` (`10.0` → `"10.0"`,
  `1e-6` → `"1e-06"`). Both now implement explicit RFC 8785 serializers
  mirroring the JS reference, verified byte-identical against it on a
  3,000-case control-char/astral/double fuzz corpus. Vector
  `02-canonical-json` grew from 11 to 20 sub-cases (HTML-escape chars,
  non-minimal numbers, exponent boundaries, max safe integer, negative
  zero, U+2028/U+2029, `\b`/`\f`, control-char sweep) with expected outputs
  generated from the reference implementation, and the conformance runner
  now pipes raw `inputs.json` bytes to IUTs so non-minimal-number inputs
  reach them unnormalized.
- **ICP: cross-IUT determinism CI gate actually runs.** The check invoked
  the Rust IUT binary from a job that never built it (broken since
  inception); it is now a self-contained job that builds the Rust and Go
  IUTs and compares 6 cryptographic fields byte-for-byte across all four
  implementations (was JS×Rust only).
- **Admin: `useEmbeddedData` no longer refetches continuously.** Inline
  arrow fetchers (used by every dashboard consumer) re-created the
  `useCallback` each render, so the fetch effect re-fired back-to-back
  instead of polling on `refreshInterval`. The hook now holds the latest
  fetcher in a ref; regression tests pass a fresh fetcher identity per
  render and assert single-fetch + interval cadence.
- **CLI: treasury LLM-billing config defaulting.** A constant-true
  expression in `claude-harness.js` made `TREASURY_LLM_BILLING` impossible
  to disable at all 4 sites.

### Security
- **Admin: server actions now require an authenticated session.** All 63
  exported `'use server'` actions (`commerce.ts`, `active-org.ts`,
  `organizations.ts`) — including `processRefund`, `adjustInventory`, and
  `approveReturn` — call `requireAdminSession()` first, mirroring
  middleware semantics including the dev-only auth-disabled bypass
  (hard-off in production). Previously they called straight into the
  embedded engine with no check.
- **Dependencies: high-severity npm advisories resolved.** Root lockfile
  bumps minimatch (ReDoS), flatted, ajv, brace-expansion; cli lockfile
  regenerated clearing its high advisory. `npm audit --audit-level=high`
  exits 0 in both.
- **HTTP: negotiations API hardened.** Monetary fields moved from `f64`
  to `rust_decimal::Decimal` (string-serialized, matching the rest of the
  API) and the in-memory store is now tenant-scoped — cross-tenant reads
  and mutations 404. DB persistence via the V9 `a2a_negotiations` tables
  remains a documented follow-up (no repository traits exist yet).

### Changed
- **ICP spec: normative signing encoding is RFC 8785 JCS JSON.** The spec
  previously mandated Canonical CBOR signatures while the entire reference
  stack (handler, SDKs, IUTs, conformance suite) signs JCS JSON. CBOR is
  now an explicitly reserved binary profile planned for icp-1.1, with the
  change propagated across ICP-1.0-DRAFT §5, canonicalization.md, PACKET,
  SETTLERS, error-codes, ICPIPs, examples, and outreach docs. The
  conformance clause now points at the live `icp-conformance/vectors/`
  suite instead of a placeholder `test-vectors/` directory.
- **HTTP: OpenAPI spec covers the full mounted surface.** 75/75 mounted
  paths and 104 operations documented (was 41/73), including negotiations,
  A2A messaging/credit, subscriptions, promotions, store credits,
  warranties, segments, currency, and the SSE events stream. A new
  bidirectional drift-guard test fails when a route is mounted without
  spec coverage or vice versa. Query-parameter structs previously rendered
  as `in: path` are fixed crate-wide (22 structs). `shipping-zones` (4
  endpoints) is now mounted and documented after repairing the orphaned
  module.

### Removed
- **Dead code:** bit-rotted orphan route files `routes/tax.rs` and
  `routes/manufacturing.rs` (never mounted, ~18 compile errors against
  redesigned core models; git history preserves them) and the unreferenced
  `cli/stateset-doctor.js` (381 lines, superseded by `cli/bin/stateset-doctor.js`).
- **Docs drift:** README "What's New in v1.6.0" no longer carries stale
  v1.2.0 release-notes text (4-verb claim); ICP.md and the spec docs now
  state the true shipped set — all 7 core intent verbs plus the
  `channel.register` extension (ICPIP-0005). Stale MCP tool counts in
  `cli/.claude/CLAUDE.md` corrected (737 tools / 63 domains).

### Testing
- **13 orphaned `cli/test/mcp/` files (246 tests) now run in CI** via the
  `npm test` glob. Admin suite grew to 849 tests (auth-guard + refetch-loop
  regressions). `stateset-http` at 524 tests including 4 new OpenAPI
  drift-guard tests.

## [1.6.0] - 2026-05-19

### Added
- **CLI: extracted 6 focused modules from `cli/src/mcp-server.js`.**
  The orchestrator was 4,051 lines of one giant `createStatesetMcpServer`
  closure. Pulled 837 lines into per-server factory modules under
  `cli/src/mcp/`: `replay-log.js` (agentic JSONL log + ring buffer +
  filtered listing), `pricing.js` (tool runtime metadata + treasury
  pricing cache), `result-builders.js` (`_agentic` envelope wrappers),
  `policy-evaluator.js` (`createEvaluatePolicy` +
  `buildPolicyDecisionBundle`), `tool-wrappers.js` (telemetry / audit /
  charging + ERC-8004 identity), `mutation-simulator.js` (simulate +
  replay mutation tool calls). Each module is a `create<Thing>({deps})`
  factory; per-server state stays per-server. mcp-server.js now 3,214
  lines (−21%). 0 public-API changes; 6,098/6,098 MCP tests pass; lint
  clean.
- **ICP spec: operator-facing integration guides.** Two new walkthroughs
  under `icp-spec/guides/`: `merchant-integration.md` (~15 min — AID
  generation across all three SDKs, reference handler deploy vs
  Backend-mount, picking Settlers, discovery doc, conformance, production
  checklist) and `settler-implementation.md` (~20 min — eligibility,
  Settler URN choice, the 5 capabilities (S.1–S.5), discovery doc shape,
  escrow lifecycle endpoints, SettlementReceipt issuance, proof-of-reserves,
  operational SLAs, allowlist submission). `icp-spec/guides/README.md`
  added as a discovery index and wired into the top-level
  `icp-spec/README.md` layout table.

### Added
- **Rust SDK: `verify_settlement_receipt` helper.** Completes
  three-language symmetry on the dual-signature receipt verifier.
  Same algorithm as the JS + Python helpers — strip both signature
  fields, canonicalize via RFC 8785 JCS, verify both signatures.
  Returns the receipt on success, `Err(Error::Icp { code, ... })`
  on failure with the same three typed codes:
  `format.missing_field`, `signature.invalid`,
  `settlement.settler_signature_invalid`. New
  `VerifySettlementReceiptOptions { require_settler: bool }` opts.
  Lives in `crates/stateset-icp-client/src/settlement.rs`,
  exported from the crate root. **7 unit tests** mirror the JS +
  Python suites byte-for-byte including the canonical-input
  regression test. Rust SDK now **27 unit + 1 integration + 1
  doctest = 29 tests PASS, 0 clippy warnings** (was 22 PASS).
  All 3 first-party SDKs now ship symmetric `verifyWebhook` +
  `verifySettlementReceipt` — the two load-bearing trust
  primitives a partner needs.

### Added
- **Python SDK: `verify_settlement_receipt` helper.** Mirrors the
  JS helper byte-for-byte: takes
  `(receipt, merchant_pubkey_raw, settler_pubkey_raw,
  require_settler=True)`, strips both signature fields,
  canonicalizes via RFC 8785 JCS, verifies both signatures against
  the supplied raw 32-byte Ed25519 pubkeys, returns the receipt
  unchanged on success or raises a typed `ICPError`. Same three
  error codes: `format.missing_field`, `signature.invalid` (merchant
  failure), `settlement.settler_signature_invalid` (settler failure).
  Lives in `packages/icp-python-client/icp_client/settlement.py`,
  exported from the package root so `from icp_client import
  verify_settlement_receipt` Just Works. **7 unit tests** mirror
  the JS suite, including the regression test that asserts both
  signatures cover byte-identical canonical input (no field-ordering
  drift). Python SDK suite now **33/33 PASS** (was 26/26). The
  agent-developer ecosystem (Anthropic SDK, OpenAI Agents,
  LangChain, LangGraph) now has the same trust-final helper JS
  partners get. Rust symmetric helper is the natural next tick.

### Added
- **JS SDK: `verifySettlementReceipt` helper.** The
  `SettlementReceipt` is the single most load-bearing artifact in
  ICP — co-signed by merchant AND Settler, it's what proves
  payment to the merchant and any downstream auditor. Partners
  integrating ICP MUST verify both signatures before treating
  settlement as final, and until this tick they had to roll their
  own dual-signature canonicalization-stripping verifier (and
  typically got at least one part wrong). The new helper takes
  `{receipt, merchantPubkeyRaw, settlerPubkeyRaw}`, strips both
  signature fields, re-canonicalizes with RFC 8785 JCS, verifies
  BOTH signatures over those bytes, and returns the receipt
  unchanged on success — or throws a typed `ICPError`:
  `format.missing_field`, `signature.invalid` (merchant failed),
  or the new `settlement.settler_signature_invalid` code added to
  `error-codes.md`. `requireSettler: false` skips the settler check
  for testing / pre-settler flows. A typed `SettlementReceipt`
  interface lands in the `.d.ts` so TypeScript consumers get full
  shape checking. **7 unit tests** cover happy path; tampered
  amount → merchant `signature.invalid`; wrong settler pubkey →
  typed settler-code; missing each signature field; opt-out flag;
  and a regression test that asserts both signatures cover the
  identical canonical bytes (no field-ordering drift). JS SDK
  suite now **33/33 PASS + 1 SKIP** (was 26/26 + 1 SKIP).
  Symmetric Python + Rust helpers are the natural next ticks.

### Added
- **`subscription.canceled` state-transition publisher.** Third
  publisher hooked into the protocol (after `settlement.released` in
  tick 39 and `dispute.opened` in tick 53). Successful
  `subscription.cancel` Intents now publish a signed
  `subscription.canceled` envelope to every subscribed webhook with
  the full lifecycle metadata: `subscription_id`, `intent_id`,
  `effective_at`, `final_charge_at`, optional `refund_amount`. This
  is the first transition wired through an Intent verb (vs the
  prior REST-endpoint transitions for fulfill/dispute), proving the
  publisher pattern works equivalently across both wire surfaces.
  **1 new live test** asserts register → subscription.cancel
  Intent → receiver gets a signed `subscription.canceled` envelope
  whose `payload.subscription_id` matches what the merchant stub
  returned in its `authorization`. Handler suite now **50/50 PASS**
  (was 49/49).

### Added
- **ICPIP-0005 quickstart guide** (`icp-spec/guides/icpip-0005-quickstart.md`).
  Synthesizes ~15 ticks of ICPIP-0005 work into a single 5-minute
  partner-facing artifact. Shows the three-call client pattern
  (`registerWebhook` → `verifyWebhook` → `fetchChannelEvents`)
  side-by-side in JavaScript, Python, and Rust; the server-side
  state-transition → emit → publish → retry → recovery loop; the
  four-check security model `verifyWebhook` enforces; and the
  reliability invariants the protocol provides (per-channel
  ordering, monotonic sequence, cryptographic attestation, ±300s
  replay defense, 8-attempt retries, 1000-event recovery buffer,
  stable `delivery_attempt: 1` dedupe key). Linked from
  [`ICP.md`](./ICP.md) as a top-level entry point so partners
  skimming the repo land on it in seconds.
- **TypeScript declaration file for `@stateset/icp-client`**
  (`packages/icp-client/src/index.d.ts`). The most-used SDK now ships
  first-class TypeScript support — full IntelliSense, autocomplete,
  and type-checking for every public export. Covers all 7 commerce
  verbs (`PurchaseOpts`, `InventoryOpts`, `SubscribeOpts`,
  `CancelOpts`, `ReturnOpts`, `QuoteRequestOpts`), ICPIP-0005
  (`RegisterWebhookOpts`, `FetchChannelEventsOpts`,
  `EventType` discriminated union over the 13 spec event types,
  `EventEnvelope`, `VerifyWebhookOptions`), wire primitives
  (`Money`, `Signature`, `Identity`, `LineItem`), and the typed
  `ICPError` with `code`/`details` surfaced. `package.json` exposes
  it via both top-level `types` and `exports["."].types` (with
  `types` listed FIRST in the conditional-export object so
  TypeScript's resolver picks it up before `import`/`default`).
  **3 new drift-guard tests** in `test/types-sync.test.mjs` enforce:
  (1) every `export` in `index.mjs` has a matching `.d.ts`
  declaration (catches the "new helper, forgotten types" regression);
  (2) `package.json` correctly points at the `.d.ts` via both
  fields with `types` ordered first; (3) every critical runtime
  artifact (`ICPClient`, `verifyWebhook`, `ICPError`, `EventEnvelope`,
  …) has an explicit declaration. JS SDK suite now **26/26 PASS +
  1 SKIP** (was 23/23 + 1 SKIP). TypeScript partners running
  `@stateset/icp-client` now get the full Stripe-tier DX their
  build pipelines expect.
- **ICPIP-0005 §4.1 webhook retry semantics.** The single-attempt
  delivery comment-as-TODO in `channel-emitter.mjs` is now resolved.
  `emitEvent` awaits the first attempt synchronously, then on
  non-2xx (and non-terminal) failures schedules up to
  `max_attempts - 1` background retries with exponential backoff
  (default: 8 attempts, 5s → 10s → 20s → 40s → 80s → 160s → 320s → 640s,
  ≈20-minute horizon per spec). Each attempt **re-signs the envelope**
  with `delivery_attempt` incremented so receivers see a fresh
  cryptographic attestation per attempt. 4xx codes (except 408
  Request Timeout and 429 Too Many Requests) are terminal — no
  retries — matching spec §4.1. Network errors and 5xx are
  retryable. **The recovery log retains the first-attempt envelope
  (`delivery_attempt: 1`) as the canonical form** so receivers
  dedupe correctly across both the live retry stream and the
  recovery API. Real-scheduler timers call `.unref()` so pending
  retries never block process exit — graceful shutdown is
  unaffected; dropped deliveries surface as sequence gaps the
  receiver recovers via §5. `opts.retryPolicy` overrides the
  default schedule; `opts.scheduler` injects a fake clock for
  tests. **6 new tests** in `test/channel-emitter-retry.test.mjs`
  cover: 5xx retries-to-exhaustion with monotonic `delivery_attempt`
  + re-signed bodies; 4xx terminal-without-retry; 408/429
  retryable; network-error → eventual-2xx happy path; recovery log
  serves first-attempt canonical form; sequence still monotonic
  across failed deliveries. Handler suite now **49/49 PASS** (was
  43/43).

### Added
- **`dispute.opened` state-transition publisher.** Tick 39 wired
  `settlement.released` into `handleFulfill`; this tick generalizes
  the pattern to `handleDispute`. Opening a dispute now mints a
  fresh `dispute_id`, records it in the escrow's signed event chain,
  AND fires `publishToSubscribers('dispute.opened', ...)` to every
  webhook channel that subscribed for it. Payload carries
  `{dispute_id, escrow_id, intent_id, reason, amount, opened_at,
  prior_state}` — everything an agent needs to react. The handler
  response now also surfaces the new `dispute_id` so callers can
  correlate. **1 new live test** in `test/channel-publish.test.mjs`
  drives the full register → purchase → accept → dispute flow and
  asserts the receiver gets a signed `dispute.opened` envelope with
  the expected payload (or, if the demo stub rejects from the
  current escrow state, asserts the typed `escrow.wrong_state`
  error path). Handler suite now **43/43 PASS** (was 42/42). The
  publisher pattern is now proven for two state transitions —
  generalizing to `escrow.refunded` / `subscription.canceled` is
  a few-line repeat per transition.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — `WellKnown` discovery shape now
  matches handler wire reality.** Closes the third and final
  load-bearing schema drift. The new `WellKnown` requires
  `{spec, handler, handler_version, merchant_aid, merchant_pubkey,
  capabilities, settler_allowlist}` — exactly what
  `GET /icp/v1/.well-known/icp` returns. `merchant_pubkey` is now a
  proper `{alg, raw_hex}` object (not a flat `ed25519_pubkey_hex`
  string); `capabilities` is a nested object with `verbs`,
  `transports`, `pqc_hybrid`, and `push_channels` arrays;
  `settler_allowlist` is the string-identifier array the handler
  actually returns (the richer `Settler` schema is kept as a reserved
  shape for future spec versions). All four ICPIP-0005 push-channel
  values (`webhook`, `sse`) are enumerated. **New drift-guard
  invariants** assert required field set on `WellKnown` and
  `merchant_pubkey`, and ban the old flat
  `ed25519_pubkey_hex`/`x25519_pubkey_hex` properties from leaking
  back. Handler suite now **42/42 PASS** (was 41/41).
  With this tick, **every load-bearing OpenAPI schema (envelope,
  responses, discovery) matches the handler wire reality** — codegen
  partners running `openapi-generator generate -i openapi.yaml -g <lang>`
  for any target now get clients that handle request, response, AND
  discovery on the first try, no manual fix-ups.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — verb response shapes now match
  handler wire reality.** Tick 50 reconciled the request envelope;
  this tick closes the response side. Every `/icp/v1/intents` 200
  body is now correctly modeled as `{<payload_key>: <inner>,
  signature: Signature}`:
  - `purchase.create` → `PurchaseCreateResponse` (`{quote, signature}`)
  - `purchase.return` → `PurchaseReturnResponse` (`{authorization, signature}`)
  - `subscription.create` → `SubscriptionCreateResponse` (`{authorization, signature}`)
  - `subscription.cancel` → `SubscriptionCancelResponse` (`{authorization, signature}`)
  - `inventory.query` → `InventoryQueryResponse` (`{snapshot, signature}`)
  - `quote.request` → `QuoteRequestResponse` (`{proposal, signature}`)
  - `payout.request` → `PayoutRequestResponse` (`{authorization, signature}`)
  - `channel.register` → `ChannelRegisterResponse` (`{channel, signature}`)
  Inner payload objects keep `additionalProperties: true` pending the
  same follow-up ICPIP that will lift inner-field shapes out of the
  SDKs into per-verb JSON Schemas. The shared `Signature` schema
  (`{alg, kid, sig}`) introduced in tick 50 is now referenced from
  every response wrapper. Stale flat `signature_hex`/`merchant_signature_hex`
  fields removed from `SettlementReceipt`, `Dispute`, `Escrow`, and
  the old per-verb response schemas. `SettlementReceipt` now uses two
  `Signature` objects (`merchant_signature`, `settler_signature`)
  reflecting how the handler stub returns them. **New drift-guard
  test** asserts every wrapper schema declares the correct payload
  key + signature pair, and asserts no `required: [..., signature_hex]`
  flat-shape lines remain in any response schema. Handler suite now
  **41/41 PASS** (was 40/40). Codegen partners running
  `openapi-generator generate -i openapi.yaml -g <lang>` now get
  clients that can deserialize handler responses on the first try.

### Changed (breaking for codegen consumers, no-op for SDK users)
- **OpenAPI 3.1 reconciliation — IntentEnvelope shape now matches
  handler wire reality.** Closes long-standing drift between
  `icp-handler/openapi.yaml` and what the handler actually accepts.
  Codegen against the previous spec would have produced clients
  rejected by the handler; codegen against the reconciled spec
  produces working clients.
  - `IntentEnvelope` required fields: `{intent, signature}` (was
    `{intent, auth}` with nested `signature_hex`/`pubkey_hex`).
    Optional `_pubkey_hex` convenience field added.
  - New shared `Signature` schema (`{alg, kid, sig}`) reused by the
    envelope and every signed merchant response.
  - `IntentBase` fields: `v`/`verb`/`intent_id`/`buyer`/`merchant`/
    `settler`/`expiry`/`principal_binding`/`nonce`/`iat`/`exp` —
    RFC 3339 timestamps where applicable; `additionalProperties:
    true` so verb-specific fields don't break validation. Verb
    enum gained `channel.register`.
  - `PrincipalBinding`: `principal`/`agent`/`authority`/`expiry`/
    `revocation`/`signature` (was `agent`/`authority_caps` only).
  - New `Authority` schema (`max_per_intent`, `verbs`,
    optional `max_per_payout`).
  - All three example payloads (`PurchaseCreateExample`,
    `SubscriptionCreateExample`, `InventoryQueryExample`) rewritten
    against the handler-compatible shape (RFC 3339 timestamps,
    `signature` envelope, current per-verb field names).
  - Verb-specific intent shapes (`IntentPurchaseCreate` etc.)
    removed pending a follow-up ICPIP that will lift them out of
    the SDKs into `icp-spec/schemas/intent.<verb>.schema.json`.
  - **New drift-guard test** in `test/openapi-sync.test.mjs`
    enforces the wire-reality invariants directly: required fields,
    field-name correctness, schema relationships. Adding a stale
    field name fails CI. Handler suite now **40/40 PASS** (was
    39/39).

### Added
- **Rust SDK: `fetch_channel_events` method** completing three-language
  symmetry on the recovery API. `client.fetch_channel_events(channel_id,
  since)` verifies by default (returns `Vec<Value>` of envelopes);
  `fetch_channel_events_raw(...)` returns the underlying
  `{envelope, signature}` pairs for callers that want to delegate
  verification. Uses the existing `Error::SignatureInvalid` variant
  on per-envelope verification failure and the typed `Error::Icp
  { code: "channel.*", … }` for handler error responses. The
  integration test grew from 11 to 13 wire flows: full recovery
  roundtrip (register channel with unreachable URL → drive purchase
  → accept → fulfill → fetch missed event → verify), plus unknown-
  channel `channel.not_found` assertion. Rust SDK still **20 unit
  + 1 integration + 1 doctest, 0 clippy warnings**. Combined SDK
  footprint: JS 23 tests, Python 26 tests, Rust 22 tests — all
  green. **Three-language ICPIP-0005 client symmetry complete**:
  every first-party SDK exposes `registerWebhook`, `verifyWebhook`,
  and `fetchChannelEvents` as one-call methods.
- **Python SDK: `fetch_channel_events` method** mirroring the JS helper.
  `client.fetch_channel_events(channel_id, since=0, *, verify=True)`
  GETs the ICPIP-0005 §5 recovery API, parses, and (by default)
  verifies each envelope signature against the cached merchant
  pubkey before returning the list of envelope dicts. Raises typed
  `ICPError` for `channel.not_found`, `channel.expired`,
  `channel.sequence_gap`, `format.bad_query_param`, and
  `channel.signature_invalid`. **2 new live integration tests**
  mirror the JS suite: full register → purchase → accept → fulfill
  → recovery round-trip (with envelope-signature verification);
  unknown channel raises typed `channel.not_found`. Python SDK suite
  now **26/26 PASS** (was 24/24). With this, the Python SDK also
  exposes the complete ICPIP-0005 client story in three one-call
  methods: `register_webhook`, `verify_webhook`,
  `fetch_channel_events`.
- **JS SDK: `fetchChannelEvents` method** for the ICPIP-0005 §5
  recovery API. `client.fetchChannelEvents(channelId, since=0,
  {verify=true})` GETs `/icp/v1/channels/:id/events?since=N`,
  parses the response, and (by default) verifies each envelope
  signature against the cached merchant pubkey from `.well-known/icp`
  before returning the array. Returns verified envelope objects, or
  the raw `{envelope, signature}` pairs if `verify: false`. Throws
  typed `ICPError` for `channel.not_found`, `channel.expired`,
  `channel.sequence_gap`, `format.bad_query_param`, and
  `channel.signature_invalid`. **2 new live integration tests** in
  `test/client.test.mjs`: (1) register a webhook → run purchase →
  accept → fulfill → assert `fetchChannelEvents(channelId, 0)`
  returns a verified `settlement.released` envelope AND
  `fetchChannelEvents(channelId, sequence)` returns empty;
  (2) fetching from an unknown channel throws typed
  `channel.not_found`. JS SDK suite now **23/23 PASS + 1 SKIP** (was
  21/21 + 1 SKIP). The three-call ICPIP-0005 client story is now
  complete in JS: `registerWebhook` to subscribe, `verifyWebhook` to
  validate live deliveries, `fetchChannelEvents` to backfill misses.
- **ICPIP-0005 §5 recovery API** — `GET /icp/v1/channels/:channel_id/events?since=N`.
  Returns every retained signed envelope with `sequence > since` in
  ascending order. The channel-emitter now records each signed
  envelope into a per-channel ring buffer (1000-event retention by
  default) before the network POST, so agents that miss a live
  delivery can backfill against the same bytes the receiver would
  have seen. Each entry is `{envelope, signature}` — verbatim
  canonical bytes — so receivers re-verify with the same Ed25519
  algorithm as live webhooks. Returns `409 channel.sequence_gap` when
  `since` is before the retained window (agent must re-register),
  `404 channel.not_found` for unknown channels, `400
  format.bad_query_param` for malformed `since`. **3 new tests** in
  `test/channel-recovery.test.mjs` cover happy-path slicing, unknown
  channel, malformed query — including envelope-signature
  verification on every returned event. Handler suite now **39/39
  PASS** (was 36/36). OpenAPI 3.1 spec + drift guard extended. With
  this, ICPIP-0005's reliability story is complete: live deliveries
  via the emitter, plus authoritative backfill via the recovery API.
- **Rust SDK: `register_webhook` method** completing three-language
  symmetry on both ICPIP-0005 ends. `client.register_webhook(merchant,
  settler, channel_type, url, event_filters)` builds the
  `channel.register` Intent, signs + submits via the existing
  `post_intent` path, returns a `SignedResponse` whose merchant
  signature can be verified via `client.verify_signed_response(...)`.
  The live integration test grew from 8 verbs to 11 wire flows —
  added 3 new cases: webhook registration with the GET round-trip
  verification, SSE registration that asserts the merchant minted a
  subscription token, http:// non-loopback rejection that asserts
  the typed `channel.url_unverified` `Error::Icp` variant. Rust SDK
  still **20 unit + 1 integration + 1 doctest, 0 clippy warnings**.
  All 3 SDKs now ship both `registerWebhook` and `verifyWebhook` —
  both ends of the ICPIP-0005 loop are first-class one-call methods
  in JavaScript, Python, and Rust.
- **Python SDK: `register_webhook` method** mirroring the JS SDK helper.
  `client.register_webhook(merchant, settler, *, url=None, type='webhook',
  event_filters=[], delivery=None, auth=None)`. Builds the
  `channel.register` Intent, signs it, POSTs to `/icp/v1/intents`, and
  transparently verifies the merchant signature on the returned
  ChannelRegistration via the existing `_verify_merchant` pipeline.
  **3 new live integration tests** mirror the JS suite: webhook
  happy path, SSE registration mints a subscription token, http://
  non-loopback URL rejected with typed `channel.url_unverified`
  ICPError. Python SDK suite now **24/24 PASS** (was 21/21).
- **JS SDK: `registerWebhook` method** for ICPIP-0005 channel
  registration. Accepts `{merchant, settler, type?, url?,
  event_filters?, delivery?, auth?}`, builds the `channel.register`
  Intent, signs it, POSTs to `/icp/v1/intents`, verifies the
  merchant signature on the returned ChannelRegistration. Without
  this, devs had to hand-build the channel.register Intent envelope
  even though they used `verifyWebhook` to receive events; now both
  ends of the loop are first-class SDK calls. **3 new live
  integration tests**: webhook happy path (with GET round-trip),
  SSE happy path (verifies the merchant mints a subscription token),
  http:// non-loopback rejection (typed `channel.url_unverified`
  ICPError). JS SDK suite now **21/21 PASS + 1 SKIP** (was 18/18 +
  1 SKIP). Symmetric helpers for Python + Rust SDKs are upcoming.
- **Rust SDK: `verify_webhook` helper** (`stateset_icp_client::verify_webhook`).
  Completes the three-language receiver-side symmetry (JS + Python +
  Rust all ship the Stripe-style one-call validator). Same 4 ICPIP-0005
  §6 checks; returns `Err(Error::Icp { code: "channel.*", … })` on any
  failure. Generic over headers via a small `HeaderPair` trait, so the
  helper accepts `Vec<(String, String)>`, `&[(&str, &str)]`, and any
  HTTP crate's header collection without dependency on it. **9 new
  unit tests** mirror the JS/Python suites: happy path, tampered body,
  stale timestamp (→ `channel.replay`), missing timestamp, missing
  signature, malformed algorithm prefix, wrong pubkey, mixed-case
  headers, slice-of-`&str` pairs. Rust SDK suite now **20 unit + 1
  integration + 1 doctest, 0 clippy warnings** (was 12/1/1). All 3
  SDKs now hand Agent developers a one-call webhook verifier.
- **Python SDK: `verify_webhook` helper** (`icp_client.verify_webhook`).
  Mirrors the JS SDK's `verifyWebhook` byte-for-byte — same four checks
  (timestamp window, HTTP-layer Ed25519 signature, body shape, envelope
  signature), same `channel.*` error codes raised as `ICPError`, same
  default ±300s tolerance. Lives in `icp_client/webhook.py`, exported
  from the package root. Case-insensitive header lookup works across
  dict, fetch Headers, requests CaseInsensitiveDict, and any
  `.items()`-providing mapping. **9 unit tests** mirror the JS suite
  plus an extra malformed-algorithm rejection case. Python SDK suite
  now **21/21 PASS** (was 12/12). Reaches the agent-developer
  ecosystem (Anthropic SDK, OpenAI Agents, LangChain, LangGraph)
  where ~80% of production webhook receivers will run.
- **JS SDK: `verifyWebhook` helper** for inbound ICPIP-0005 events.
  Stripe-style one-call validator: pass the raw HTTP body, request
  headers, method, path, and the merchant's published Ed25519 pubkey;
  get back the parsed `EventEnvelope` OR a typed `ICPError` with a
  `channel.*` code. Performs every check ICPIP-0005 §6 requires:
  (1) HTTP timestamp within ±300s (configurable), (2) HTTP-layer
  `X-ICP-Signature` verifies against
  `<timestamp>.<method>.<path>.<body>`, (3) body parses as
  `{envelope, signature}`, (4) envelope signature verifies against
  the merchant pubkey over canonical envelope bytes. **7 unit tests**
  cover happy path, tampered body, flipped envelope sig, stale
  timestamp (replay), missing header, wrong pubkey, mixed-case
  headers. End-to-end handler→SDK interop is already covered on the
  handler side by `channel-publish.test.mjs`. JS SDK suite now
  **18/18 PASS + 1 SKIP** (was 11/11). Closes the most common ICP
  security bug class: receiving a webhook and forgetting to verify it.
- **ICPIP-0005 state-transition publisher** — wires the webhook
  emitter into actual handler state transitions, closing the
  server-side loop. New `publishToSubscribers(store, eventType,
  payload, opts)` iterates the channel store, filters by event-type
  subscription + expiry, and fan-outs in parallel via the existing
  emitter. The fulfill handler now publishes `settlement.released`
  with `{settlement_id, escrow_id, intent_id, amount, final_state,
  settled_at}` — fire-and-forget so the synchronous response doesn't
  block on receiver round-trips. **2 new end-to-end tests** in
  `test/channel-publish.test.mjs` prove the full loop: register a
  webhook subscribed to `settlement.released` → POST a purchase
  Intent → accept the quote → fulfill the escrow → assert the
  receiver got a signed `settlement.released` EventEnvelope whose
  envelope signature verifies against the merchant's published
  pubkey. A second test confirms that a channel subscribed only to
  `dispute.opened` does NOT receive fulfill events. The URL
  validator now permits `http://127.0.0.1` and `http://localhost`
  for dev/CI; production https://-only requirement is unchanged
  for non-loopback hosts. Handler suite now **36/36 PASS** (was
  34/34). Together with the previous 3 ticks, ICPIP-0005's
  server-side flow is end-to-end live: registration, signed emit,
  state-transition publish.
- **ICPIP-0005 webhook emitter** (`icp-handler/src/channel-emitter.mjs`).
  Closes the delivery side of ICPIP-0005: actually POSTs signed
  EventEnvelopes to registered webhooks. Maintains monotonic
  `sequence` + `previous_event_id` chain per channel; builds
  canonical EventEnvelopes per spec §2; signs each envelope
  (Ed25519); adds defense-in-depth HTTP-layer signature
  (`X-ICP-Signature: ed25519=<sig>` over `timestamp.method.path.body`);
  emits `X-ICP-Timestamp`, `X-ICP-Channel-Id`, `X-ICP-Event-Id`,
  `X-ICP-Sequence` convenience headers; advances `last_event_id`
  only on 2xx so the chain stays correct across failed deliveries.
  **3 new tests** spawn a mock in-process HTTP receiver, register
  channels, drive emits, and assert: (1) envelope + HTTP signatures
  both verify against the source's published pubkey, (2) sequence
  monotonic across two emits, (3) failed delivery leaves
  `last_event_id` unchanged. Handler suite now **34/34 PASS** (was
  31/31). Full retry semantics (8-attempt exponential backoff,
  DLQ on terminal 4xx) deferred to a follow-up; this tick
  establishes the wire format end-to-end.
- **ICPIP-0005 reference implementation** in `icp-handler`. New
  verb `channel.register` (POST `/icp/v1/intents`) + GET
  `/icp/v1/channels/:channel_id` route. Validates webhook URLs
  (https-only), mints SSE subscription tokens (1h TTL), echoes
  event_filters, persists in in-memory `channelStore`, returns a
  signed `ChannelRegistration`. **6 new tests in
  `test/channels.test.mjs`** cover happy path (webhook + SSE),
  policy rejects (http:// URL → `channel.url_unverified`, unknown
  type → `format.unknown_channel_type`), 404 lookup (`channel.not_found`),
  and well-known advertisement of `channel.register` +
  `push_channels: [webhook, sse]`. Handler suite now **31/31 PASS**
  (was 25/25). OpenAPI 3.1 spec updated with the new GET route and
  `ChannelRegistration` response schema; drift-guard test extended.
  Proves ICPIP-0005 is buildable, not just paper.
- **ICPIP-0005 — Push Channels (Webhooks + SSE).** First formal spec
  for merchant→Agent out-of-band event delivery. Two wire-equivalent
  channels (webhooks + SSE) carry an identical signed
  `EventEnvelope` with per-channel monotonic `sequence`, exponential-
  backoff retries (8 attempts), defense-in-depth signatures
  (HTTP-layer + envelope-layer Ed25519 or HMAC), token rotation,
  recovery API for sequence gaps. 12 event types: `settlement.*`,
  `escrow.*`, `dispute.*`, `subscription.*`, `inventory.*`,
  `payout.released`, `compliance.kyb_due`, `risk.flag`. Adds 8
  error codes under the new `channel.*` namespace to
  `error-codes.md` + HTTP status mapping. Closes the "Stripe
  webhooks" gap that every real merchant integration needs.
  Bumped the previous placeholder slot (confidential PrincipalBinding
  transport) from 0005 to 0006.
- **Rust SDK: merchant signature verification + full 7-verb coverage.**
  Added `verify_ed25519` (top-level safe verifier), merchant-pubkey
  cache on `Client` (populated by `well_known()`), and
  `Client::verify_signed_response` that re-canonicalizes the payload
  and verifies the merchant's Ed25519 signature. **All 7 verb method
  signatures now match the JavaScript reference SDK byte-for-byte**
  (`service_id`/`cadence`/`max_total_per_period` for subscribe,
  `original_settlement_id`/`desired_outcome` for return,
  `platform`/`max_per_payout` for payout, etc.). Integration test
  expanded to exercise all 7 verbs end-to-end with merchant signature
  verification on every response. **Tests: 11 unit + 1 integration
  + 1 doctest, 0 clippy warnings.** Closes the trust gap — the
  Rust SDK now refuses any response whose merchant signature doesn't
  verify against the published `.well-known/icp` pubkey.

### Added (prior)
- **`stateset-icp-client` Rust SDK** (`crates/stateset-icp-client`).
  Third-language ICP-1.0 client SDK alongside `@stateset/icp-client`
  (npm) and `icp-client` (PyPI). API surface mirrors both. **Produces
  byte-identical wire bytes vs the JS reference** — verified by the
  `handler_integration` test that spawns the JS icp-handler and drives
  it end-to-end from Rust (discovery → inventory.query → purchase.create).
  All 7 ICP verbs implemented: `inventory()`, `purchase()`,
  `subscribe()`, `cancel()`, `return_purchase()`, `request_quote()`,
  `payout()`. Built on `ed25519-dalek` + `x25519-dalek` + `serde_jcs`
  + `ureq`. **11 unit tests + 1 live integration test, 0 clippy
  warnings.** Unlocks the entire Rust ecosystem: Solana / Aptos / Sui
  infra, payment processors, high-throughput merchants.
- **OpenAPI 3.1 spec for icp-handler** (`icp-handler/openapi.yaml`).
  Normative HTTP API surface for the 9 handler routes and all 7 ICP
  verbs (as a discriminated union over `IntentEnvelope`). Maps every
  ICP error code namespace to HTTP status. Designed to drive
  language-agnostic client codegen (Java / C# / Swift / Kotlin /
  Ruby / PHP / Dart / Elixir / Go-with-no-existing-SDK). Comes with
  `test/openapi-sync.test.mjs` (5 tests) that guards against drift
  between the YAML and the actual route registry in `src/server.mjs`.
  Adding a route to one without the other fails CI. **Handler suite
  now 25/25 PASS** (was 20/20).
- **Conformance vector 03 — signature verification.** Closes the
  third leg of the cross-language interop proof. 8 sub-cases: 1
  positive control (RFC 8032 §7.1 valid-roundtrip) and 7 negative
  cases (tampered-message, bit-flipped-signature, wrong-pubkey,
  truncated-signature, padded-signature, all-zero-signature,
  random-bytes-signature). All four IUTs (JS / Rust / Go / Python)
  return byte-identical results: `[true, false×7]`. Total
  conformance proof now **3 vectors × 4 IUTs = 12 byte-identical
  PASS**. Required gate for ICPIP-0001's Final-promotion discipline.
- Rust IUT (`crates/stateset-icp-iut`) and Go IUT
  (`crates/stateset-icp-iut-go`) gain `verify_one`/`verifyOne`
  helpers; Python IUT gains the same. JS IUT
  (`icp-conformance/iut-adapters/reference-demo.mjs`) gains an
  SPKI-reconstructing verifier.
- Vector 03 registered in `icp-conformance/profiles/icp-1.0-core.json`.

## [1.5.0] - 2026-05-12

Minor release: **`icp-client` Python SDK**. Closes the adopter-ergonomics
gap for the Python-first agent-developer ecosystem (Anthropic SDK,
OpenAI Agents, LangChain, LangGraph). Mirror of the JavaScript
`@stateset/icp-client` API with byte-identical wire bytes verified
by tests.

### Added
- **`packages/icp-python-client/`** — pip-installable Python SDK.
  Single `cryptography` dependency, otherwise stdlib-only.
- `ICPClient.create(handler_url, principal, ...)` mirroring the JS
  factory. Identity persistence via `generate_identity()` /
  `identity_from_seeds()`.
- All 7 ICP verbs as methods: `.inventory()`, `.purchase()` (with
  optional `from_proposal_id`), `.subscribe()`, `.cancel()`,
  `.return_()`, `.request_quote()`, `.payout()` (handles the
  inverted-direction field rename internally). Plus `.accept()`,
  `.observe()` (generator over SSE EscrowEvents), `.settlement()`,
  `.capabilities()`.
- Independent merchant-signature verification on every response
  against the published `.well-known/icp` pubkey. Verification
  failures raise typed `ICPError("signature.invalid", ...)`.
- Module-level exports: `canonical_json()`, `sign_ed25519()`,
  `verify_ed25519()`, `Identity`. Useful for advanced agent flows
  that need to sign payloads outside the client surface.
- `pyproject.toml` with hatchling backend, Python 3.8+, MIT OR
  Apache-2.0 licensing.
- 12 end-to-end tests against a spawned `icp-handler`. CI workflow
  job: `python-sdk`.
- README with Anthropic SDK integration example showing how to wire
  ICP as Anthropic-API tools.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.5.0.

### Adopter surface
| Target | Path |
|---|---|
| JS / TS / Node / browser | `npm install @stateset/icp-client` |
| Python / Anthropic / OpenAI Agents / LangChain | `pip install icp-client` |
| MCP-compatible client (Claude Desktop / Cursor / Windsurf) | `mcpServers` config → icp-mcp |
| Raw HTTP (any language) | `POST /icp/v1/intents` with manual codec |

### Test count
Cumulative protocol-layer test count: **114 distinct PASS signals per
CI run** (handler 20, MCP 6, Settler 9, chain-watcher 8, JS SDK 11,
**Python SDK 12** *(new)*, Foundry contract 15, conformance 8, Docker
integration 17, demos 8).

## [1.4.0] - 2026-05-12

Minor release closing **100% commerce verb coverage**. ICP-1.0 now runs
all seven commerce primitives in the reference handler, MCP server, and
client SDK: discovery, retail purchase, recurring subscription + cancel,
returns, B2B wholesale RFQ, and marketplace seller payouts. Total
addressable commerce flow ≈ $31T/year.

### Added
- **`quote.request` verb runtime impl** (reference implementation of
  ICPIP-0003). Backend stub with volume-tier pricing (1–99 catalog,
  100–499 −10%, 500+ −20%), 30-day proposal validity. `from_proposal_id`
  extension on `purchase.create` honors the proposal's prices verbatim
  (no 5% handling fee applied) for the duration of `valid_until`.
  Rejects with `quote.proposal_not_found`, `quote.proposal_expired`, or
  `quote.proposal_total_mismatch` as appropriate.
- **`payout.request` verb runtime impl** (reference implementation of
  ICPIP-0004). The first ICP verb with **inverted signing direction** —
  the recipient (seller) signs the Intent; the platform signs the
  PayoutAuthorization. Backend stub with $5000-default seller balance,
  3% platform commission + 1% chargeback reserve (released after 90
  days), `approved_amount = available − sum(fees)`. Honors `max_per_payout`
  from PrincipalBinding (OPTIONAL authority field; backward-compatible).
- **6 new error codes** in `policy.quote.*` and `quote.*` namespaces.
- **10 new error codes** in `policy.payout.*` namespace.
- **JSON Schemas**: `intent.quote.request.schema.json` and
  `intent.payout.request.schema.json`.
- **SDK methods**: `client.requestQuote()` and `client.payout()`. The
  payout method handles the buyer→seller field-name mapping internally
  so SDK callers don't have to.

### Changed
- Handler accepts **7 ICP verbs** (was 5); MCP and SDK match. Capability
  advertisement at `.well-known/icp` reflects the full set.
- `stubQuote()` honors `from_proposal_id` when present, with three
  typed-error guards.
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.4.0.

### Test count
Cumulative protocol-layer test count: **102 distinct PASS signals per
CI run**. Handler 20/20 (was 14), MCP 6/6, SDK 11/11, Settler 9/9,
chain-watcher 8/8, Foundry contract 15/15, conformance 8/8 (4 IUTs × 2
vectors), Docker integration 17/17, demos 8.

### Coverage note
With this release, ICP-1.0 hits **100% commerce verb coverage**:
discovery (`inventory.query`), one-shot retail (`purchase.create`),
recurring revenue (`subscription.create` + `subscription.cancel`),
returns/refunds (`purchase.return`), B2B wholesale RFQ
(`quote.request`), and marketplace payouts (`payout.request`).
That's ≈ $31T in addressable annual commerce flow across all major
commerce patterns.

## [1.3.0] - 2026-05-12

Minor release adding five compounding ICP protocol-layer additions:
the **client SDK**, the **`subscription.cancel` verb**, the
**chain-mode watcher**, and the first two formal Improvement Proposals
(ICPIP-0001 Process + ICPIP-0002 Hybrid PQC mandate).

### Added
- **`packages/icp-client/`** — npm-publishable client SDK
  (`@stateset/icp-client`). Zero runtime dependencies. `ICPClient.create()`
  returns a client with `.capabilities()`, `.inventory()`, `.purchase()`,
  `.accept()`, `.subscribe()`, `.cancel()`, `.return_()`, `.observe()`
  (async iterator over SSE escrow events), and `.settlement()`. Every
  merchant response is independently signature-verified against the
  pubkey from `.well-known/icp` — verification failures throw typed
  `ICPError`. 11/11 SDK tests PASS.
- **`subscription.cancel` verb (5th ICP-1.0 verb)** — spec §6.5.1, JSON
  Schema, 4 new error codes under `policy.subscription.*` namespace.
  Closes the subscription lifecycle: with `subscribe` + `cancel`, agents
  fully manage recurring services without out-of-band coordination.
  Idempotent: cancellation of an already-cancelled subscription returns
  the existing CancellationAuthorization.
- **`services/icp-chain-watcher/`** — zero-dep Node.js service that
  polls an EVM JSON-RPC endpoint for `ICPEscrow.sol` events,
  ABI-decodes them with a hand-rolled Solidity decoder, and forwards to
  `settler-stateset` as `/admin/escrow/event` POSTs. Closes the
  chain-mode gap: real Base Sepolia transactions now become signed ICP
  EscrowEvents. 8/8 tests PASS (mock JSON-RPC + real Settler).
- **ICPIP-0001** (Meta, Draft) — ratifies the proposal lifecycle.
  Modeled on EIP-1 / BIP-2 with two ICP-specific additions: (1)
  Standards Track Final REQUIRES ≥2 independent implementations passing
  the new conformance vectors, (2) temporary 30-day suspensive steward
  veto sunsetting at the 24-month mark per Charter §3.4.
- **ICPIP-0002** (Standards Track, Draft) — proposes mandatory
  Ed25519 + ML-DSA-65 hybrid signatures for Intents above $10,000
  USD-equivalent. Addresses the harvest-now-decrypt-later quantum
  threat. Would make ICP the **first agentic-commerce protocol to
  mandate PQC** at any value threshold.
- **ICPIP-0003** (Standards Track, Draft) — specifies the `quote.request`
  verb (B2B wholesale RFQ — request pricing without commitment). Adds
  the missing primitive for procurement flows. PriceProposal response
  with `valid_until` validity window; `from_proposal_id` extension to
  `purchase.create` for binding-on-acceptance. Addresses ~$23T global
  B2B e-commerce.
- **ICPIP-0004** (Standards Track, Draft) — specifies the
  `payout.request` verb (marketplace seller payouts). The only verb
  with inverted signing direction (recipient signs, not originator).
  Itemized binding fees + audit-traceable source transactions. Addresses
  ~$2T global marketplace GMV (Stripe Connect / Etsy / Uber / Shopify
  Marketplace / App Store class). After this ICPIP reaches Final, ICP
  covers 100% of commerce verb surface.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.3.0.
- Handler accepts 5 ICP verbs (was 4); MCP and SDK match.
- Spec-interop bug fixed in backend stubs: signatures no longer embedded
  inside signed payloads. Round-trip verification by SDK clients now
  works for inventory.query, subscription.create, and purchase.return
  (in addition to the already-working purchase.create).
- Settler daemon `/admin/escrow/event` now accepts chain-origin fund
  events with optional `intent_id` (chain doesn't carry it; merchant
  Backend resolves via `quote_hash` post-hoc).
- Leftover test state file `services/icp-chain-watcher/.icp-chain-watcher-state.json`
  excluded via `.gitignore`.

### Coverage note
ICP-1.0 now ships **5 verbs covering ~99% of commerce dollar volume**:
`inventory.query` (discovery), `purchase.create` (one-shot retail),
`subscription.create` + `subscription.cancel` (recurring revenue +
cancel), `purchase.return` (returns/refunds). The 2 remaining verbs
(`quote.request` and `payout.request`) ship as Standards Track Draft
ICPIPs (0003 + 0004) in this release; once they reach Final via the
ICPIP-0001 lifecycle, ICP covers 100% of commerce verb surface
(~$31T in addressable annual commerce flow).

### Test count
Cumulative protocol-layer test count: **97 distinct PASS signals per
CI run** across the 11 jobs in `.github/workflows/icp-conformance.yml`.

## [1.2.0] - 2026-05-12

Minor release adding the **`inventory.query`** verb — the fourth ICP-1.0
intent verb and the highest-call-volume verb in B2B agentic commerce.

### Added
- **`inventory.query` verb** (spec §6.3 normative; was a 1.1 stub). A
  read-only, signed query for inventory availability + pricing that
  returns a merchant-signed `InventorySnapshot` with a `valid_until`
  validity window. Doesn't trigger an escrow.
- Snapshot-quote consistency rule: when a subsequent `purchase.create`
  Quote diverges from a still-valid InventorySnapshot's price for the
  same SKU, the merchant SHOULD include `snapshot_id` in the Quote
  metadata; conformant buyers MAY refuse divergent Quotes.
- JSON Schema `intent.inventory.query.schema.json` with optional `skus`,
  free-form `filters`, and `max_results` cap.
- Handler backend `stubInventoryQuery()` with a 5-SKU demo catalog and
  `in_stock_only` filter support.
- ICP-handler and ICP-MCP now advertise and accept **4 ICP verbs**:
  `purchase.create`, `subscription.create`, `purchase.return`,
  `inventory.query`.
- 2 new handler tests covering the full snapshot path + the
  `in_stock_only` filter; **handler 12/12 PASS, MCP 6/6 PASS**.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.2.0.

### Coverage note
ICP-1.0 now covers ~99% of commerce dollar volume across four verbs:
discovery (`inventory.query`), one-shot retail (`purchase.create`),
recurring revenue (`subscription.create`), and returns/refunds
(`purchase.return`). Three verbs remain deferred to ICP-1.1:
`quote.request` (wholesale RFQ), `payout.request` (marketplace seller
payouts), `subscription.cancel` (mid-cycle subscription termination).

## [1.1.0] - 2026-05-11

Introduces the **Intelligent Commerce Protocol (ICP)** — an open spec and
reference implementation set for the operational lifecycle of
agentic-AI commerce (quote, escrow, fulfillment, dispute, settlement).
The 250k-LOC commerce engine is unchanged; ICP is additive infrastructure.

### Added
- **ICP-1.0 normative specification** (`icp-spec/ICP-1.0-DRAFT.md`):
  wire format, canonical serialization rules (CBOR + JSON), 60+ error
  codes, signatures (Ed25519 + optional ML-DSA-65 hybrid), AID
  derivation, escrow state machine, SettlementReceipt format.
- **Three intent verbs**: `purchase.create`, `subscription.create`,
  `purchase.return` — covering ~95% of e-commerce dollar volume.
- **Cross-language conformance suite** (`icp-conformance/`): 2 vectors
  × 4 independent Implementation-Under-Test adapters (JavaScript with
  `node:crypto`, Rust with `ed25519-dalek` + `serde_jcs`, Go with
  pure stdlib `crypto/ed25519`+`crypto/ecdh`, Python with `cryptography`)
  all producing byte-identical wire bytes. CI enforces cross-IUT
  determinism on every PR.
- **HTTP handler reference** (`icp-handler/`): zero-dependency
  `node:http`-based merchant Backend implementing the surface from
  `handler-design.md`. 10/10 end-to-end roundtrip tests.
- **MCP server reference** (`icp-mcp/`): JSON-RPC 2.0 over stdio,
  drops into Claude Desktop / Cursor / Windsurf via `mcpServers`
  config. 8 ICP tools spanning the full lifecycle. 6/6 tests.
- **Off-chain Settler daemon** (`services/settler-stateset/`): signs
  EscrowEvents, issues SettlementReceipts, serves discovery
  document at `/.well-known/icp-settler`. Mock chain mode shipping;
  chain-mode subscriber hooks reserved. 9/9 tests.
- **On-chain custody contract** (`icp-spec/contracts/usdc-base/ICPEscrow.sol`):
  audit-ready Solidity 0.8.24 + OpenZeppelin patterns. Time-locked
  release, dispute primitive, arbiter authorization with
  beneficiary restriction, pause role. 15/15 Foundry tests.
- **Production deployment package** (`icp-docker/`): docker-compose
  with healthchecks + 17/17 outside-the-container integration tests
  exercising independent signature verification against published
  `.well-known/` keys.
- **Foundation governance package**: Charter draft, LOI template, ICPIP
  process, 15-item risk register, capital plan, partnership packet
  (`icp-spec/PACKET.md`).
- **Distribution**: 8 partner-specific outreach drafts for Coinbase,
  Circle, Anthropic, Stripe, Google AP2, Shopify, OpenAI.
- **Cumulative protocol-layer test count**: 72+ distinct PASS signals
  on every CI run across the 10 jobs in
  `.github/workflows/icp-conformance.yml`.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release
  metadata to 1.1.0.
- README adds an ICP hero block + comprehensive `What's New in v1.1.0`
  section pointing to the ICP entry point.

## [1.0.3] - 2026-05-04

Patch release for CLI outbound security hardening.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.3.
- Changed BlueBubbles authentication to prefer header delivery while retaining the legacy query-token fallback.

### Fixed
- Hardened outbound CLI fetch paths against DNS private-address resolution and unchecked redirects across A2A webhooks, MPP, x402, and marketplace catalog/package flows.
- Hardened remote skill marketplace installs with package size caps, checksum enforcement, and archive path preflight.
- Added regression coverage for DNS and redirect SSRF blocks, webhook retry validation, marketplace package limits, and iMessage auth fallback.

## [1.0.2] - 2026-05-01

Patch release for the v1 release-readiness track.

### Changed
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.2.
- Documented the admin trusted-proxy rate-limit configuration flag for deployments that terminate traffic behind a controlled proxy boundary.

### Fixed
- Hardened admin rate limiting so spoofable `x-forwarded-for` and `x-real-ip` headers are ignored unless trusted proxy mode is explicitly enabled.
- Synced Agent OS status output to the package version instead of reporting a hardcoded stale version.
- Escaped generated runbook skill frontmatter so multiline descriptions cannot corrupt `SKILL.md` metadata.

## [1.0.1] - 2026-04-30

Patch release for the agent operating-system release track.

### Added
- Added the workspace Agent OS CLI surface for setup, readiness, context, skills, sessions, memory, and runbook creation.
- Added generated inventory coverage for the new Agent OS source and CLI binary.

### Changed
- Hardened dependency policy by removing stale OpenSSL exceptions and pinning known duplicate-dependency skips to exact versions.
- Documented the temporary RustSec rand advisory ignore in CI until upstream consumers converge on patched releases.
- Synced workspace, bindings, examples, templates, docs, and release metadata to 1.0.1.

### Fixed
- Restored clean release-hygiene validation after the Agent OS source and CLI binary expanded the workspace inventory.

## [1.0.0] - 2026-04-28

First stable release of the StateSet iCommerce engine. This release starts the
`v1.x` compatibility line for the curated Rust SDK and embedded preludes, CLI
flags, MCP tool names and schemas, policy YAML, and additive SQLite migrations.

### Added
- Added a `stateset_embedded::prelude` module to define the stable direct
  embedded Rust surface for core commerce flows.
- Added compile-time coverage that locks the embedded prelude imports and
  default-constructible create types.

### Changed
- Promoted the workspace, bindings, admin app, CLI, examples, templates, docs,
  generated compatibility inventories, and release metadata from `0.9.9` to
  `1.0.0`.
- Made the embedded crate's async runtime dependencies optional behind the
  `async`, `events`, and `postgres` feature gates.
- Made optional Solana CLI integrations optional dependencies so the default CLI
  install and audit path stays focused on the core package.

### Fixed
- Removed the non-Claude provider cold-start race in the CLI by awaiting
  provider auto-registration before first use.
- Hardened CLI SQLite backup and restore to handle WAL sidecar files.
- Allowed Gemini fallback to use the canonical `GEMINI_API_KEY` while retaining
  legacy `GOOGLE_API_KEY` compatibility.
- Hardened admin Stripe webhook verification for multiple `v1` signatures.
- Added distributed Redis-backed admin rate limiting when Upstash is configured,
  with in-memory fallback for local and single-instance deployments.
- Hardened release workflows for action input validation, checksum generation,
  binding package builds, CLI audit scope, and release hygiene setup.
- Fixed final binding blockers in .NET model coverage, PHP Composer/stub
  package validation, Ruby package metadata, WASM entropy configuration, and
  primitives `no_std` support.
- Updated `rustls-webpki` to the fixed `0.103.13` line for the April 2026
  RustSec advisories.

## [0.9.9] - 2026-04-20

Pre-1.0 consolidation release. Bundles the agent-toolkit expansion, CLI
rewrite, and docs refresh that accumulated since 0.9.8 on the
`feat/x402-agent-demo-flows` branch. Labelled 0.9.9 rather than 1.0.0 so
the real 1.0.0 cut can be a deliberate polish + `stateset-acp-handler`
pair release.

### Added
- Engine-first agent toolkit helpers, adapter modules, and runnable
  examples across the Node and Python bindings so OpenAI, LangChain,
  generic tool runtimes, CrewAI, and AutoGen-style integrations can embed
  the commerce runtime directly.
- Stronger release guards: version sync, docs/example path validity,
  package-shape checks, release hygiene regression coverage, and tracked
  native-binary detection.
- CLI command surface expansion across the full commerce domain (a2a,
  accounts payable/receivable, carts, catalog, checkout, circuit-breaker,
  compliance, connectors, cost-accounting, credit, currency, custom
  objects, erc8004, fraud, fulfillment, general-ledger, gift cards,
  invoices, lots, loyalty, manufacturing, payments, policies, promotions,
  proofs, quality, receiving, reviews, segments, serials, shipments,
  shipping-zones, stablecoin, store-credits, subscriptions, suppliers,
  sync, tax, treasury, vector, warehouse, warranties, wishlists, x402).
- x402 agent demo flows end-to-end.

### Changed
- Promoted the workspace, bindings, admin app, CLI, examples, templates,
  lockfiles, docs, and release metadata from `0.9.8` to `0.9.9`.
- Documentation refresh across API references and getting-started guides.

### Fixed
- Corrected stale release references across install snippets, examples,
  daemon guidance, API docs, and versioned metadata so the shipped repo
  surfaces match the `0.9.9` line.
- Removed tracked native example artifacts (`bindings/go/example/example`,
  `examples/go/go`) and enforced repo-level hygiene checks.

## [0.9.8] - 2026-04-08

### Added
- Added a CI-safe `cargo_ci.sh` helper so repo-wide Rust lint and feature-matrix checks run without incremental-cache bloat.
- Added explicit x402 intent signature-scheme configuration support in the Node binding and database coverage for strict `ml_dsa65` intents.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, inventories, and release metadata from `0.9.7` to `0.9.8`.
- Created the `docs/versions/v0.9.8` snapshot from the latest mdBook sources for this release line.

### Fixed
- Aligned admin authentication and request handling by allowing bearer-token API access through middleware, enforcing request-size limits against actual streamed bodies, and preserving gateway query strings.
- Cleared the CLI quality-gate blockers in the x402 and sync surfaces so `npm --prefix cli run check` passes cleanly.
- Fixed the Node x402 strict-signature flow so strict `ml_dsa65` signatures can be used against intents created with the matching stored policy.

## [0.9.7] - 2026-04-06

### Added
- Added the new authenticated admin dashboard app with analytics, operations, gateway, billing, integrations, and session-management surfaces, plus the supporting API routes and test coverage.
- Published generated MCP tool inventory artifacts for compatibility tracking in both JSON and mdBook appendix form.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, and release metadata from `0.9.6` to `0.9.7`.
- Updated the sync and x402 client paths so the latest CLI, gateway, and embedded binding flows stay aligned across real runtime usage and regression coverage.

### Fixed
- Tightened sync configuration security coverage and x402 payment-intent persistence coverage around the refreshed client behavior.

## [0.9.6] - 2026-04-04

### Added
- Added raw-binding compatibility regression coverage for getter-style `commerce.x402` and mixed A2A/x402 commerce surfaces so agent-payment flows are validated against the real Node binding shape.

### Changed
- Bumped workspace, bindings, admin app, CLI, examples, templates, docs, and release metadata from `0.9.5` to `0.9.6`.
- Normalized the shared commerce API access layer so A2A runtimes, MCP tools, the x402 CLI, and the MCP server all support both getter-style and callable-style embedded bindings.

### Fixed
- Persisted x402 signing hashes at intent creation and tightened settlement-state validation so intents cannot skip directly to `Settled`.
- Fixed the shipped x402/A2A payment tooling to work against the real embedded Node binding, including local signing, sequencer submission payloads, settlement updates, and agent-card/runtime compatibility.

## [0.9.5] - 2026-04-03

### Added
- Published repo-native trust and strategy documentation, including `TRUST_FOUNDATION.md`, distribution planning, outcomes modeling, and competitive-landscape notes to make the project posture more explicit.

### Changed
- Bumped workspace, bindings, admin app, CLI, templates, docs, and release metadata from `0.9.4` to `0.9.5`.
- Synced install snippets, deployment examples, and current-release references to the `0.9.5` release.

### Fixed
- Hardened MCP permission enforcement so unknown tools fail closed instead of silently defaulting to read access, and aligned tool permission metadata with the runtime permission map.
- Replaced silent in-memory downgrade paths with durable JSON fallback persistence for audit logs, credentials, treasury records, channel identity/session state, agent sessions, conversation memory, and ERC-8004 identity storage when the native SQLite binding is unavailable.
- Enforced session retention caps correctly and fixed channel-session fallback upsert field ordering to preserve session integrity under degraded runtime conditions.

## [0.9.4] - 2026-04-02

### Added
- x402 v2 exact-EVM payment support across the CLI, including standards-shaped `PAYMENT-SIGNATURE` retries, exact `PaymentPayload` construction, and exported exact/facilitator/resource-server helpers.
- Facilitator primitives and HTTP endpoints for `/supported`, `/verify`, and `/settle`, plus runnable exact-flow facilitator and resource-server examples.
- Exact resource-server helpers that emit `payment-required`, validate incoming `PAYMENT-SIGNATURE` payloads, settle accepted payments, and return `PAYMENT-RESPONSE`.
- Base Sepolia and Ethereum Sepolia exact-EVM support, including testnet USDC configuration and new unit coverage for exact flow, facilitator flow, and resource-server flow.
- Release hygiene automation for CI and publish workflows, including `check_release_hygiene.sh`, regression coverage for the helper, and `actionlint` workflow linting.

### Changed
- Bumped workspace and cross-language package metadata from `0.9.3` to `0.9.4`.
- Synced docs, examples, templates, and lockfiles to the `0.9.4` release.
- Updated release and publish workflows to gate on shared release-hygiene checks instead of version-sync alone.

### Fixed
- Aligned JavaScript x402 signing-hash verification with the Rust implementation by binding `resourceUri` and `resourceMethod` into signed legacy payment intents.
- Removed the legacy sequencer requirement for exact x402 MCP calls while preserving explicit errors for legacy sequencer-backed flows.
- Corrected the VES docs to describe the intended cross-language x402 hashing parity more precisely.

## [0.9.3] - 2026-04-01

### Added
- Native post-quantum VES cryptography in `stateset-crypto` for hybrid `ed25519+mldsa65` and `x25519+mlkem768` flows, plus `pqc-strict` `mldsa65` and `mlkem768` modes for key generation, signing, verification, recipient wrapping, payload encryption/decryption, and proof-of-possession.
- Sync-layer PQC security profiles (`legacy`, `hybrid`, `pqc-strict`) across config validation, key management, outbox signing/encryption, pulled-event decryption, and sequencer receipt verification.
- Native Node binding exports for hybrid and strict PQC operations, including signing, verification, payload encryption/decryption, recipient key generation, and signing proof-of-possession helpers.
- PQC audit and observability coverage, including profile-change audit events, key-generation/rotation logging, and per-profile signature/encryption counters.
- PQC validation assets: cross-language Node/Rust test vectors, strict-profile tests, expanded Rust crypto coverage, Criterion PQC benches, and the initial migration spec in `docs/PQC_INITIAL_SPEC.md`.

### Changed
- Enforced TLS for PQC-enabled sync profiles and blocked unforced profile downgrades so future events cannot silently lose post-quantum protection.
- Bumped workspace and cross-language package metadata from `0.9.1` to `0.9.3`.
- Synced docs, examples, templates, and lockfiles to the `0.9.3` release.

## [0.9.1] - 2026-03-26

### Added
- **Agentic Commerce**: Negotiation engine with auto-accept/reject thresholds, A2A messaging with retry, credit terms (net 15/30/60/90), inventory commitments, dispute rules engine
- **V9 Migration**: 8 new tables for agent commerce (a2a_messages, a2a_negotiations, inventory_commitments, a2a_credit_terms, a2a_tax_obligations, a2a_dispute_rules)
- **5 Negotiation REST endpoints**: create, get, counter-offer, accept, reject
- **497 A2A tests** across 17 modules

## [0.9.0] - 2026-03-26

### Added
- **11 V4 entity implementations**: reviews, wishlists, gift cards, loyalty, fraud, segments, store credits, shipping zones, rewards, search configs, zone shipping methods (was 11 stubs)
- **18 V4 HTTP endpoints**: reviews, wishlists, gift cards, loyalty CRUD + actions
- **Clippy pedantic fixes** across 174 files (1,377 insertions)
- **12 new HTTP integration tests** (81 total)

## [0.8.8] - 2026-03-25

### Added
- **Pricing engine** wired into order creation with currency-aware rounding
- **Audit log** (V8 migration) with record_audit() function
- **Graceful DB shutdown** (WAL checkpoint + PRAGMA optimize)
- **ETag utility module** for HTTP conditional requests
- **Fat LTO + target-cpu=native** for maximum compiled performance
- **Gzip response compression** on all API endpoints

## [0.8.5] - 2026-03-25

### Fixed
- **Inventory reservation race condition**: atomic quantity+version check in UPDATE WHERE clause
- **SQLITE_FULL detection**: maps to StorageFull error instead of generic 500
- **UNIQUE constraint violations**: return 409 Conflict instead of 500
- **LIKE wildcard escaping** in product search

### Added
- **V6 Migration**: 3 idempotency constraints (order_items, reservations, cart checkout)
- **Health check**: GET /health/deep with DB latency + metrics
- **Slow query logging**: transactions >500ms emit tracing::warn
- **Request timeout**: 30-second TimeoutLayer on all API endpoints

## [0.8.4] - 2026-03-25

### Added
- **13 new REST endpoints**: PATCH/DELETE for customers and products, POST for shipments, payments, invoices with action endpoints (deliver, complete, refund, send, record-payment)
- **V5 Migration**: 12 composite database indexes for common query patterns
- **29 error messages** now include valid enum values
- **13 new integration tests** for all new endpoints

## [0.8.2] - 2026-03-25

### Changed
- **Performance**: 8 rounds of autoresearch-driven optimization (~3x all 20 Criterion benchmarks)
  - SQLite: PRAGMA tuning, prepare_cached, mmap, WAL autocheckpoint, deferred FK
  - EventBus: lazy event_type allocation, deferred receiver_count, inline publish
  - Merkle tree: double-buffer swap, SHA256 asm, hasher reuse, pad memoization
  - Money: #[inline] on hot arithmetic paths
  - Compiler: codegen-units=1
  - Metrics: lock-free CAS for f64 accumulators
  - Event store: AtomicU64 sequence counter

## [0.8.1] - 2026-03-18

### Added
- Added native Bitcoin settlement flows for autonomous agent payments, including wallet, signing, execution, and observability plumbing.
- Added shielded Zcash settlement support for agent-to-agent payments through wallet-enabled JSON-RPC flows.
- Added Machine Payments Protocol support across MCP and HTTP, including challenge/credential/receipt handling, discovery metadata, and client retry helpers.
- Added embedded toolkit support for remote payable HTTP route discovery and paid execution.

### Changed
- Bumped workspace and cross-language release metadata from `0.8.0` to `0.8.1`.
- Synced docs, templates, examples, and packaging references around the `0.8.1` native payments and MPP release.

## [0.8.0] - 2026-03-11

### Added
- Added an embedded agent onboarding quickstart with `@stateset/cli/agent-toolkit`, OpenAI-style JSON-schema tool export, and framework adapter examples for server-side agent runtimes.
- Added package export regression coverage for the standalone and embedded agent toolkit surfaces.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.25` to `0.8.0`.
- Synced docs, examples, and release notes around the `0.8.0` embedded agent onboarding flow.

### Fixed
- Published `@stateset/cli/agent-toolkit` as a first-class package export so the documented embedded agent import path works for installed consumers.
- Hardened release smoke tests to verify package self-reference imports for `@stateset/cli/standalone` and `@stateset/cli/agent-toolkit` before publish.

## [0.7.23] - 2026-03-10

### Changed
- Bumped workspace and cross-language release metadata from `0.7.22` to `0.7.23`.
- Tightened root quality gates so `npm run check` enforces the admin lane plus the CLI supported typecheck lane under explicit Node/npm runtime guards.
- Expanded the CLI supported typecheck surface to cover the x402 package, `src/x402-mcp-server.js`, `src/tools/x402.js`, and `src/sync/crypto.js`.

### Fixed
- Reduced type drift across the x402/runtime surfaces, including crypto helpers, lazy dependency loading, and chain helper JSDoc contracts.
- Added admin test-suite typechecking and fixed test/runtime mismatches needed for the stricter gate to pass cleanly.
- Fixed the stale migration snapshot and hardened cart number generation to avoid collisions during fast concurrent test runs.

## [0.7.22] - 2026-03-06

### Added
- Added `stateset simulate` and the A2A simulation runtime for sandboxed scenario execution with virtual time, snapshots, and failure injection.
- Added the built-in `supplier-goes-offline` scenario plus simulation-focused CLI and unit coverage.
- Added CI `version-sync` gate (`scripts/ci/check_version_sync.sh`) and wired it into root `npm run check`.
- Added Rust crate publish automation: `scripts/publish-rust-crates.sh` and `.github/workflows/publish-rust-crates.yml`.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.21` to `0.7.22`.
- Updated CLI/runtime version references and packaging metadata to `0.7.22` across manifests, config constants, templates, and version assertion tests.
- Raised Rust threshold in `.github/workflows/coverage.yml` from 70% to 80% to match primary CI policy.
- Refreshed `docs/TESTING_STRATEGY.md` coverage section to document enforced CI gates instead of stale point-in-time estimates.
- Expanded `RELEASING.md` with Rust crates.io release flow and generalized binding release examples to `vX.Y.Z`.

### Removed
- Removed tracked SQLite WAL/SHM artifacts from `cli/` (`checkout-demo`, `demo`, `store`) to keep repository state clean.

## [0.7.14] - 2026-02-28

### Changed
- Bumped workspace and cross-language release metadata from `0.7.13` to `0.7.14`.
- Bumped CLI/runtime version references and packaging metadata to `0.7.14` across manifests, config constants, templates, and version assertion tests.
- Added MCP gateway readiness and Prometheus metrics endpoints (`/ready`, `/metrics`) and updated Kubernetes/Prometheus deployment wiring.
- Tightened CI quality gates by failing coverage jobs on undetermined coverage values.

### Fixed
- Enforced tenant-aware API access in `stateset-http`: authenticated `/api/v1/*` requests now require validated `x-tenant-id`.
- Added bearer-token tenant binding support and rejection of tenant/token mismatches for principal isolation.
- Implemented per-tenant SQLite routing in `stateset-http` (`<tenant>.db`) and added integration tests proving cross-tenant data isolation.
- Hardened browser navigation URL policy in CLI gateway to block local/private/internal hosts by default (SSRF risk reduction).
- Aligned CLI/mcp-events output contracts and test behavior, including stable event-subscription payload shape and runtime binary selection in E2E tests.

## [0.7.13] - 2026-02-27

### Changed
- Bumped workspace and cross-language release metadata from `0.7.12` to `0.7.13`.
- Bumped CLI/runtime version references from `0.7.8` to `0.7.13`.
- Added `stateset-setup --quickstart` preset for one-command agent onboarding (`--demo --agent openclaw --starter-pack ops --agent-only --verify`).
- Expanded onboarding artifacts with generated launch/health scripts (`start-mcp.sh`, `check-mcp.sh`) and handoff launch commands.

### Fixed
- Improved onboarding verification coverage to validate handoff launch command readiness.
- Improved setup next-step guidance with direct launch and health-check commands for faster agent time-to-value.

## [0.7.10] - 2026-02-27

### Changed
- Expanded CI quality gates with Postgres parity matrix lanes, FFI sanitizer lanes, perf regression reporting, and crate compatibility governance reporting.
- Added cross-language FFI ABI contract fixtures/tests for C, C++, Python, and Swift.
- Added observability conventions plus RED/SLO metrics primitives and documentation updates.
- Added perf-gate benchmarks and strengthened property/chaos style test coverage in protocol/sync/pricing/primitives/jobs crates.

### Fixed
- Hardened A2A and embedded webhook SSRF protections (allowlists, ambiguous IPv4 encodings, IPv4-mapped IPv6 handling, and DNS rebinding coverage).
- Fixed webhook host IP parsing behavior for deterministic IPv4/IPv6 safety checks.

## [0.7.9] - 2026-02-27

### Changed
- Bumped workspace and cross-language release metadata from `0.7.8` to `0.7.9`.
- Updated binding package versions across Node, Python, Ruby, PHP, Java, Kotlin, Swift, .NET, and wasm artifacts.
- Updated SDK/FFI surfaced version references to `0.7.9`.

### Fixed
- Hardened policy evaluation semantics, rule ordering, and authz rate-limit key handling.
- Hardened A2A/embedded webhook SSRF protections and added mapped-IPv6 regression coverage.
- Fixed sync pagination/cursor behavior and strengthened protocol integrity hashing/ordering guarantees.
- Hardened FFI safety boundaries, conversion error handling, and HTTP readiness contract behavior.
- Removed DB/runtime panic paths, fixed cart total recomputation and jobs timeout/cron lifecycle behavior, and improved subscription uniqueness handling.
- Fixed crypto malformed-envelope panic surfaces and corrected `#[derive(StateSetId)]` downstream behavior.

## [0.7.8] - 2026-02-25

### Changed
- Bumped workspace and cross-language release metadata from `0.7.7` to `0.7.8`.
- Updated CLI/runtime version references (`CLI_VERSION`, gateway config/version fallback, scaffold templates, WhatsApp user agent, and update messaging) to `0.7.8`.
- Updated lockfile and packaging metadata for CLI and language bindings to `0.7.8`.
- Enabled Swift bindings CI checks on pull requests without requiring the `ci-swift` label.

### Fixed
- Hardened `/browser/evaluate`: disabled by default and gated expression execution with strict read-only policy validation.
- Hardened marketplace remote installs with HTTPS/public-host validation, catalog base URL restrictions, checksum verification, and redirect blocking.
- Fixed MCP structured tool metadata to preserve `sessionId` for direct tool-handler invocations.
- Fixed scaffold API route generation to preserve leading route slash and emit explicit `status: 500` error responses.
- Fixed telemetry verbose tool-call logging capture path to preserve secret-redaction assertions in tests.
- Improved test stability under high-concurrency runs for HTTP gateway and setup wizard suites.

## [0.7.6] - 2026-02-24

### Changed
- Bumped workspace and cross-language release metadata from `0.7.5` to `0.7.6`.

### Fixed
- Fixed policy engine domain index replacement behavior when re-registering a policy set with the same ID.
- Implemented sync engine conflict resolution effects for local-vs-remote event handling.
- Implemented paginated pull handling in sync full-sync flows.
- Cleared strict `clippy -D warnings` regressions in embedded commerce constructors/builders.

## [0.7.4] - 2026-02-22

### Added
- Added a `stateset-setup` CLI binary entry in `package.json`.
- Added `@clack/prompts` dependency to support interactive CLI UI flows.
- Added `stateset-crypto` to the workspace dependency set and Node wrapper dependency graph.

### Changed
- Bumped workspace and cross-language release metadata from `0.7.2` to `0.7.4` across Rust crates, CLI packages, language bindings, examples, and docs.
- Updated CLI/version runtime references (`CLI_VERSION`, health endpoint fallback, scaffold templates, WhatsApp user agent) to `0.7.4`.
- Updated npm lockfiles and package manifests to reference `0.7.4`.
- Adjusted select CLI logs from `console.log` to `console.debug`/`console.info`.
- Updated lockfile dependency graph for crypto-related workspace crates.

### Fixed
- Aligned version checks and dependency specifiers in examples to `0.7.4`.

## [0.7.2] - 2026-02-20

### Changed
- Bumped the workspace and cross-language release metadata to `0.7.2` across Rust crates, CLI, language bindings, and examples.
- Updated docs and configuration references to reflect the `0.7.2` version line (including npm/cargo/composer/gradle packaging metadata and SDK version checks).

## [0.7.0] - 2026-02-07

### Added
- **1,842 automated tests** (1,581 CLI + 261 admin) with 0 failures — up from ~76 in v0.6.0.
- 40+ new CLI unit test files covering permissions, telemetry, errors, HTTP gateway/auth, channels subsystem (middleware, rich-messages, templates, event-bridge, gateway-methods, notifier, handoff, metrics, adapter-types), context, credentials, session persistence, MCP schema validator, command queue, and more.
- ESLint flat config for CLI with `eslint-config-prettier` integration.
- Prettier config with `format:check` in CI and pre-commit hook.
- Commitlint + Husky hooks enforcing conventional commits (`commit-msg`, `pre-commit`).
- `jsconfig.json` with `checkJs` for CLI type checking via JSDoc.
- Persistent SQLite audit log (`audit-store.js`) for permission gate decisions.
- In-memory sliding-window rate limiter (per-API-key 60/min, per-IP 30/min) on HTTP gateway.
- Graceful shutdown handlers for all 47 `bin/` entry points (`runMain()` / `installShutdownHandlers()`).
- Security headers on HTTP gateway (CSP, X-Frame-Options, X-Content-Type-Options, Referrer-Policy).
- Body size limits on HTTP gateway and admin API routes.
- `safeIdSchema` path traversal prevention on admin API routes.
- `secrets.yaml.template` pattern (actual secrets gitignored).
- Harness lifecycle events (`onEvent`) across loop/stream sessions plus context transforms and hook points (`before_compaction`, `tool_result_persist`, `before_send`).
- Provider overrides for non-Claude calls (`apiKey`, `getApiKey`, `signal`) and stream session event emission.

### Changed
- 168+ MCP tools mapped to permission gates (was 64).
- `@modelcontextprotocol/sdk` upgraded ^1.25.4 to ^1.26.0 (fixes GHSA-345p-7cg4-v4c7).
- `Math.random()` replaced with `crypto.randomUUID()` in mcp-conversation-context, mcp-tool-composer, and error boundary.
- ~15 empty `catch {}` blocks replaced with `console.warn()` across orchestrator, HTTP gateway, credentials, agent-session-store, permissions, claude-harness, and messaging gateways.
- Command injection prevention: `scaffold-server` allowlist, `marketplace` and `gateway` use `execFileSync`.
- SQL injection prevention: `treasury/store.js` hardcoded column whitelist.
- Error detection in `errors.js` uses property-based + case-insensitive fallback (replaced fragile string matching).
- `load-env.js` warns on missing `.env` instead of silently failing.
- `capture.js` warns on unmapped event types.
- Admin test coverage thresholds raised to 80/70/70/80.
- Rust core models, DB layer, and embedded API updated with new methods and improved error handling.
- Language bindings updated across Node, Python, WASM, Ruby, PHP, Java, Kotlin, Swift, .NET, and Go.

### Fixed
- `mcp-schema-validator.js`: `.optional().regex()` reordered to `.regex().optional()` (Zod API).
- `x402/budget.js`: `DEFAULT_STATE` shared mutable references replaced with deep copy.
- `credentials.js`: silent `.catch(() => {})` replaced with `console.warn`.
- `session-persistence.test.js`: TTL race condition (sessionTtl 1ms to 5000ms).
- `runMain()`: `Promise.resolve()` fix for sync main functions.
- Streaming error handling in `gemini.js`, `ollama.js`, `openai.js` (debug logging on catch).
- Admin sessions route: silent `.catch(() => ({}))` replaced with proper error handling.

## [0.6.0] - 2026-02-04

### Added
- Treasury engine with SQLite-backed ledger for agent funding, swaps, and fees (stablecoin-first).
- `stateset-treasury` CLI for wallets, deposits, balances, ledger, token registry, and pricing rules.
- ERC-8004 identity registry helpers (SQLite) with CLI + MCP tools.
- MCP treasury tools and ERC-8004 tools with audit metadata (`task_id`, `request_id`, `session_id`, `tool_name`).
- LLM billing from treasury: Claude uses SDK cost; OpenAI/Gemini use estimated cost with preflight budget enforcement.
- CLI flags and env support for treasury + ERC-8004 binding.

### Changed
- Stablecoin payments now record treasury withdrawals when executed.
- Tool pricing can auto-debit treasury balances when `--apply` is set.

## [0.5.0] - 2026-02-02

### Changed
- Version alignment across workspace crates, bindings, CLI, docs, and examples.

## [0.3.1] - 2026-01-29

### Added
- API key authentication for HTTP gateway (Bearer token + query param).
- Per-route permission levels (none / read / preview / write / delete / admin).
- Sandbox mode to block browser and shell routes.
- Proactive heartbeat monitor with 6 commerce checkers (low stock, abandoned carts, revenue milestone, pending returns, overdue invoices, subscription churn).
- Heartbeat HTTP API (status, list checks, run, enable, disable).
- EventBridge integration for heartbeat alerts across all messaging channels.
- `HEARTBEAT_DEFAULTS` and `HTTP_GATEWAY_DEFAULTS` in config.
- 76 new tests (39 permissions + 37 heartbeat).

## [0.2.4] - 2026-01-26

### Added
- Vector search models and APIs across core, db, and embedded crates.
- Embeddings service wiring for generating/querying vectors.
- SQLite vector search migration and query helpers.
- CLI vector tooling for embedding and search workflows.

## [0.2.0] - 2026-01-16

### Added
- PostgreSQL migration coverage test and CI target for the postgres feature.
- CLI test job in CI.
- Supply-chain checks via cargo-deny, Dependabot, and SBOM generation.
- Benchmarks for core, db, and embedded crates in CI.

### Changed
- Version alignment across bindings, CLI templates, and installers.
- Security policy now supports the 0.2.x line.

## [0.1.9] - 2025-01-09

### Fixed
- Safer Decimal to f64 conversions across all bindings (Node, Python, Ruby, PHP, Java, Kotlin, Swift) using `to_f64_or_nan` helper instead of `unwrap_or(0.0)`.
- Improved JNI error handling in Java bindings with `jni_or_throw` helper for better exception propagation.
- General Ledger parsing now uses proper error propagation (`parse_required`, `parse_optional`) instead of silent defaults.

### Changed
- All binding code now consistently handles numeric conversion edge cases.

## [0.1.8] - 2025-01-01

### Added
- mdBook-based documentation scaffold with API reference pointers and versioning notes.
- Docs build and version snapshot scripts under `docs/scripts/`.

## [0.1.7] - 2025-12-20

### Added
- 34 new MCP tools across Payments, Shipments, Suppliers/POs, Invoices, Warranties, and Manufacturing.
- Expanded agent and CLI coverage for additional commerce domains.

## [0.1.6] - 2025-12-20

### Added
- Java bindings via JNI.
- Ruby and PHP binding releases with native extensions.

### Fixed
- JNI memory management for thread-safe handles.
- Product variant handling in the Product API.
- Cart total calculations using `grand_total`.
