# CLI

The `stateset` CLI is a natural-language interface to the embedded engine and MCP tools.
Tip: `ss` is a shorthand alias for `stateset`.

## Safety model

- Read-only by default.
- Writes require `--apply`.

## Common commands

```bash
stateset "show me pending orders"
stateset "convert $100 USD to EUR"
stateset --apply "ship order #12345 with tracking FEDEX123"
```

## Vector search

Hybrid semantic + BM25 search is available when `OPENAI_API_KEY` is set. If SQLite
FTS5 isn't available, it falls back to embedding-only search.

```bash
export OPENAI_API_KEY=sk-...

stateset "find products similar to wireless earbuds"
stateset "search customers like enterprise retail buyers"
stateset "find orders mentioning backorder or late shipment"
```

## Full reference

The canonical CLI reference lives in `examples/cli-reference.md` and `examples/workflows.md`.
