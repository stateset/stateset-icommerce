# StateSet iCommerce v1.0.0 Release Notes

Release date: 2026-04-28

v1.0.0 is the first stable release of the StateSet iCommerce engine. It turns
the work from the pre-1.0 release train into a coordinated `v1.x` line for the
Rust SDK, embedded engine, CLI, language bindings, docs, generated inventories,
and release workflows.

This release was validated locally with the full release preflight:

```bash
npm run check:release
```

The release should still be tagged only from a commit that also passes the
remote GitHub Actions `CI Success` gate.

---

## Install

```bash
cargo add stateset-sdk --features full
pip install stateset-embedded==1.0.0
npm install @stateset/embedded@1.0.0
npm install -g @stateset/cli@1.0.0
gem install stateset_embedded -v 1.0.0
```

Other binding version lines are also aligned to `1.0.0`: Go, Java, Kotlin,
Swift, .NET, PHP, Ruby, Node.js, Python, and WASM.

---

## Stability Contract

The `v1.x` compatibility contract applies to these documented stable surfaces:

- Curated Rust SDK and embedded preludes.
- Language binding version line.
- CLI flags and command contracts.
- MCP tool names and schemas.
- Policy YAML.
- Additive SQLite migration direction.

Patch releases in `v1.x` are for non-breaking fixes, security updates,
performance improvements, and documentation. Minor releases in `v1.x` should be
additive for the stable surfaces above.

This is not a blanket stability claim for every internal module or every
experimental binding wrapper. The trust and stability boundaries are documented
in `TRUST_FOUNDATION.md`.

---

## Highlights

### Stable Embedded Surface

- Added `stateset_embedded::prelude` as the direct stable embedded Rust surface
  for core commerce flows.
- Added compile-time tests that lock the prelude imports and the
  default-constructible create types used by examples.
- Scoped the v1 Rust stability promise to the curated SDK and embedded preludes
  instead of over-promising on every public Rust item in the workspace.

### Smaller Embedded Builds

- Made the embedded crate's async runtime dependencies optional.
- Gated async pieces behind the `async`, `events`, and `postgres` features.
- Preserved the default SQLite/events path while allowing minimal embedders to
  opt into a smaller dependency surface.

### CLI Reliability

- Fixed a non-Claude provider cold-start race by awaiting provider
  auto-registration before first use.
- Kept manual provider registration safe by avoiding accidental overrides during
  auto-registration.
- Updated Gemini fallback to prefer `GEMINI_API_KEY` while preserving legacy
  `GOOGLE_API_KEY` compatibility.
- Hardened CLI SQLite backup and restore to include WAL sidecar files and remove
  stale sidecars on restore.

### Admin Security And Production Behavior

- Hardened Stripe webhook verification for multiple `v1` signatures.
- Switched webhook secret reads to runtime lookup so tests and deployments do not
  capture stale environment values.
- Added timing-safe signature comparison.
- Added Redis-backed distributed rate limiting for admin middleware when Upstash
  Redis is configured, with local in-memory fallback for development and
  single-instance deployments.

### Release Hygiene

- Promoted workspace, bindings, admin app, CLI, examples, templates, docs,
  release workflows, and generated compatibility inventories from `0.9.9` to
  `1.0.0`.
- Updated PHP Composer branch alias to `1.0.x-dev`.
- Refreshed generated binding and workspace inventories.
- Kept `Cargo.lock` changes limited to local workspace package version bumps,
  avoiding opportunistic third-party dependency churn.

---

## Validated Surface

The final release gate validated the following:

- Workspace version and release hygiene checks for `1.0.0`.
- Release hygiene regression tests.
- Rust formatting, tests, clippy, and feature matrix.
- Node binding debug build, tests, and package-shape checks.
- Python binding tests, wheel build, sdist build, and package-shape checks.
- Admin typecheck, test typecheck, lint, and full Vitest suite.
- CLI check suite.
- Engine examples for JavaScript and Python surfaces.
- Docs tool-reference checks, mdBook freshness, and versioned docs snapshot flow.
- Agent, API-command, binding API, HTTP gateway, MCP API, MCP inventory, Rust
  OpenAPI, and workspace inventory checks.
- `git diff --check`.

Generated inventory checks currently report:

| Inventory | v1.0.0 Value |
|-----------|--------------|
| Workspace members | 29 |
| CLI binaries | 49 |
| Binding surfaces | 10 |
| Agent inventory | 20 agents |
| MCP tools | 726 across 62 policy domains |
| API command coverage | 60 tool modules, fully covered |
| MCP API coverage | 354 audited methods, fully covered |
| HTTP gateway routes | 44 built-in routes |
| Rust OpenAPI | 40 paths, 57 operations |

---

## Binding Support

All packages are aligned to the `1.0.0` version line.

| Surface | Package | Release Tier | Notes |
|---------|---------|--------------|-------|
| Rust SDK | `stateset-sdk` | GA | Reference facade for stable Rust consumers. |
| Rust embedded | `stateset-embedded` | GA | Direct embedded engine with curated prelude. |
| CLI | `@stateset/cli` | GA | MCP server, commerce command surface, provider fallback. |
| Node.js | `@stateset/embedded` | GA | Native Node binding plus agent framework adapters. |
| Python | `stateset-embedded` | GA | Python binding plus agent framework adapters. |
| WASM | `@stateset/embedded-wasm` | Beta | Browser-oriented subset; persistence constraints apply. |
| Ruby | `stateset_embedded` | Beta | Core commerce binding. |
| PHP | `stateset/embedded` | Beta | Composer package with native-extension path. |
| .NET | `StateSet.Embedded` | Beta | Core commerce binding. |
| Go | `stateset-go` | Experimental | FFI wrapper surface. |
| Swift | `stateset-swift` | Experimental | SwiftPM/C FFI surface. |
| Java | `stateset-java` | Experimental | JNI wrapper surface. |
| Kotlin | `stateset-kotlin` | Experimental | Kotlin/JNI wrapper surface. |

GA surfaces are covered by the `v1.x` compatibility contract. Beta and
Experimental surfaces are released on the same version line for coordination,
but their caveats remain documented and should not be treated as full parity
with the Rust SDK.

---

## Upgrade From 0.9.9

Rust:

```toml
[dependencies]
stateset-sdk = "1.0"
stateset-embedded = "1.0"
```

Node.js:

```bash
npm install @stateset/embedded@1.0.0
npm install -g @stateset/cli@1.0.0
```

Python:

```bash
pip install stateset-embedded==1.0.0
```

No migration is required for existing SQLite databases on the supported
additive migration path. If your integration depends on non-prelude Rust items
or experimental binding wrappers, review `TRUST_FOUNDATION.md` and the binding
docs before treating that surface as stable.

---

## Release Tags

The release hygiene script accepts the following tag shapes for this line:

- `v1.0.0`
- `cli-v1.0.0`
- `py-v1.0.0`
- `java-v1.0.0`
- `php-v1.0.0`
- `ruby-v1.0.0`

---

## Known Boundaries

- Hosted control assurances and public audit claims are outside the scope of
  this repo release note.
- `pq hard finality` is not claimed by this release note.
- Some advanced accounting, A2A, and integration surfaces remain outside the
  curated prelude and may continue to evolve in additive v1 minors.
- Remote CI remains the authoritative gate before publishing artifacts.

---

## Acknowledgments

v1.0.0 is the stabilization point for the embedded, local-first commerce engine:
SQLite-native, agent-oriented, and validated across Rust, Node.js, Python,
admin, CLI, docs, and generated compatibility inventories.
