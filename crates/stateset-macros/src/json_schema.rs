//! Implementation of the `#[derive(JsonSchema)]` proc macro.
//!
//! Generates a function that returns a `serde_json::Value` representing a JSON
//! Schema object for the annotated struct.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, Fields, GenericArgument, PathArguments, Type, parse2};

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
        if segment.ident == "Option" {
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(inner)) = args.args.first() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

/// Determines whether a type is `Vec<T>` and returns the inner type if so.
fn extract_vec_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Vec" {
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(GenericArgument::Type(inner)) = args.args.first() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

/// Get the last segment ident of a type path (e.g., `std::string::String` ->
/// `"String"`).
fn type_ident_str(ty: &Type) -> Option<String> {
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
    } else {
        None
    }
}

/// Generate a `serde_json::json!(...)` token stream for the JSON Schema of a
/// given type.
fn type_to_schema(ty: &Type) -> TokenStream {
    // Option<T> — unwrap and recurse (the caller handles required vs optional)
    if let Some(inner) = extract_option_inner(ty) {
        return type_to_schema(inner);
    }

    // Vec<T> → array
    if let Some(inner) = extract_vec_inner(ty) {
        let items_schema = type_to_schema(inner);
        return quote! {
            ::serde_json::json!({
                "type": "array",
                "items": #items_schema
            })
        };
    }

    // Named types
    if let Some(ident) = type_ident_str(ty) {
        return match ident.as_str() {
            "String" | "str" => quote! {
                ::serde_json::json!({ "type": "string" })
            },
            "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
            | "u128" | "usize" => quote! {
                ::serde_json::json!({ "type": "integer" })
            },
            "f32" | "f64" | "Decimal" => quote! {
                ::serde_json::json!({ "type": "number" })
            },
            "bool" => quote! {
                ::serde_json::json!({ "type": "boolean" })
            },
            "Uuid" => quote! {
                ::serde_json::json!({ "type": "string", "format": "uuid" })
            },
            _ => {
                // Fallback: treat unknown types as string
                quote! {
                    ::serde_json::json!({ "type": "string" })
                }
            }
        };
    }

    // Ultimate fallback
    quote! {
        ::serde_json::json!({ "type": "string" })
    }
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;

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
            return Err(syn::Error::new_spanned(
                name,
                "JsonSchema only supports structs",
            ));
        }
    };

    let fn_name = format_ident!("{}_json_schema", to_snake_case(&name.to_string()));

    // Build the properties and required arrays
    let mut property_inserts = Vec::new();
    let mut required_names = Vec::new();

    for field in fields {
        let field_name = field
            .ident
            .as_ref()
            .expect("named fields have identifiers");
        let field_name_str = field_name.to_string();

        let is_optional = extract_option_inner(&field.ty).is_some();
        let schema = type_to_schema(&field.ty);

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

    Ok(quote! {
        /// Returns a JSON Schema for this struct.
        #vis fn #fn_name() -> ::serde_json::Value {
            let mut properties = ::serde_json::Map::new();
            #(#property_inserts)*

            ::serde_json::json!({
                "type": "object",
                "properties": ::serde_json::Value::Object(properties),
                "required": [#(#required_names),*]
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

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
        assert!(
            output_str.contains("serde_json"),
            "should reference serde_json"
        );
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

        assert!(
            output_str.contains("\"string\""),
            "String field should map to \"string\" type"
        );
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

        assert!(
            output_str.contains("\"number\""),
            "f64 should map to \"number\" type"
        );
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

        assert!(
            output_str.contains("\"boolean\""),
            "bool should map to \"boolean\" type"
        );
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

        assert!(
            output_str.contains("\"uuid\""),
            "Uuid should produce format: uuid"
        );
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

        assert!(
            output_str.contains("\"array\""),
            "Vec<T> should map to \"array\" type"
        );
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
        assert!(
            parts.len() == 2,
            "should have a 'required' key in output"
        );
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

        assert!(
            output_str.contains("compile_error"),
            "enums should produce compile error"
        );
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
    fn decimal_maps_to_number() {
        let input = quote! {
            pub struct Money {
                pub amount: Decimal,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("\"number\""),
            "Decimal should map to \"number\" type"
        );
    }
}
