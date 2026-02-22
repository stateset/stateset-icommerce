# stateset-sync

Event-sourcing sync engine for StateSet iCommerce.

Provides the core sync primitives for managing network state between local
SQLite stores and the remote VES sequencer:

- **Outbox** -- append-only event log for recording local mutations
- **EventBuffer** -- bounded FIFO buffer for pulled remote events
- **ConflictResolver** -- pluggable strategies (`RemoteWins`, `LocalWins`, `LastWriterWins`)
- **Transport** -- async trait abstracting push/pull over any network protocol
- **SyncEngine** -- orchestrator tying outbox, buffer, conflict resolution, and transport together
