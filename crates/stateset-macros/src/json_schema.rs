//! Implementation of the `#[derive(JsonSchema)]` proc macro.
//!
//! Generates a function that returns a `serde_json::Value` representing a JSON
//! Schema object for the annotated struct.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Fields, GenericArgument, LitStr, PathArguments, Type, parse2};

/// Entry point: parse the derive input and emit the expanded token stream.
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = match parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

    expand(input).unwrap_or_else(|err| err.to_compile_error())
}

/// Convert a `PascalCase` name to `snake_case`.
fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap_or(c));
        } else {
            result.push(c);
        }
    }
    result
}

/// Determines whether a type is `Option<T>` and returns the inner type if so.
fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Option"
            && let PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(GenericArgument::Type(inner)) = args.args.first()
        {
            return Some(inner);
        }
    }
    None
}

/// Determines whether a type is `Vec<T>` and returns the inner type if so.
fn extract_vec_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Vec"
            && let PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(GenericArgument::Type(inner)) = args.args.first()
        {
            return Some(inner);
        }
    }
    None
}

/// Get the last segment ident of a type path (e.g., `std::string::String` ->
/// `"String"`).
fn type_ident_str(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// Generate a `serde_json::json!(...)` token stream for the JSON Schema of a
/// given type.
fn type_to_schema(ty: &Type) -> syn::Result<TokenStream> {
    // Option<T> — unwrap and recurse (the caller handles required vs optional)
    if let Some(inner) = extract_option_inner(ty) {
        let schema = type_to_schema(inner)?;
        return Ok(quote! { ::serde_json::json!({ "anyOf": [#schema, { "type": "null" }] }) });
    }

    // Vec<T> → array
    if let Some(inner) = extract_vec_inner(ty) {
        let items_schema = type_to_schema(inner)?;
        return Ok(quote! {
            ::serde_json::json!({
                "type": "array",
                "items": #items_schema
            })
        });
    }

    // Named types
    if let Some(ident) = type_ident_str(ty) {
        return Ok(match ident.as_str() {
            "String" | "str" => quote! {
                ::serde_json::json!({ "type": "string" })
            },
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" => quote! {
                ::serde_json::json!({ "type": "integer" })
            },
            "Decimal" => quote! {
                ::serde_json::json!({ "type": "string", "pattern": "^-?[0-9]+(\\.[0-9]+)?$" })
            },
            "f32" | "f64" => quote! {
                ::serde_json::json!({ "type": "number" })
            },
            "bool" => quote! {
                ::serde_json::json!({ "type": "boolean" })
            },
            "Uuid" => quote! {
                ::serde_json::json!({ "type": "string", "format": "uuid" })
            },
            _ => {
                return Err(syn::Error::new_spanned(
                    ty,
                    "unsupported schema type; supply #[schema(with = \"path::schema_function\")] for this field",
                ));
            }
        });
    }
    Err(syn::Error::new_spanned(
        ty,
        "unsupported schema type; supply #[schema(with = \"path::schema_function\")] for this field",
    ))
}

#[derive(Default)]
struct SerdeOptions {
    rename: Option<String>,
    rename_all: Option<String>,
    skip: bool,
    optional: bool,
    deny_unknown_fields: bool,
    schema_with: Option<syn::Path>,
}

fn serde_options(attributes: &[Attribute]) -> syn::Result<SerdeOptions> {
    let mut options = SerdeOptions::default();
    for attribute in attributes.iter().filter(|attr| attr.path().is_ident("schema")) {
        attribute.parse_nested_meta(|meta| {
            if !meta.path.is_ident("with") {
                return Err(meta.error("expected schema(with = \"path::schema_function\")"));
            }
            let path: LitStr = meta.value()?.parse()?;
            options.schema_with = Some(path.parse()?);
            Ok(())
        })?;
    }
    for attribute in attributes.iter().filter(|attr| attr.path().is_ident("serde")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                options.rename = Some(meta.value()?.parse::<LitStr>()?.value());
            } else if meta.path.is_ident("rename_all") {
                let rule: LitStr = meta.value()?.parse()?;
                if !["lowercase", "UPPERCASE", "PascalCase", "camelCase", "snake_case",
                    "SCREAMING_SNAKE_CASE", "kebab-case", "SCREAMING-KEBAB-CASE"].contains(&rule.value().as_str()) {
                    return Err(syn::Error::new_spanned(rule, "unsupported serde rename rule"));
                }
                options.rename_all = Some(rule.value());
            } else if meta.path.is_ident("skip") || meta.path.is_ident("skip_serializing") {
                options.skip = true;
            } else if meta.path.is_ident("skip_serializing_if") {
                let _: LitStr = meta.value()?.parse()?;
                options.optional = true;
            } else if meta.path.is_ident("default") {
                options.optional = true;
                if meta.input.peek(syn::Token![=]) { let _: LitStr = meta.value()?.parse()?; }
            } else if meta.path.is_ident("deny_unknown_fields") {
                options.deny_unknown_fields = true;
            } else if meta.path.is_ident("alias") || meta.path.is_ident("deserialize_with") {
                let _: LitStr = meta.value()?.parse()?;
            } else if meta.path.is_ident("skip_deserializing") {
                // Deserialization-only attributes do not alter serialized field names or types.
            } else if (meta.path.is_ident("with") || meta.path.is_ident("serialize_with")) && options.schema_with.is_some() {
                let _: LitStr = meta.value()?.parse()?;
            } else {
                return Err(meta.error("serde attribute is not supported by JsonSchema; use an explicit schema for this type"));
            }
            Ok(())
        })?;
    }
    Ok(options)
}

fn rename_field(name: &str, rule: Option<&str>) -> String {
    match rule {
        Some("UPPERCASE" | "SCREAMING_SNAKE_CASE") => name.to_ascii_uppercase(),
        Some("PascalCase" | "camelCase") => {
            let mut result = String::new();
            let mut capitalize = true;
            for ch in name.chars() {
                if ch == '_' {
                    capitalize = true;
                } else if capitalize {
                    result.push(ch.to_ascii_uppercase());
                    capitalize = false;
                } else {
                    result.push(ch);
                }
            }
            if rule == Some("camelCase")
                && let Some(first) = result.get_mut(..1)
            {
                first.make_ascii_lowercase();
            }
            result
        }
        Some("kebab-case") => name.replace('_', "-"),
        Some("SCREAMING-KEBAB-CASE") => name.to_ascii_uppercase().replace('_', "-"),
        _ => name.to_owned(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "JsonSchema requires a concrete struct without generic parameters",
        ));
    }
    let container = serde_options(&input.attrs)?;
    if container.schema_with.is_some() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "schema(with) belongs on a field; write a manual schema function for a custom root",
        ));
    }

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "JsonSchema only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(name, "JsonSchema only supports structs"));
        }
    };

    let fn_name = format_ident!("{}_json_schema", to_snake_case(&name.to_string()));

    // Build the properties and required arrays
    let mut property_inserts = Vec::new();
    let mut required_names = Vec::new();
    let mut property_names = std::collections::HashSet::new();

    for field in fields {
        let Some(field_name) = field.ident.as_ref() else {
            return Err(syn::Error::new_spanned(
                name,
                "JsonSchema only supports structs with named fields",
            ));
        };
        let options = serde_options(&field.attrs)?;
        if options.skip {
            continue;
        }
        let rust_name = field_name.to_string();
        let rust_name = rust_name.strip_prefix("r#").unwrap_or(&rust_name);
        let field_name_str = options
            .rename
            .clone()
            .unwrap_or_else(|| rename_field(rust_name, container.rename_all.as_deref()));
        if !property_names.insert(field_name_str.clone()) {
            return Err(syn::Error::new_spanned(
                field,
                "duplicate serialized field name in JsonSchema",
            ));
        }
        let is_optional =
            extract_option_inner(&field.ty).is_some() || options.optional || container.optional;
        let schema = if let Some(function) = options.schema_with {
            quote! { #function() }
        } else {
            type_to_schema(&field.ty)?
        };

        property_inserts.push(quote! {
            properties.insert(
                ::std::string::String::from(#field_name_str),
                #schema,
            );
        });

        if !is_optional {
            required_names.push(field_name_str);
        }
    }

    let extra_properties = container.deny_unknown_fields.then(|| {
        quote! {
            schema["additionalProperties"] = ::serde_json::json!(false);
        }
    });
    Ok(quote! {
        /// Returns a JSON Schema for this struct.
        #vis fn #fn_name() -> ::serde_json::Value {
            let mut properties = ::serde_json::Map::new();
            #(#property_inserts)*

            let mut schema = ::serde_json::json!({
                "type": "object",
                "properties": ::serde_json::Value::Object(properties),
                "required": [#(#required_names),*]
            });
            #extra_properties
            schema
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn unsupported_serialization_shapes_fail_explicitly() {
        for input in [
            quote! { #[serde(transparent)] struct Wrapped { value: String } },
            quote! { struct Flat { #[serde(flatten)] value: Other } },
            quote! { struct Custom { #[serde(serialize_with = "custom")] value: String } },
            quote! { struct Generic<T> { value: T } },
            quote! { #[serde(rename_all = "unknown")] struct Invalid { value: String } },
            quote! { struct Collision { #[serde(rename = "same")] a: String, #[serde(rename = "same")] b: String } },
        ] {
            let output = derive(input).to_string();
            assert!(output.contains("compile_error"), "must refuse an inaccurate schema: {output}");
        }
    }

    #[test]
    fn explicit_schema_supports_custom_serializers_and_skipped_types() {
        let output = derive(quote! {
            struct Custom {
                #[schema(with = "custom_schema")]
                #[serde(serialize_with = "serialize_custom")]
                value: CustomValue,
                #[serde(skip)] hidden: Unknown,
            }
        })
        .to_string();
        assert!(!output.contains("compile_error"), "{output}");
        assert!(output.contains("custom_schema"));
        assert!(!output.contains("hidden"));
    }

    #[test]
    fn field_renaming_matches_serde_field_rules() {
        for (rule, expected) in [
            ("lowercase", "first_name"),
            ("UPPERCASE", "FIRST_NAME"),
            ("PascalCase", "FirstName"),
            ("camelCase", "firstName"),
            ("snake_case", "first_name"),
            ("SCREAMING_SNAKE_CASE", "FIRST_NAME"),
            ("kebab-case", "first-name"),
            ("SCREAMING-KEBAB-CASE", "FIRST-NAME"),
        ] {
            assert_eq!(rename_field("first_name", Some(rule)), expected);
        }
        assert_eq!(rename_field("__private", Some("camelCase")), "private");
    }

    #[test]
    fn generates_schema_function() {
        let input = quote! {
            pub struct CreateOrder {
                pub customer_id: String,
                pub amount: f64,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("fn create_order_json_schema"),
            "should generate create_order_json_schema function"
        );
        assert!(output_str.contains("serde_json"), "should reference serde_json");
    }

    #[test]
    fn string_field_produces_string_type() {
        let input = quote! {
            pub struct Simple {
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"string\""), "String field should map to \"string\" type");
    }

    #[test]
    fn integer_field_produces_integer_type() {
        let input = quote! {
            pub struct Quantities {
                pub count: i32,
                pub total: u64,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("\"integer\""),
            "integer fields should map to \"integer\" type"
        );
    }

    #[test]
    fn float_field_produces_number_type() {
        let input = quote! {
            pub struct Pricing {
                pub price: f64,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"number\""), "f64 should map to \"number\" type");
    }

    #[test]
    fn bool_field_produces_boolean_type() {
        let input = quote! {
            pub struct Flags {
                pub active: bool,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"boolean\""), "bool should map to \"boolean\" type");
    }

    #[test]
    fn uuid_field_produces_string_with_format() {
        let input = quote! {
            pub struct WithUuid {
                pub id: Uuid,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"uuid\""), "Uuid should produce format: uuid");
    }

    #[test]
    fn vec_field_produces_array_type() {
        let input = quote! {
            pub struct WithItems {
                pub items: Vec<String>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"array\""), "Vec<T> should map to \"array\" type");
    }

    #[test]
    fn option_field_not_required() {
        let input = quote! {
            pub struct MixedFields {
                pub required_name: String,
                pub optional_note: Option<String>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        // Both fields should appear in properties
        assert!(
            output_str.contains("\"required_name\""),
            "required_name should appear in properties"
        );
        assert!(
            output_str.contains("\"optional_note\""),
            "optional_note should appear in properties"
        );

        // The "required" array should only contain "required_name".
        // In the generated code the required array is emitted as:
        //   "required": ["required_name"]
        // We verify that optional_note does NOT appear after "required".
        // Since the token stream uses quote!, we look at the structure:
        // the required_names vec should only have "required_name".
        // We can verify by checking the generated code does not list
        // optional_note in the json!({ ... "required": [...] }) invocation.
        // The required array literal is at the end of the json! macro.
        // We split at "required" and check the second part.
        let parts: Vec<&str> = output_str.splitn(2, "\"required\"").collect();
        assert!(parts.len() == 2, "should have a 'required' key in output");
        let after_required = parts[1];
        assert!(
            after_required.contains("\"required_name\""),
            "required_name should be in required array"
        );
        assert!(
            !after_required.contains("\"optional_note\""),
            "optional_note should NOT be in required array"
        );
    }

    #[test]
    fn snake_case_conversion() {
        assert_eq!(to_snake_case("CreateOrder"), "create_order");
        assert_eq!(to_snake_case("ProductFilter"), "product_filter");
        assert_eq!(to_snake_case("A"), "a");
        assert_eq!(to_snake_case("ABC"), "a_b_c");
        assert_eq!(to_snake_case("simple"), "simple");
        assert_eq!(to_snake_case("HTMLParser"), "h_t_m_l_parser");
    }

    #[test]
    fn enum_rejected() {
        let input = quote! {
            pub enum NotAStruct {
                A,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("compile_error"), "enums should produce compile error");
    }

    #[test]
    fn private_struct_produces_private_fn() {
        let input = quote! {
            struct Internal {
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            !output_str.contains("pub fn internal_json_schema"),
            "private struct should produce non-pub function"
        );
        assert!(
            output_str.contains("fn internal_json_schema"),
            "should still generate the function"
        );
    }

    #[test]
    fn decimal_maps_to_string() {
        let input = quote! {
            pub struct Money {
                pub amount: Decimal,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"string\""), "Decimal should map to a string type");
        assert!(output_str.contains("\"pattern\""));
    }

    #[test]
    fn all_integer_types_map_to_integer() {
        for int_type in &[
            "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        ] {
            let ident = syn::Ident::new(int_type, proc_macro2::Span::call_site());
            let input = quote! {
                pub struct IntTest {
                    pub value: #ident,
                }
            };

            let output = derive(input);
            let output_str = output.to_string();
            assert!(
                output_str.contains("\"integer\""),
                "{} should map to \"integer\" type",
                int_type
            );
        }
    }

    #[test]
    fn f32_maps_to_number() {
        let input = quote! {
            pub struct FloatTest {
                pub value: f32,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("\"number\""), "f32 should map to \"number\" type");
    }

    #[test]
    fn unsupported_types_require_an_explicit_schema() {
        let input = quote! {
            pub struct WithUnknown {
                pub custom: MyCustomType,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("compile_error"));
        assert!(output_str.contains("unsupported schema type"));
    }

    #[test]
    fn nested_option_vec_produces_array() {
        let input = quote! {
            pub struct Nested {
                pub tags: Option<Vec<String>>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(
            output_str.contains("\"array\""),
            "Option<Vec<String>> should produce array type (Option unwrapped)"
        );
    }

    #[test]
    fn vec_of_uuid_produces_array_with_uuid_items() {
        let input = quote! {
            pub struct WithIds {
                pub ids: Vec<Uuid>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("\"array\""), "Vec<Uuid> should produce array type");
        assert!(output_str.contains("\"uuid\""), "Vec<Uuid> items should have uuid format");
    }

    #[test]
    fn multiple_required_fields_all_listed() {
        let input = quote! {
            pub struct MultiReq {
                pub name: String,
                pub age: i32,
                pub active: bool,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        let parts: Vec<&str> = output_str.splitn(2, "\"required\"").collect();
        assert!(parts.len() == 2, "should have a 'required' key");
        let after_required = parts[1];
        assert!(after_required.contains("\"name\""), "name should be in required");
        assert!(after_required.contains("\"age\""), "age should be in required");
        assert!(after_required.contains("\"active\""), "active should be in required");
    }

    #[test]
    fn all_optional_fields_produces_empty_required_array() {
        let input = quote! {
            pub struct AllOptional {
                pub name: Option<String>,
                pub count: Option<i32>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        // Both fields in properties
        assert!(output_str.contains("\"name\""), "name should appear in properties");
        assert!(output_str.contains("\"count\""), "count should appear in properties");

        // Required array should be empty — the token stream should contain "required" : []
        let parts: Vec<&str> = output_str.splitn(2, "\"required\"").collect();
        assert!(parts.len() == 2, "should have 'required' key");
        let after_required = parts[1];
        assert!(!after_required.contains("\"name\""), "name should NOT be in required array");
        assert!(!after_required.contains("\"count\""), "count should NOT be in required array");
    }

    #[test]
    fn empty_struct_produces_empty_properties() {
        let input = quote! {
            pub struct Empty {}
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("fn empty_json_schema"), "should generate the schema function");
        assert!(output_str.contains("\"object\""), "should produce object type");
    }

    #[test]
    fn snake_case_edge_cases() {
        assert_eq!(to_snake_case(""), "");
        assert_eq!(to_snake_case("a"), "a");
        assert_eq!(to_snake_case("AB"), "a_b");
        assert_eq!(to_snake_case("XMLParser"), "x_m_l_parser");
        assert_eq!(to_snake_case("myField"), "my_field");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn tuple_struct_rejected() {
        let input = quote! {
            pub struct TupleSchema(String, i32);
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("compile_error"), "tuple structs should produce compile error");
    }

    #[test]
    fn mixed_required_optional_fields() {
        let input = quote! {
            pub struct MixedReqOpt {
                pub required_a: String,
                pub optional_b: Option<i32>,
                pub required_c: bool,
                pub optional_d: Option<Uuid>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        // All four in properties
        assert!(output_str.contains("\"required_a\""), "required_a in properties");
        assert!(output_str.contains("\"optional_b\""), "optional_b in properties");
        assert!(output_str.contains("\"required_c\""), "required_c in properties");
        assert!(output_str.contains("\"optional_d\""), "optional_d in properties");

        let parts: Vec<&str> = output_str.splitn(2, "\"required\"").collect();
        let after_required = parts[1];
        assert!(after_required.contains("\"required_a\""), "required_a in required");
        assert!(after_required.contains("\"required_c\""), "required_c in required");
        assert!(!after_required.contains("\"optional_b\""), "optional_b NOT in required");
        assert!(!after_required.contains("\"optional_d\""), "optional_d NOT in required");
    }

    #[test]
    fn vec_of_integers_produces_array_with_integer_items() {
        let input = quote! {
            pub struct WithCounts {
                pub counts: Vec<i64>,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("\"array\""), "Vec<i64> should produce array type");
        assert!(output_str.contains("\"integer\""), "Vec<i64> items should be integer");
    }

    #[test]
    fn schema_function_visibility_matches_struct() {
        // pub struct -> pub fn
        let input_pub = quote! {
            pub struct PubSchema {
                pub name: String,
            }
        };
        let output = derive(input_pub);
        let output_str = output.to_string();
        assert!(
            output_str.contains("pub fn pub_schema_json_schema"),
            "pub struct should produce pub fn"
        );

        // private struct -> private fn
        let input_priv = quote! {
            struct PrivSchema {
                pub name: String,
            }
        };
        let output = derive(input_priv);
        let output_str = output.to_string();
        assert!(
            !output_str.contains("pub fn priv_schema_json_schema"),
            "private struct should NOT produce pub fn"
        );
        assert!(
            output_str.contains("fn priv_schema_json_schema"),
            "private struct should produce private fn"
        );
    }
}
