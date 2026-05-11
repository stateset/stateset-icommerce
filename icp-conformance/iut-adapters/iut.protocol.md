# IUT Adapter Protocol

An IUT (Implementation Under Test) adapter is a thin program that lets
the conformance runner exercise an ICP implementation. Adapters can be
written in any language; the only contract is JSON over stdio.

## Invocation

```sh
<adapter-command> <test-name>
```

- `adapter-command` is the entry point declared in
  `iut-adapters/registry.json` (e.g. `node iut-adapters/reference-demo.mjs`).
- `test-name` is the vector directory name (e.g. `01-aid-derivation`).

## Input

The runner writes a single JSON object to the adapter's **stdin**.
The shape depends on the test, but always matches the
`inputs.json` of the corresponding vector. Example for `01-aid-derivation`:

```json
{
  "test": "01-aid-derivation",
  "agent": {
    "ed25519_seed_hex": "0a1b2c...",
    "x25519_seed_hex": "1a2b3c..."
  },
  "intent": { ... },
  "params": { "include_signature": true }
}
```

## Output

The adapter writes a single JSON object to **stdout** containing all
fields the test's `expected.json` declares. Extra fields are permitted
(the runner ignores them). Missing required fields are a FAIL.

Example output for `01-aid-derivation`:

```json
{
  "ed25519_pubkey_hex": "...",
  "x25519_pubkey_hex": "...",
  "aid": "aid:v1:zABC...",
  "intent_canonical_bytes_hex": "7b226275...",
  "intent_signature_hex": "..."
}
```

## Error handling

- Adapters MUST NOT print non-JSON to **stdout**. All logs go to
  **stderr**.
- If the adapter cannot perform a test (e.g. unsupported feature), it
  SHOULD exit with code `2` and write a JSON object to stderr explaining
  why: `{"error": "unsupported", "reason": "ml-dsa-65 not implemented"}`.
  The runner reports this as `SKIP`, not `FAIL`.
- Any other non-zero exit is `FAIL`.
- A timeout (default 30s, configurable per test) is `FAIL`.

## Registry

Adapters declare themselves in `iut-adapters/registry.json`:

```json
{
  "reference-demo": {
    "command": ["node", "iut-adapters/reference-demo.mjs"],
    "language": "javascript",
    "implementation": "icp-spec/examples/01-aid-and-sign/demo.mjs",
    "version": "1.0.0",
    "supports": ["icp-1.0-core"]
  }
}
```

Fields:

- `command` — argv list to invoke the adapter.
- `language` — informational, for the dashboard.
- `implementation` — what's actually being tested.
- `version` — adapter version (independent of impl version).
- `supports` — list of profiles this adapter implements. Vectors outside
  this list are skipped automatically.

## Determinism requirement

Every input the runner provides is deterministic. Every output the
adapter produces MUST be byte-identical across runs given the same
inputs. Sources of nondeterminism (random nonces, current time) MUST be
parameterized as inputs to the test, never read from the OS.

This is the load-bearing property that makes cross-implementation
testing meaningful: two adapters that produce different `aid` for the
same seed are not both correct.
