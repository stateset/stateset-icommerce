# stateset-sync

Event-sourcing sync engine for StateSet iCommerce.

Provides the core sync primitives for managing network state between local
SQLite stores and the remote VES sequencer:

- **Outbox** -- append-only log for provisional local mutations before sequencing
- **EventBuffer** -- bounded FIFO buffer for pulled remote events
- **ConflictResolver** -- pluggable strategies (`RemoteWins`, `LocalWins`, `LastWriterWins`)
- **Transport** -- async trait abstracting push/pull over any network protocol
- **SequencerHttpTransport** -- concrete Rust HTTP transport for the documented sequencer REST API
- **SyncEngine** -- orchestrator tying outbox, buffer, conflict resolution, and transport together

`SyncEvent` carries the core VES envelope metadata needed for push/pull parity, including `command_id`, `base_version`, `source_agent_id`, and `agent_key_id`. The HTTP transport forwards those fields when present and only falls back to its configured agent identity when a local event does not specify them.

Authority contract:

- Local outbox sequence numbers are only FIFO positions inside one agent's pending queue.
- Canonical cross-agent ordering comes from the sequencer and must be treated separately.
- Successful pushes can return per-event acknowledgements mapping local event ids to canonical remote sequence numbers.
- When acknowledgements are present, the engine removes exactly the acknowledged local events instead of draining by prefix count.
- Explicit non-retryable rejections move those events out of the outbox into a dead-letter queue; retryable rejections stay pending.
- When per-event acknowledgements are available, `stateset-sync` retains a bounded confirmation log mapping local event ids to canonical remote sequences and receipt handles.
- Pull pagination must keep the observed canonical remote sequence separate from any server continuation cursor for the next request.

Durability contract:

- `outbox_path` persists pending local events.
- `state_path` persists remote cursor state, latest remote head metadata (`state_root`, `last_commitment_id`), highest acknowledged remote sequence, any retained push confirmations, any retained dead-letter entries, and any in-progress pull continuation cursor.
- If `state_path` is omitted but `outbox_path` is present, the engine stores a sibling snapshot next to the outbox file.
- `SyncEngine::dead_letters()`, `SyncEngine::dead_letter_for_event`, `SyncEngine::dead_letters_for_command`, `SyncEngine::dead_letters_for_entity`, `SyncEngine::latest_dead_letter_for_command`, and `SyncEngine::latest_dead_letter_for_entity` expose retained dead-letter entries for operator inspection.
- `SyncEngine::requeue_dead_letter` and `SyncEngine::discard_dead_letter` let operator workflows resolve retained dead-letter entries explicitly.
- `confirmation_capacity` bounds how many sequencer confirmations are retained durably for inspection after outbox drain.
- `SyncEngine::confirmations()` and `SyncEngine::drain_confirmations()` expose the retained local-to-canonical mapping.
- `SyncEngine::confirmation_for_event`, `SyncEngine::confirmation_for_remote_sequence`, `SyncEngine::confirmations_for_receipt`, `SyncEngine::confirmations_for_command`, `SyncEngine::confirmations_for_entity`, `SyncEngine::latest_confirmation_for_command`, and `SyncEngine::latest_confirmation_for_entity` provide direct lookup over the retained confirmation log.
- `SyncEngine::status()` exposes `caught_up`, `next_pull_cursor`, and `retained_confirmations` for pagination-aware health checks.

Rust transport example:

```rust
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
