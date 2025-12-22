# Sync (VES)

Verifiable Event Sync (VES) keeps local SQLite stores aligned with the sequencer service.

## Typical flow

```bash
stateset-sync init --sequencer-url <url> --tenant-id <uuid> --store-id <uuid> --api-key <key>
stateset-sync push
stateset-sync pull
```

## Key management

```bash
stateset-sync keys:generate
stateset-sync keys:register
stateset-sync keys:rotate --all --register
```

## Full guide

See `examples/getting-started-sync.md` for end-to-end setup and `examples/troubleshooting.md` for common issues.
