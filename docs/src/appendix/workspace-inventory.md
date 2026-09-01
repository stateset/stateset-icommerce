# Workspace Inventory

This page is generated from the local workspace manifests and package metadata.
Do not edit it by hand. Regenerate it with:

```bash
node ./scripts/ci/generate_workspace_inventory.mjs
```

Machine-readable output lives at `artifacts/compatibility/workspace-inventory.json`.

## Summary

| Metric | Value |
| --- | --- |
| Workspace version | `1.28.2` |
| Workspace members | 30 |
| Default members | 19 |
| Rust crates in workspace | 22 |
| Binding crates in workspace | 8 |
| Excluded local binding manifests | 2 |
| CLI binaries | 53 |
| CLI optional dependencies | 15 |
| Admin local embedded binding | `file:../bindings/node` |

## Product Graph Layers

These layers are computed from direct internal dependencies after excluding
test-only support crates ('stateset-benches', 'stateset-integration-tests',
and 'stateset-test-utils') so the runtime/product graph is easier to read.

| Layer | Packages |
| --- | --- |
| L1 | `stateset-a2a`, `stateset-authz`, `stateset-crypto`, `stateset-icp-client`, `stateset-jobs`, `stateset-macros`, `stateset-migrations`, `stateset-observability`, `stateset-policy`, `stateset-pricing`, `stateset-primitives` |
| L2 | `stateset-core`, `stateset-icp-iut`, `stateset-sync` |
| L3 | `stateset-db`, `stateset-embedded-wasm` |
| L4 | `stateset-embedded` |
| L5 | `stateset-dotnet`, `stateset-embedded-node`, `stateset-go`, `stateset-http`, `stateset-java`, `stateset-kotlin`, `stateset-sdk`, `stateset-swift` |
| L6 | `stateset-embedded-python`, `stateset-ffi` |

## Highest Fan-In Crates

| Package | Direct dependents |
| --- | --- |
| `stateset-core` | 13 |
| `stateset-crypto` | 12 |
| `stateset-embedded` | 10 |
| `stateset-primitives` | 6 |
| `stateset-db` | 4 |
| `stateset-observability` | 3 |
| `stateset-pricing` | 2 |
| `stateset-sdk` | 2 |
| `stateset-a2a` | 1 |
| `stateset-authz` | 1 |

## Binding Topology

| Binding | Cargo package | Published package | Direct internal deps |
| --- | --- | --- | --- |
| `bindings/dotnet` | `stateset-dotnet` | — | `stateset-core`, `stateset-crypto`, `stateset-embedded` |
| `bindings/go` | `stateset-go` | — | `stateset-core`, `stateset-crypto`, `stateset-embedded` |
| `bindings/java` | `stateset-java` | — | `stateset-core`, `stateset-crypto`, `stateset-embedded`, `stateset-primitives` |
| `bindings/kotlin` | `stateset-kotlin` | — | `stateset-core`, `stateset-crypto`, `stateset-embedded` |
| `bindings/node` | `stateset-embedded-node` | `@stateset/embedded` | `stateset-core`, `stateset-crypto`, `stateset-db`, `stateset-embedded` |
| `bindings/python` | `stateset-embedded-python` | — | `stateset-core`, `stateset-crypto`, `stateset-db`, `stateset-embedded`, `stateset-primitives`, `stateset-sdk` |
| `bindings/swift` | `stateset-swift` | — | `stateset-core`, `stateset-crypto`, `stateset-embedded` |
| `bindings/wasm` | `stateset-embedded-wasm` | `@stateset/embedded-wasm` | `stateset-core`, `stateset-crypto`, `stateset-pricing` |

## Excluded Local Binding Manifests

These binding crates exist in-repo but are intentionally excluded from default
workspace membership because they require host runtimes or headers.

| Directory | Cargo package | Description |
| --- | --- | --- |
| `bindings/php` | `stateset-embedded-php` | PHP bindings for StateSet Embedded Commerce |
| `bindings/ruby` | `stateset_embedded` | Ruby bindings for StateSet Embedded Commerce |

## CLI Surface

| Metric | Value |
| --- | --- |
| Top-level source groups | 106 |
| Tool modules | 93 |
| A2A modules | 61 |
| JS dependencies | 13 |
| Optional integrations | 15 |

## CLI Top-Level Source Groups

| Group | Files |
| --- | --- |
| `tools` | 93 |
| `commands` | 88 |
| `a2a` | 61 |
| `mcp` | 31 |
| `channels` | 29 |
| `adapters` | 20 |
| `sync` | 18 |
| `utils` | 15 |
| `harness` | 14 |
| `chains` | 9 |
| `x402` | 9 |
| `knowledge` | 6 |
| `memory` | 6 |
| `skills` | 5 |
| `treasury` | 5 |
| `providers` | 4 |
| `autonomous` | 3 |
| `heartbeat` | 3 |
| `mpp` | 3 |
| `plugins` | 3 |
| `policies` | 3 |
| `voice` | 3 |
| `workflows` | 3 |
| `approvals` | 2 |
| `coverage` | 2 |
| `webhooks` | 2 |
| `whatsapp` | 2 |
| `agent-catalog.js` | 1 |
| `agent-debugger.js` | 1 |
| `agent-definitions.js` | 1 |
| `agent-os.js` | 1 |
| `agent-router.js` | 1 |
| `agent-session-store.js` | 1 |
| `agent-toolkit.js` | 1 |
| `audit-store.js` | 1 |
| `browser` | 1 |
| `catalog` | 1 |
| `checkout` | 1 |
| `claude-harness.js` | 1 |
| `cli-schema.js` | 1 |
| `command-queue.js` | 1 |
| `command-tooling.js` | 1 |
| `commerce.js` | 1 |
| `compliance` | 1 |
| `config` | 1 |
| `config.js` | 1 |
| `connectors` | 1 |
| `context-guard.js` | 1 |
| `context.js` | 1 |
| `conversation-history.js` | 1 |
| `credentials.js` | 1 |
| `database.js` | 1 |
| `discord` | 1 |
| `doctor-checks.js` | 1 |
| `dry-run.js` | 1 |
| `env.js` | 1 |
| `erc8004` | 1 |
| `errors.js` | 1 |
| `google-chat` | 1 |
| `graceful-shutdown.js` | 1 |
| `harness-hooks.js` | 1 |
| `harness-utils.js` | 1 |
| `imessage` | 1 |
| `index.js` | 1 |
| `kernel-boundary.js` | 1 |
| `kernel-config.js` | 1 |
| `kernel-tool-execution.js` | 1 |
| `load-env.js` | 1 |
| `logger.js` | 1 |
| `main-cli-options.js` | 1 |
| `matrix` | 1 |
| `mcp-conversation-context.js` | 1 |
| `mcp-event-streamer.js` | 1 |
| `mcp-schema-validator.js` | 1 |
| `mcp-server-registry.js` | 1 |
| `mcp-server.js` | 1 |
| `mcp-tool-composer.js` | 1 |
| `mcp-tool-discovery.js` | 1 |
| `model-fallback.js` | 1 |
| `offline.js` | 1 |
| `omarchy.js` | 1 |
| `output.js` | 1 |
| `permissions.js` | 1 |
| `privacy.js` | 1 |
| `progress.js` | 1 |
| `prompts.js` | 1 |
| `retry-helpers.js` | 1 |
| `scaffold-server.js` | 1 |
| `scaffold-templates.js` | 1 |
| `seeds` | 1 |
| `session-persistence.js` | 1 |
| `session.js` | 1 |
| `settings.js` | 1 |
| `signal` | 1 |
| `slack` | 1 |
| `standalone.js` | 1 |
| `suggestions.js` | 1 |
| `teams` | 1 |
| `telegram` | 1 |
| `telemetry.js` | 1 |
| `theme.js` | 1 |
| `tiers.js` | 1 |
| `tool-schema.js` | 1 |
| `tutorial.js` | 1 |
| `ui.js` | 1 |
| `x402-mcp-server.js` | 1 |
