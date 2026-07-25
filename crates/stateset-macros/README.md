# stateset-macros

[![crates.io](https://img.shields.io/crates/v/stateset-macros.svg)](https://crates.io/crates/stateset-macros)
[![docs.rs](https://docs.rs/stateset-macros/badge.svg)](https://docs.rs/stateset-macros)

Derive macros that remove the boilerplate from commerce domain models: strongly-typed
UUID newtypes, Create/Update/Filter DTOs, and Zod-compatible JSON schemas.

A domain model in a commerce engine tends to need three near-identical shapes — one
for creation, one for partial update, one for filtering. Writing them by hand is how
they drift apart. These macros generate them from the model.

## Macros

| Macro | Generates |
|-------|-----------|
| `StateSetId` | A strongly-typed UUID newtype with `Copy + Eq + Hash + Display + Serialize + Deserialize`, `From<Uuid>`, and `FromStr` |
| `GenerateDto` | `Create{Name}`, `Update{Name}` (all fields `Option<T>`), and `{Name}Filter` structs, driven by `#[dto(...)]` attributes |
| `JsonSchema` | A JSON schema compatible with Zod validation on the TypeScript side |

## Usage

```rust,ignore
use stateset_macros::StateSetId;

#[derive(StateSetId)]
/// A unique invoice identifier.
pub struct InvoiceId(uuid::Uuid);

// Generated: Copy, Eq, Hash, Display, Serialize, Deserialize, From<Uuid>, FromStr
let id: InvoiceId = uuid::Uuid::new_v4().into();
println!("{id}");
```

DTO generation is driven by attributes on the struct and its fields:

```rust,ignore
use stateset_macros::GenerateDto;

#[derive(GenerateDto)]
#[dto(create, update, filter)]
pub struct Product {
    #[dto(skip_create)]
    pub id: ProductId,
    pub name: String,
    pub sku: String,
    #[dto(skip_update)]
    pub created_at: DateTime<Utc>,
}
// Generates: CreateProduct { name, sku, created_at }
//            UpdateProduct { name: Option<String>, sku: Option<String> }
//            ProductFilter { id, name, sku, created_at, limit, offset } — all Option
```

Struct-level attributes are `create`, `update`, and `filter`; field-level are
`skip_create`, `skip_update`, and `skip_filter`. `JsonSchema` produces a
`{snake_case_name}_json_schema() -> serde_json::Value` function. See the
[API docs](https://docs.rs/stateset-macros) for details.

Examples are marked `ignore` because a derive macro needs a real type context and the
surrounding domain crate to expand meaningfully; the macros are exercised by the
expansion tests in this crate and across `stateset-core`.

## Part of StateSet iCommerce

Used throughout [`stateset-core`](https://crates.io/crates/stateset-core) and
available through [`stateset-sdk`](https://crates.io/crates/stateset-sdk)'s `macros`
feature.

## License

MIT OR Apache-2.0
