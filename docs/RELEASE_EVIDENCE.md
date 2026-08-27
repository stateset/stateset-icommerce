# Release evidence checklist

Use this file as the release-candidate handoff. Link immutable CI artifacts;
do not mark a row complete from a local run alone.

| Gate | Required evidence | Status |
| --- | --- | --- |
| Rust correctness | CI URL for workspace tests, clippy, format, and invariant suite | Pending |
| Binding compatibility | Cross-binding vectors and generated binding inventory artifact | Pending |
| Exact money | Boundary tests covering parse, serialization, arithmetic, and refunds | Pending |
| SQLite/Postgres parity | Backend parity matrix artifact | Pending |
| MCP catalog | Generated catalog/profile counts and command-coverage artifact | Pending |
| MCP safety | Preview/apply, strict-kernel, auth, Host, and Origin negative tests | Pending |
| Storefront settlement | Invalid token/from/to/amount, reverted, low-confirmation, and replay tests | Pending |
| Protocol claims | Versioned external conformance-suite artifact for every claimed protocol | Pending |
| Supply chain | SBOM, dependency audit, provenance/signature, and package smoke installs | Pending |
| Operations | Backup/restore drill, migration rollback rehearsal, and incident runbook link | Pending |

Release notes must identify skipped gates and their user-visible impact. A
planned feature, adjacent repository, or skipped platform is never counted as
passing evidence.
