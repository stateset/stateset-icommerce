# Getting Started

## Install

Rust:

```bash
cargo add stateset-embedded
```

Node.js:

```bash
npm install @stateset/embedded@0.2.4
```

Python:

```bash
pip install stateset-embedded==0.2.4
```

## Initialize (Rust)

```rust
use stateset_embedded::Commerce;

let commerce = Commerce::new("./store.db")?;
```

## Use the CLI

Read-only by default:

```bash
stateset "show me pending orders"
```

Apply writes explicitly:

```bash
stateset --apply "ship order #12345 with tracking FEDEX123"
```

Next steps:
- See [Examples](examples.md) for end-to-end flows.
- Browse the [API Reference](api/index.md) for your language.
