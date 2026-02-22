//! Implementation of the `#[derive(StateSetId)]` proc macro.
//!
//! Generates a strongly-typed UUID newtype that is equivalent to the output of
//! the `define_id!` macro in `stateset-primitives`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse2};

/// Entry point: parse the derive input and emit the expanded token stream.
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = match parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

    expand(input).unwrap_or_else(|err| err.to_compile_error())
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;

    // Collect doc attributes from the original struct.
    let doc_attrs: Vec<_> = attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .collect();

    let name_str = name.to_string();

    Ok(quote! {
        #(#doc_attrs)*
        #[derive(
            ::core::clone::Clone,
            ::core::marker::Copy,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::cmp::PartialOrd,
            ::core::cmp::Ord,
            ::core::hash::Hash,
            ::serde::Serialize,
            ::serde::Deserialize,
        )]
        #[serde(transparent)]
        #[must_use]
        #vis struct #name(::uuid::Uuid);

        impl #name {
            /// Create a new random ID (UUID v4).
            #[inline]
            pub fn new() -> Self {
                Self(::uuid::Uuid::new_v4())
            }

            /// Create a nil (all-zeros) ID.
            #[inline]
            pub const fn nil() -> Self {
                Self(::uuid::Uuid::nil())
            }

            /// Create from an existing [`Uuid`](::uuid::Uuid).
            #[inline]
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
            pub const fn into_uuid(self) -> ::uuid::Uuid {
                self.0
            }

            /// Returns `true` if this is a nil (all-zeros) ID.
            #[inline]
            pub const fn is_nil(&self) -> bool {
                self.0.is_nil()
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

        #[cfg(any(test, feature = "arbitrary"))]
        impl ::proptest::arbitrary::Arbitrary for #name {
            type Parameters = ();
            type Strategy = ::proptest::strategy::MapInto<
                ::proptest::arbitrary::StrategyFor<[u8; 16]>,
                Self,
            >;

            fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
                use ::proptest::strategy::Strategy;
                ::proptest::arbitrary::any::<[u8; 16]>().prop_map_into()
            }
        }

        #[cfg(any(test, feature = "arbitrary"))]
        impl ::core::convert::From<[u8; 16]> for #name {
            fn from(bytes: [u8; 16]) -> Self {
                Self(::uuid::Uuid::from_bytes(bytes))
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    /// Verify that the macro parses and produces a non-empty token stream.
    #[test]
    fn basic_expansion_produces_tokens() {
        let input = quote! {
            /// A test identifier.
            pub struct TestId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        // Struct definition
        assert!(
            output_str.contains("struct TestId"),
            "should contain struct definition"
        );

        // Constructor methods
        assert!(output_str.contains("fn new"), "should contain new()");
        assert!(output_str.contains("fn nil"), "should contain nil()");
        assert!(
            output_str.contains("fn from_uuid"),
            "should contain from_uuid()"
        );
        assert!(
            output_str.contains("fn as_uuid"),
            "should contain as_uuid()"
        );
        assert!(
            output_str.contains("fn into_uuid"),
            "should contain into_uuid()"
        );
        assert!(
            output_str.contains("fn is_nil"),
            "should contain is_nil()"
        );

        // Trait impls
        assert!(
            output_str.contains("impl :: core :: default :: Default for TestId"),
            "should implement Default"
        );
        assert!(
            output_str.contains("impl :: core :: fmt :: Debug for TestId"),
            "should implement Debug"
        );
        assert!(
            output_str.contains("impl :: core :: fmt :: Display for TestId"),
            "should implement Display"
        );
        assert!(
            output_str.contains("impl :: core :: str :: FromStr for TestId"),
            "should implement FromStr"
        );
        assert!(
            output_str.contains("impl :: core :: convert :: From < :: uuid :: Uuid > for TestId"),
            "should implement From<Uuid>"
        );
        assert!(
            output_str.contains("impl :: core :: convert :: From < TestId > for :: uuid :: Uuid"),
            "should implement From<TestId> for Uuid"
        );
        assert!(
            output_str.contains("impl :: core :: convert :: AsRef < :: uuid :: Uuid > for TestId"),
            "should implement AsRef<Uuid>"
        );
    }

    /// Verify the Debug impl uses the type name.
    #[test]
    fn debug_uses_type_name() {
        let input = quote! {
            pub struct InvoiceId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("\"InvoiceId\""),
            "Debug impl should contain the type name literal"
        );
    }

    /// Verify `#[must_use]` is present.
    #[test]
    fn must_use_present() {
        let input = quote! {
            pub struct OrderId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("must_use"),
            "should contain #[must_use]"
        );
    }

    /// Verify `#[serde(transparent)]` is present.
    #[test]
    fn serde_transparent() {
        let input = quote! {
            pub struct PaymentId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("transparent"),
            "should contain serde(transparent)"
        );
    }

    /// Verify proptest Arbitrary impl is behind cfg.
    #[test]
    fn proptest_behind_cfg() {
        let input = quote! {
            pub struct CartId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("proptest"),
            "should contain proptest Arbitrary impl"
        );
        assert!(
            output_str.contains("arbitrary"),
            "should reference the 'arbitrary' feature"
        );
    }

    /// Verify doc comments are preserved.
    #[test]
    fn doc_comments_preserved() {
        let input = quote! {
            #[doc = " A unique widget identifier."]
            pub struct WidgetId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("A unique widget identifier"),
            "should preserve doc comments"
        );
    }

    /// Verify private structs produce private output.
    #[test]
    fn private_visibility() {
        let input = quote! {
            struct InternalId;
        };

        let output = derive(input);
        let output_str = output.to_string();

        // Should NOT have `pub struct` — only `struct`
        assert!(
            !output_str.starts_with("pub struct"),
            "private input should produce private output"
        );
    }
}
