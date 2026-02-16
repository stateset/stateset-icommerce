# Docs

This directory contains the mdBook source for StateSet iCommerce.

Related planning docs:

- [Agentic Commerce Baseline](./AGENTIC_COMMERCE_BASELINE.md)

## Build locally

```bash
cargo install mdbook
mdbook build docs
```

## Serve locally

```bash
mdbook serve docs
```

## API references

Generate per-binding API docs into `docs/api/` (requires language-specific doc tools):

```bash
./docs/scripts/generate-api.sh
```

## Version snapshots

Create a versioned snapshot under `docs/versions/`:

```bash
./docs/scripts/snapshot-version.sh v0.7.0
```
