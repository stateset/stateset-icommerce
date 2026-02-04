# Getting Started

## Install

Rust:

```bash
cargo add stateset-embedded
```

Node.js:

```bash
npm install @stateset/embedded@0.6.0
```

Python:

```bash
pip install stateset-embedded==0.6.0
```

## Initialize (Rust)

```rust
use stateset_embedded::Commerce;

let commerce = Commerce::new("./store.db")?;
```

## Use the CLI

Tip: `ss` is a shorthand alias for `stateset`.

Read-only by default:

```bash
stateset "show me pending orders"
```

Apply writes explicitly:

```bash
stateset --apply "ship order #12345 with tracking FEDEX123"
```

Optional: enable hybrid vector search (semantic + BM25):

```bash
export OPENAI_API_KEY=sk-...
stateset "find products similar to wireless earbuds"
```

Next steps:
- See [Examples](examples.md) for end-to-end flows.
- Browse the [API Reference](api/index.md) for your language.
