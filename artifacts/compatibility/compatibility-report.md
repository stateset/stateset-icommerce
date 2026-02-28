# Crate Compatibility Report

- Generated (UTC): `2026-02-27T17:59:31.529425+00:00`
- Total rows: `15`
- Passed: `0`
- Failed: `0`
- Skipped: `15`

| Crate | Feature Set | Status | Duration (s) |
|---|---|---|---:|
| `stateset-primitives` | `default` | SKIPPED | 0 |
| `stateset-core` | `default` | SKIPPED | 0 |
| `stateset-core` | `metrics` | SKIPPED | 0 |
| `stateset-db` | `default` | SKIPPED | 0 |
| `stateset-db` | `postgres` | SKIPPED | 0 |
| `stateset-db` | `postgres+saga` | SKIPPED | 0 |
| `stateset-embedded` | `default` | SKIPPED | 0 |
| `stateset-embedded` | `postgres` | SKIPPED | 0 |
| `stateset-embedded` | `postgres+events` | SKIPPED | 0 |
| `stateset-ffi` | `default` | SKIPPED | 0 |
| `stateset-sdk` | `default` | SKIPPED | 0 |
| `stateset-http` | `default` | SKIPPED | 0 |
| `stateset-protocol` | `default` | SKIPPED | 0 |
| `stateset-migrations` | `default` | SKIPPED | 0 |
| `stateset-authz` | `default` | SKIPPED | 0 |

## Declared Features

| Crate | Features | Default Feature Members |
|---|---|---|
| `stateset-authz` | `-` | `-` |
| `stateset-core` | `default, embeddings, metrics, sqlx-postgres, test-utils` | `-` |
| `stateset-db` | `default, postgres, saga, sqlite, vector` | `sqlite` |
| `stateset-embedded` | `default, events, hex, hmac, opentelemetry, opentelemetry-otlp, opentelemetry_sdk, postgres, prometheus, r2d2, r2d2_sqlite, reqwest, rusqlite, sha2, sqlite, sqlite-events, sqlx, tracing-opentelemetry, vector` | `events, sqlite` |
| `stateset-ffi` | `cbindgen, default` | `-` |
| `stateset-http` | `-` | `-` |
| `stateset-migrations` | `-` | `-` |
| `stateset-primitives` | `arbitrary, default, sqlx-postgres, std` | `std` |
| `stateset-protocol` | `-` | `-` |
| `stateset-sdk` | `core, crypto, default, full, macros, policy` | `core` |
