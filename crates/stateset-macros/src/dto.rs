//! Implementation of the `#[derive(GenerateDto)]` proc macro.
//!
//! Auto-generates Create, Update, and Filter DTO structs from a domain model
//! struct based on `#[dto(...)]` attributes.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, DeriveInput, Field, Fields, Meta, parse2};

/// Entry point: parse the derive input and emit the expanded token stream.
pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = match parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.to_compile_error(),
    };

    expand(input).unwrap_or_else(|err| err.to_compile_error())
}

/// Parsed struct-level `#[dto(...)]` configuration.
#[derive(Default)]
struct DtoConfig {
    create: bool,
    update: bool,
    filter: bool,
}

/// Parsed field-level `#[dto(...)]` configuration.
#[derive(Default)]
struct FieldConfig {
    skip_create: bool,
    skip_update: bool,
    skip_filter: bool,
}

fn parse_dto_config(attrs: &[Attribute]) -> DtoConfig {
    let mut config = DtoConfig::default();

    for attr in attrs {
        if !attr.path().is_ident("dto") {
            continue;
        }
        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in &nested {
                if let Meta::Path(path) = meta {
                    if path.is_ident("create") {
                        config.create = true;
                    } else if path.is_ident("update") {
                        config.update = true;
                    } else if path.is_ident("filter") {
                        config.filter = true;
                    }
                }
            }
        }
    }

    config
}

fn parse_field_config(field: &Field) -> FieldConfig {
    let mut config = FieldConfig::default();

    for attr in &field.attrs {
        if !attr.path().is_ident("dto") {
            continue;
        }
        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            for meta in &nested {
                if let Meta::Path(path) = meta {
                    if path.is_ident("skip_create") {
                        config.skip_create = true;
                    } else if path.is_ident("skip_update") {
                        config.skip_update = true;
                    } else if path.is_ident("skip_filter") {
                        config.skip_filter = true;
                    }
                }
            }
        }
    }

    config
}

/// Filter out `#[dto(...)]` attributes so they do not propagate to generated
/// structs.
fn non_dto_attrs(field: &Field) -> Vec<&Attribute> {
    field.attrs.iter().filter(|a| !a.path().is_ident("dto")).collect()
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let vis = &input.vis;
    let config = parse_dto_config(&input.attrs);

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    name,
                    "GenerateDto only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(name, "GenerateDto only supports structs"));
        }
    };

    let mut output = TokenStream::new();

    // -----------------------------------------------------------------------
    // Create DTO
    // -----------------------------------------------------------------------
    if config.create {
        let create_name = format_ident!("Create{}", name);
        let create_fields: Vec<_> = fields
            .iter()
            .filter(|f| !parse_field_config(f).skip_create)
            .map(|f| {
                let field_name = &f.ident;
                let field_ty = &f.ty;
                let attrs = non_dto_attrs(f);
                quote! {
                    #(#attrs)*
                    pub #field_name: #field_ty,
                }
            })
            .collect();

        output.extend(quote! {
            #[derive(
                ::core::fmt::Debug,
                ::core::clone::Clone,
                ::serde::Serialize,
                ::serde::Deserialize,
                ::core::default::Default,
            )]
            #vis struct #create_name {
                #(#create_fields)*
            }
        });
    }

    // -----------------------------------------------------------------------
    // Update DTO
    // -----------------------------------------------------------------------
    if config.update {
        let update_name = format_ident!("Update{}", name);
        let update_fields: Vec<_> = fields
            .iter()
            .filter(|f| !parse_field_config(f).skip_update)
            .map(|f| {
                let field_name = &f.ident;
                let field_ty = &f.ty;
                let attrs = non_dto_attrs(f);
                quote! {
                    #(#attrs)*
                    pub #field_name: ::core::option::Option<#field_ty>,
                }
            })
            .collect();

        output.extend(quote! {
            #[derive(
                ::core::fmt::Debug,
                ::core::clone::Clone,
                ::serde::Serialize,
                ::serde::Deserialize,
                ::core::default::Default,
            )]
            #vis struct #update_name {
                #(#update_fields)*
            }
        });
    }

    // -----------------------------------------------------------------------
    // Filter DTO
    // -----------------------------------------------------------------------
    if config.filter {
        let filter_name = format_ident!("{}Filter", name);
        let filter_fields: Vec<_> = fields
            .iter()
            .filter(|f| !parse_field_config(f).skip_filter)
            .map(|f| {
                let field_name = &f.ident;
                let field_ty = &f.ty;
                let attrs = non_dto_attrs(f);
                quote! {
                    #(#attrs)*
                    pub #field_name: ::core::option::Option<#field_ty>,
                }
            })
            .collect();

        output.extend(quote! {
            #[derive(
                ::core::fmt::Debug,
                ::core::clone::Clone,
                ::serde::Serialize,
                ::serde::Deserialize,
                ::core::default::Default,
            )]
            #vis struct #filter_name {
                #(#filter_fields)*
                /// Maximum number of results to return.
                pub limit: ::core::option::Option<i64>,
                /// Number of results to skip.
                pub offset: ::core::option::Option<i64>,
            }
        });
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn create_dto_generated() {
        let input = quote! {
            #[dto(create)]
            pub struct Order {
                pub id: OrderId,
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("struct CreateOrder"), "should generate CreateOrder struct");
        assert!(output_str.contains("pub id : OrderId"), "should include id field");
        assert!(output_str.contains("pub name : String"), "should include name field");
    }

    #[test]
    fn update_dto_wraps_in_option() {
        let input = quote! {
            #[dto(update)]
            pub struct Product {
                pub name: String,
                pub price: f64,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("struct UpdateProduct"),
            "should generate UpdateProduct struct"
        );
        assert!(output_str.contains("Option < String >"), "name should be Option<String>");
        assert!(output_str.contains("Option < f64 >"), "price should be Option<f64>");
    }

    #[test]
    fn filter_dto_has_limit_offset() {
        let input = quote! {
            #[dto(filter)]
            pub struct Customer {
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("struct CustomerFilter"),
            "should generate CustomerFilter struct"
        );
        assert!(
            output_str.contains("pub limit : :: core :: option :: Option < i64 >"),
            "should have limit field"
        );
        assert!(
            output_str.contains("pub offset : :: core :: option :: Option < i64 >"),
            "should have offset field"
        );
    }

    #[test]
    fn skip_create_excludes_field() {
        let input = quote! {
            #[dto(create)]
            pub struct Invoice {
                #[dto(skip_create)]
                pub id: InvoiceId,
                pub amount: f64,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("struct CreateInvoice"), "should generate CreateInvoice");
        assert!(!output_str.contains("id : InvoiceId"), "id should be skipped in Create DTO");
        assert!(output_str.contains("pub amount : f64"), "amount should be present");
    }

    #[test]
    fn skip_update_excludes_field() {
        let input = quote! {
            #[dto(update)]
            pub struct Shipment {
                pub tracking: String,
                #[dto(skip_update)]
                pub created_at: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("struct UpdateShipment"), "should generate UpdateShipment");
        assert!(output_str.contains("pub tracking"), "tracking should be present");
        assert!(!output_str.contains("created_at"), "created_at should be skipped");
    }

    #[test]
    fn skip_filter_excludes_field() {
        let input = quote! {
            #[dto(filter)]
            pub struct Return {
                pub id: ReturnId,
                #[dto(skip_filter)]
                pub internal_notes: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("struct ReturnFilter"), "should generate ReturnFilter");
        assert!(output_str.contains("pub id"), "id should be present");
        assert!(!output_str.contains("internal_notes"), "internal_notes should be skipped");
    }

    #[test]
    fn all_three_dtos() {
        let input = quote! {
            #[dto(create, update, filter)]
            pub struct Warranty {
                #[dto(skip_create)]
                pub id: WarrantyId,
                pub name: String,
                #[dto(skip_update)]
                pub created_at: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("struct CreateWarranty"), "should generate CreateWarranty");
        assert!(output_str.contains("struct UpdateWarranty"), "should generate UpdateWarranty");
        assert!(output_str.contains("struct WarrantyFilter"), "should generate WarrantyFilter");
    }

    #[test]
    fn no_dtos_when_none_requested() {
        let input = quote! {
            pub struct Standalone {
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.is_empty(), "should produce no output when no dto attributes");
    }

    #[test]
    fn non_pub_struct_produces_non_pub_dtos() {
        let input = quote! {
            #[dto(create)]
            struct Internal {
                pub name: String,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        // The DTO struct itself should not be pub (matching the input visibility)
        assert!(
            !output_str.contains("pub struct CreateInternal"),
            "should not generate pub struct for non-pub input"
        );
        assert!(output_str.contains("struct CreateInternal"), "should still generate the struct");
    }

    #[test]
    fn enum_rejected() {
        let input = quote! {
            #[dto(create)]
            pub enum NotAStruct {
                A,
                B,
            }
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("compile_error"), "should produce compile error for enums");
    }

    #[test]
    fn tuple_struct_rejected() {
        let input = quote! {
            #[dto(create)]
            pub struct TupleStruct(String, i32);
        };

        let output = derive(input);
        let output_str = output.to_string();

        assert!(
            output_str.contains("compile_error"),
            "should produce compile error for tuple structs"
        );
    }
}
