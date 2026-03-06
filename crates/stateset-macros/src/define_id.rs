//! Implementation of the `#[derive(StateSetId)]` proc macro.
//!
//! The derive macro expects a tuple struct newtype:
//! `pub struct MyId(uuid::Uuid);`
//!
//! Unlike attribute macros, derive macros cannot rewrite the original item.
//! This implementation therefore emits impl blocks for an existing newtype.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type, parse2};

/// Entry point: parse the derive input and emit the expanded token stream.
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = match parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

    expand(input).unwrap_or_else(|err| err.to_compile_error())
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    validate_stateset_id_input(&input)?;

    let name = &input.ident;
    let name_str = name.to_string();

    Ok(quote! {
        impl #name {
            /// Create a new random ID (UUID v4).
            #[inline]
            #[must_use]
            pub fn new() -> Self {
                Self(::uuid::Uuid::new_v4())
            }

            /// Create a nil (all-zeros) ID.
            #[inline]
            #[must_use]
            pub const fn nil() -> Self {
                Self(::uuid::Uuid::nil())
            }

            /// Create from an existing [`Uuid`](::uuid::Uuid).
            #[inline]
            #[must_use]
            pub const fn from_uuid(id: ::uuid::Uuid) -> Self {
                Self(id)
            }

            /// Get the inner [`Uuid`](::uuid::Uuid).
            #[inline]
            pub const fn as_uuid(&self) -> &::uuid::Uuid {
                &self.0
            }

            /// Consume and return the inner [`Uuid`](::uuid::Uuid).
            #[inline]
            #[must_use]
            pub const fn into_uuid(self) -> ::uuid::Uuid {
                self.0
            }

            /// Returns `true` if this is a nil (all-zeros) ID.
            #[inline]
            #[must_use]
            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl ::core::clone::Clone for #name {
            #[inline]
            fn clone(&self) -> Self {
                *self
            }
        }

        impl ::core::marker::Copy for #name {}

        impl ::core::cmp::PartialEq for #name {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl ::core::cmp::Eq for #name {}

        impl ::core::cmp::PartialOrd for #name {
            #[inline]
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                ::core::option::Option::Some(self.cmp(other))
            }
        }

        impl ::core::cmp::Ord for #name {
            #[inline]
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering {
                self.0.cmp(&other.0)
            }
        }

        impl ::core::hash::Hash for #name {
            #[inline]
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl ::serde::Serialize for #name {
            #[inline]
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                ::serde::Serialize::serialize(&self.0, serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for #name {
            #[inline]
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                <::uuid::Uuid as ::serde::Deserialize<'de>>::deserialize(deserializer).map(Self)
            }
        }

        impl ::core::default::Default for #name {
            #[inline]
            fn default() -> Self {
                Self::new()
            }
        }

        impl ::core::fmt::Debug for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}({})", #name_str, self.0)
            }
        }

        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::core::str::FromStr for #name {
            type Err = ::uuid::Error;

            #[inline]
            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                ::uuid::Uuid::parse_str(s).map(Self)
            }
        }

        impl ::core::convert::From<::uuid::Uuid> for #name {
            #[inline]
            fn from(id: ::uuid::Uuid) -> Self {
                Self(id)
            }
        }

        impl ::core::convert::From<#name> for ::uuid::Uuid {
            #[inline]
            fn from(id: #name) -> Self {
                id.0
            }
        }

        impl ::core::convert::AsRef<::uuid::Uuid> for #name {
            #[inline]
            fn as_ref(&self) -> &::uuid::Uuid {
                &self.0
            }
        }

    })
}

fn validate_stateset_id_input(input: &DeriveInput) -> syn::Result<()> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "StateSetId does not support generic parameters",
        ));
    }

    let data = match &input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "StateSetId can only be derived for tuple structs",
            ));
        }
    };

    let fields = match &data.fields {
        Fields::Unnamed(fields) => fields,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "StateSetId requires a single-field tuple struct: `struct MyId(uuid::Uuid);`",
            ));
        }
    };

    if fields.unnamed.len() != 1 {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "StateSetId requires exactly one tuple field of type `uuid::Uuid`",
        ));
    }

    let Some(only_field) = fields.unnamed.first() else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "StateSetId requires exactly one tuple field of type `uuid::Uuid`",
        ));
    };
    if !is_uuid_type(&only_field.ty) {
        return Err(syn::Error::new_spanned(
            &only_field.ty,
            "StateSetId field must be `uuid::Uuid`",
        ));
    }

    Ok(())
}

fn is_uuid_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            path.path.segments.last().map(|segment| segment.ident == "Uuid").unwrap_or(false)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn expansion_for_valid_tuple_struct_emits_impls_only() {
        let input = quote! {
            pub struct TestId(::uuid::Uuid);
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(!output_str.contains("struct TestId"), "derive should not redefine the struct");
        assert!(
            output_str.contains("impl TestId"),
            "derive should emit impls for the existing type"
        );
        assert!(output_str.contains("fn new"), "should contain new()");
        assert!(output_str.contains("Serialize for TestId"), "should implement Serialize");
        assert!(
            output_str.contains("Deserialize < 'de > for TestId"),
            "should implement Deserialize"
        );
    }

    #[test]
    fn rejects_unit_struct_input() {
        let input = quote! {
            pub struct InvoiceId;
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("compile_error"), "unit structs must be rejected");
    }

    #[test]
    fn rejects_wrong_field_type() {
        let input = quote! {
            pub struct OrderId(String);
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("compile_error"), "non-Uuid fields must be rejected");
    }

    #[test]
    fn rejects_generic_ids() {
        let input = quote! {
            pub struct GenericId<T>(::uuid::Uuid, ::core::marker::PhantomData<T>);
        };

        let output = derive(input);
        let output_str = output.to_string();
        assert!(output_str.contains("compile_error"), "generic IDs must be rejected");
    }
}
