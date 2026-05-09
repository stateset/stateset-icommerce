# Build Info Recipe

How to bake build & release metadata into your StateSet server binary so
the engine's `GET /version` endpoint and the admin dashboard's
`/build-info` page report verifiable provenance.

## Why this matters

Operators running the engine in production need to answer two questions
on demand:

1. **What binary is actually running?** — version, commit SHA, release
   tag, build timestamp.
2. **Did it come from a verified release pipeline?** — was it signed
   via sigstore at release time, or is it a local build that happened
   to have the right version number?

The engine surfaces this via:

- **HTTP**: `GET /version` returns a JSON body with `version`,
  `git_commit`, `git_ref`, `release_tag`, `built_at`, and `signed`.
- **Admin UI**: the `/build-info` page renders a green "Signed release"
  badge or an amber "Unsigned build" badge, with deep links to the
  GitHub commit and release.

Both reads are zero-cost at runtime — the metadata is baked into the
binary via Rust's `option_env!` macro at compile time.

## What the engine reads

| Env var (compile-time) | Becomes | Default if unset |
| --- | --- | --- |
| `CARGO_PKG_VERSION` | `version` | always set by Cargo |
| `GITHUB_SHA` | `git_commit` | `null` (omitted from JSON) |
| `GITHUB_REF_NAME` | `git_ref` | `null` (omitted from JSON) |
| `STATESET_RELEASE_TAG` | `release_tag` | `null` (omitted from JSON) |
| `STATESET_BUILD_TIMESTAMP` | `built_at` | `null` (omitted from JSON) |
| `STATESET_SIGNED` | `signed` (bool) | **`false`** |

`STATESET_SIGNED` is parsed as `true` only when set to one of `"true"`,
`"1"`, or `"yes"`. The default of `false` is a deliberate safe default
so unsigned local builds cannot accidentally claim trust.

## Local build (unsigned, for development)

No env vars needed. The default is "unsigned":

```bash
cargo build --release --bin your-server-binary
./target/release/your-server-binary &

curl http://localhost:3000/version
# {"version":"1.0.4","signed":false}
```

The admin UI will show **Unsigned build** in amber. Operators reading
that signal know not to trust the binary for production audits.

## Release pipeline (signed binary)

The standard recipe in CI (GitHub Actions example):

```yaml
- name: Build server binary with verified metadata
  env:
    # GITHUB_SHA and GITHUB_REF_NAME are auto-set by Actions.
    STATESET_RELEASE_TAG: ${{ github.ref_name }}
    STATESET_BUILD_TIMESTAMP: ${{ github.run_started_at }}
    STATESET_SIGNED: "true"
  run: cargo build --release --bin your-server-binary

- name: Sign with sigstore (keyless)
  env:
    COSIGN_YES: "true"
  run: |
    sha256sum target/release/your-server-binary > binary.SHA256SUMS
    cosign sign-blob binary.SHA256SUMS \
      --output-signature binary.SHA256SUMS.sig \
      --output-certificate binary.SHA256SUMS.pem
```

Set `STATESET_SIGNED=true` **only** in workflow runs that actually sign
the artifacts. Do not set it for unsigned debug builds.

## Verifying a deployed binary

```bash
# 1. Read the running binary's metadata.
curl -s https://your.engine.example/version | jq .
# {
#   "version": "1.0.4",
#   "git_commit": "abc123def456...",
#   "git_ref": "v1.0.4",
#   "release_tag": "v1.0.4",
#   "built_at": "2026-05-08T01:23:45Z",
#   "signed": true
# }

# 2. Cross-check the commit SHA against the GitHub release.
gh release view v1.0.4 --json targetCommitish,assets
```

In the admin dashboard, navigate to **Build info** in the sidebar. The
trust badge confirms `signed`, and the commit + release tag link out to
GitHub for one-click cross-verification.

## Why we don't ship a server binary

The `stateset-http` crate is published as a **library**, not a binary.
Operators compose it into their own server (typically a thin
`main.rs` that wires routes + state + auth) so they retain full control
over auth providers, telemetry sinks, allowlists, and other
deployment-specific concerns.

This recipe is the contract between the engine library and the
operator's binary: set these env vars at compile time and the
`/version` endpoint will report verifiable provenance. No further
engine-side changes are needed.

## Reference: Rust source

The handler lives at
[`crates/stateset-http/src/routes/health.rs`](https://github.com/stateset/stateset-icommerce/blob/main/crates/stateset-http/src/routes/health.rs)
in the `version()` and `version_response()` functions. The DTO is
`VersionResponse` in
[`crates/stateset-http/src/dto.rs`](https://github.com/stateset/stateset-icommerce/blob/main/crates/stateset-http/src/dto.rs).
