# stateset-sync

[![crates.io](https://img.shields.io/crates/v/stateset-sync.svg)](https://crates.io/crates/stateset-sync)
[![docs.rs](https://docs.rs/stateset-sync/badge.svg)](https://docs.rs/stateset-sync)

Event-sourcing sync engine for StateSet iCommerce — the primitives for reconciling a
local store with a remote VES sequencer.

Local writes land in an append-only outbox immediately, so the engine keeps working
offline. Canonical ordering comes from the sequencer, which acknowledges each event
with a remote sequence number and a receipt handle. That split — local FIFO position
versus canonical order — is the crate's central idea, and conflating the two is how
sync engines silently lose or duplicate events.

## Components

- **`Outbox`** — append-only log for provisional local mutations before sequencing
- **`EventBuffer`** — bounded FIFO buffer for pulled remote events
- **`ConflictResolver`** — pluggable strategies (`RemoteWins`, `LocalWins`, `LastWriterWins`)
- **`Transport`** — async trait abstracting push/pull over any protocol
- **`SequencerHttpTransport`** — concrete HTTP transport for the sequencer REST API
- **`SyncEngine`** — orchestrator tying outbox, buffer, conflict resolution, and transport together
- **`attestation` / `commitment` / `convergence`** — verifiable settlement proofs anchored in remote commitments

`SyncEvent` carries the VES envelope metadata needed for push/pull parity —
`command_id`, `base_version`, `source_agent_id`, `agent_key_id`. The HTTP transport
forwards those when present and only falls back to its configured agent identity when
a local event doesn't specify them.

## Usage

```rust,no_run
use stateset_sync::{SequencerHttpTransport, SyncConfig, SyncEngine};

let config = SyncConfig::new(
    "550e8400-e29b-41d4-a716-446655440000",
    "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
    "6ba7b811-9dad-11d1-80b4-00c04fd430c8",
)
.with_outbox_path("/tmp/sync-outbox.json");

let transport = SequencerHttpTransport::from_config(
    "https://sequencer.stateset.com",
    &config,
)?
.with_api_key("ss_example_key")
.with_agent_key_id(1);

let engine = SyncEngine::new(config)?;
# let _ = transport;
# let _ = engine;
# Ok::<(), stateset_sync::SyncError>(())
```

## Authority Contract

- Local outbox sequence numbers are only FIFO positions inside one agent's pending
  queue. Canonical cross-agent ordering comes from the sequencer and is tracked
  separately.
- Successful pushes may return per-event acknowledgements mapping local event ids to
  canonical remote sequence numbers. When acknowledgements are present, the engine
  removes exactly the acknowledged events instead of draining by prefix count.
- Explicit non-retryable rejections move events into a dead-letter queue; retryable
  rejections stay pending.
- Pull pagination keeps the observed canonical remote sequence separate from the
  server continuation cursor for the next request.

## Durability Contract

- `outbox_path` persists pending local events.
- `state_path` persists remote cursor state, latest remote head metadata
  (`state_root`, `last_commitment_id`), the highest acknowledged remote sequence,
  retained push confirmations and dead-letter entries, and any in-progress pull
  cursor. If omitted while `outbox_path` is set, a sibling snapshot is written next to
  the outbox file.
- `confirmation_capacity` bounds how many sequencer confirmations are retained for
  inspection after the outbox drains.

`SyncEngine` exposes lookup and operator surfaces over both logs — confirmations and
dead letters can be queried by event id, remote sequence, receipt handle, command, or
entity, then requeued or discarded — and `status()` reports `caught_up`,
`next_pull_cursor`, and `retained_confirmations` for pagination-aware health checks.
See the [API docs](https://docs.rs/stateset-sync) for the full method list.

## Part of StateSet iCommerce

Uses [`stateset-crypto`](https://crates.io/crates/stateset-crypto) for event signing
and attestation. Available through
[`stateset-sdk`](https://crates.io/crates/stateset-sdk)'s `sync` feature, which wraps
it in a higher-level `SyncRuntime`.

## License

MIT OR Apache-2.0
