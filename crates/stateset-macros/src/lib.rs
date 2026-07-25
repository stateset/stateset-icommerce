#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

//! Procedural macros for StateSet iCommerce.
//!
//! Provides derive macros to reduce boilerplate in domain model definitions:
//!
//! - [`StateSetId`](derive@StateSetId) — Generates a strongly-typed UUID newtype
//!   with full trait implementations
//! - [`GenerateDto`](derive@GenerateDto) — Auto-generates Create/Update/Filter DTOs
//!   from a domain model struct
//! - [`JsonSchema`](derive@JsonSchema) — Generates a JSON schema compatible with Zod
//!   validation

mod define_id;
mod dto;
mod json_schema;

use proc_macro::TokenStream;

/// Derive macro for generating strongly-typed ID newtypes.
///
/// The input must be a single-field tuple struct wrapping `uuid::Uuid`:
/// `pub struct InvoiceId(uuid::Uuid);`.
///
/// The macro generates the following implementations:
///
/// - `new()`, `nil()`, `from_uuid()`, `as_uuid()`, `into_uuid()`, `is_nil()`
/// - `Debug`, `Display`, `FromStr`
/// - `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`
/// - `Serialize`, `Deserialize` (transparent)
/// - `From<Uuid>`, `From<Self> for Uuid`, `AsRef<Uuid>`
/// - `Default` (generates new v4 UUID)
/// - `#[must_use]`
///
/// # Example
///
/// ```ignore
/// use stateset_macros::StateSetId;
///
/// #[derive(StateSetId)]
/// /// A unique invoice identifier.
/// pub struct InvoiceId(uuid::Uuid);
/// ```
#[proc_macro_derive(StateSetId)]
pub fn derive_stateset_id(input: TokenStream) -> TokenStream {
    define_id::derive(input.into()).into()
}

/// Derive macro for auto-generating Create, Update, and Filter DTOs.
///
/// Controlled by `#[dto(...)]` attributes on the struct and individual fields.
///
/// # Struct-level attributes
///
/// - `#[dto(create)]` — Generate a `Create{Name}` struct
/// - `#[dto(update)]` — Generate an `Update{Name}` struct (all fields
///   `Option<T>`)
/// - `#[dto(filter)]` — Generate a `{Name}Filter` struct
///
/// # Field-level attributes
///
/// - `#[dto(skip_create)]` — Omit field from Create DTO
/// - `#[dto(skip_update)]` — Omit field from Update DTO
/// - `#[dto(skip_filter)]` — Omit field from Filter DTO
///
/// # Example
///
/// ```ignore
/// use stateset_macros::GenerateDto;
///
/// #[derive(GenerateDto)]
/// #[dto(create, update, filter)]
/// pub struct Product {
///     #[dto(skip_create)]
///     pub id: ProductId,
///     pub name: String,
///     pub sku: String,
///     #[dto(skip_update)]
///     pub created_at: DateTime<Utc>,
/// }
/// // Generates: CreateProduct { name, sku, created_at }
/// //            UpdateProduct { name: Option<String>, sku: Option<String> }
/// //            ProductFilter { id: Option<ProductId>, name: Option<String>,
/// //                            sku: Option<String>, created_at: Option<DateTime<Utc>>,
/// //                            limit: Option<i64>, offset: Option<i64> }
/// ```
#[proc_macro_derive(GenerateDto, attributes(dto))]
pub fn derive_generate_dto(input: TokenStream) -> TokenStream {
    dto::derive(input.into()).into()
}

/// Derive macro for generating a JSON schema from a struct.
///
/// Produces a function `{snake_case_name}_json_schema() -> serde_json::Value`
/// that returns a JSON Schema object compatible with Zod validation.
///
/// # Type Mapping
///
/// | Rust Type | JSON Schema |
/// |-----------|-------------|
/// | `String` | `"string"` |
/// | `i32`, `i64`, `u32`, `u64` | `"integer"` |
/// | `f32`, `f64`, `Decimal` | `"number"` |
/// | `bool` | `"boolean"` |
/// | `Vec<T>` | `{ "type": "array", "items": ... }` |
/// | `Option<T>` | inner type (removed from `"required"`) |
/// | `Uuid` | `{ "type": "string", "format": "uuid" }` |
/// | Other | `{ "type": "string" }` (fallback) |
///
/// # Example
///
/// ```ignore
/// use stateset_macros::JsonSchema;
///
/// #[derive(JsonSchema)]
/// pub struct CreateOrder {
///     pub customer_id: String,
///     pub items: Vec<OrderItem>,
///     pub currency: String,
/// }
/// // Generates: pub fn create_order_json_schema() -> serde_json::Value { ... }
/// ```
#[proc_macro_derive(JsonSchema)]
pub fn derive_json_schema(input: TokenStream) -> TokenStream {
    json_schema::derive(input.into()).into()
}

/// Compiles the code examples in `README.md` as doctests, so the crates.io
/// landing page can never drift from the real API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
