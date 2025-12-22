# CLI

The `stateset` CLI is a natural-language interface to the embedded engine and MCP tools.

## Safety model

- Read-only by default.
- Writes require `--apply`.

## Common commands

```bash
stateset "show me pending orders"
stateset "convert $100 USD to EUR"
stateset --apply "ship order #12345 with tracking FEDEX123"
```

## Full reference

The canonical CLI reference lives in `examples/cli-reference.md` and `examples/workflows.md`.
