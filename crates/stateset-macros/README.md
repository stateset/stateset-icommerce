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

```rust
use stateset_macros::StateSetId;

#[derive(StateSetId)]
/// A unique invoice identifier.
pub struct InvoiceId(uuid::Uuid);

// Generated: Copy, Eq, Hash, Display, Serialize, Deserialize, From<Uuid>, FromStr
let id: InvoiceId = uuid::Uuid::new_v4().into();
println!("{id}");
```

DTO generation is driven by attributes on the struct and its fields:

```rust
use stateset_macros::GenerateDto;
use uuid::Uuid as ProductId;

#[derive(GenerateDto)]
#[dto(create, update, filter)]
pub struct Product {
    #[dto(skip_create)]
    pub id: ProductId,
    pub name: String,
    pub sku: String,
    #[dto(skip_update)]
    pub created_at: String,
}
// Generates: CreateProduct { name, sku, created_at }
//            UpdateProduct { name: Option<String>, sku: Option<String> }
//            ProductFilter { id, name, sku, created_at, limit, offset } — all Option
```

Struct-level attributes are `create`, `update`, and `filter`; field-level are
`skip_create`, `skip_update`, and `skip_filter`. `JsonSchema` produces a
`{snake_case_name}_json_schema() -> serde_json::Value` function. It preserves
Serde field names, skips, defaults, nullable options, and unknown-field refusal.
Decimal fields advertise strings. Unsupported types and serialization attributes
fail compilation instead of silently advertising a string schema. Supply
`#[schema(with = "path::schema_function")]` on a field for a nested model or
custom serializer; the function returns the complete field schema as
`serde_json::Value`. See the
[API docs](https://docs.rs/stateset-macros) for details.

The examples compile and run as doctests. Downstream integration tests also
compare generated schemas with actual Serde wire values.

## Part of StateSet iCommerce

Available through [`stateset-sdk`](https://crates.io/crates/stateset-sdk)'s `macros`
feature.

## License

MIT OR Apache-2.0
