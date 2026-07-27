# CodeQL Alert Triage

Last full triage: **2026-07-26** (129 open alerts → 4 code fixes + 125 dismissals).
Previous triage: 2026-06-14 (PR #72 — real fixes for workflow permissions,
prototype pollution, ReDoS, stack-trace exposure, request forgery).

This file is the durable record of *why* alerts were dismissed, so the queue
stays interpretable. When dismissing an alert, add its class here (or extend an
existing class) and put a one-line reason in the dismissal comment.

## Dismissal classes (2026-07-26)

| Class | Count | Reason | Verified how |
|---|---|---|---|
| `rust/access-invalid-pointer` (bindings/node, stateset-ffi) | 72 | FFI raw-pointer pattern in generated napi bindings and the C-ABI layer; entry points null-check and catch unwinds. Known CodeQL noise on FFI. | Spot-read of flagged sites; all follow the guarded handle pattern |
| `rust/cleartext-logging` | 17 | All sites are struct debug-formatting inside `#[cfg(test)] mod tests` assert messages (credit.rs, accounts_receivable.rs, embedded tests) or `examples/`. Not production logging. | Each flagged line located; `mod tests` boundaries checked (credit.rs:1214, AR:1790) |
| `js/incomplete-url-substring-sanitization` | 17 | All in `cli/test/**`; fixture URL assertions, no routing decision. | Path audit |
| `rust/hard-coded-cryptographic-value` — pqc.rs:53/54 | 2 | `VES_PQC_*_HKDF_SALT_V1` are public HKDF domain-separation constants. HKDF salts are not secrets (RFC 5869). | Read constants + usage |
| — pqc.rs:735/1207 | 2 | `[0u8; SALT_SIZE]` immediately filled by `rng.fill_bytes`; the literal is the allocation, not the value. | Read both sites |
| — x402.rs:1526/1551/1573 | 3 | Fixed test-vector nonces inside `#[cfg(test)]` (mod tests at 1425). | Read sites |
| — x402_payment_intents.rs:209 | 1 | `max_nonce.map_or(0, \|n\| n + 1)` — the 0 is a sequence start, not key material. | Read site |
| `rust/cleartext-storage-database` (sqlite/mod.rs) | 1 | `sum_decimal_query` is a generic read-side aggregation helper; stores nothing. | Read function |
| `js/request-forgery` (teams/gateway.js) | 1 | `sendActivity` validates `serviceUrl` via `isAllowedBotServiceUrl` (https + Bot Framework host allowlist) *before* minting the bot token; CodeQL does not model the custom sanitizer. Fix landed in PR #72. | Read function entry |
| `js/stack-trace-exposure` (x402) | 2 | Generic JSON responders; reaching bodies are controlled protocol error codes (`ensureV2`, typeof-string-gated challenge). | Read call paths |
| `js/xss-through-exception` (mcp-events) | 1 | `sendJson` responds `application/json`; not renderable as HTML. | Read responder |
| `js/incomplete-multi-character-sanitization` (mappers) | 2 | `stripHtml` decodes to plaintext for non-HTML sinks; `&amp;`-last decode order is deliberate and regression-tested. | June triage + tests |
| `js/incomplete-sanitization` (icp-handler tests) | 3 | Test-helper escaping of repo-controlled fixtures. | Path audit |
| `js/shell-command-injection-from-environment` (stateset-daemon) | 1 | **Won't fix**: local operator installer; interpolated paths are the operator's own install location, and the operator already executes arbitrary code. | Read `run()` call sites |

## Fixed in code (2026-07-26) — alerts close on next scan

- `js/polynomial-redos` `cli/src/treasury/index.js` — ambiguous `/\.?0+$/`
  replaced with two linear passes.
- `js/redos` ×2 `scripts/ci/{check_doc_tool_refs,generate_agent_inventory}.mjs`
  — `[^_]+(?:-[^_]+)*` could match `-` in both branches (exponential
  backtracking); now `[^_-]+(?:-[^_-]+)*`, behavior verified identical on all
  real server names.
- `js/incomplete-sanitization` `cli/scripts/generate-tool-docs.mjs` —
  `escapeCell` now escapes backslashes before pipes.

## Standing guidance

- New alerts in `bindings/node/src/lib.rs` (generated) and
  `crates/stateset-ffi` are almost always the FFI pointer pattern — verify the
  guard, then dismiss into the first class above.
- Never dismiss `js/request-forgery`, `js/shell-command-injection`, or any
  `rust/hard-coded-cryptographic-value` outside the documented classes without
  reading the site.
