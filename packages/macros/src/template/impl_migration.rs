use std::collections::HashMap;

use anyhow::{anyhow, ensure, Result};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, Type};

use crate::tools::MigrationField;

pub(crate) fn generate_impl_from(
    old_struct_fields: Vec<(Ident, Type)>,
    from_ident: String,
    to_ident: String,
    convert_rules: Vec<MigrationField>,
) -> Result<TokenStream> {
    let mut struct_fields = old_struct_fields
        .iter()
        .map(|(key, _)| {
            (
                key.clone(),
                quote! {
                    #key: old.#key,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    for rule in convert_rules.iter() {
        match rule {
            MigrationField::AddField { value, converter } => {
                let (key, ty) = value;
                match converter {
                    Some(converter) => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: #ty::from({ #converter }),
                            },
                        );
                    }
                    None => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: #ty::default(),
                            },
                        );
                    }
                }
            }
            MigrationField::CopyField {
                source,
                target,
                converter,
            } => {
                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let source_params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let source_params = quote! { { #(#source_params),* } };

                        let source_args = source
                            .iter()
                            .map(|(key, _)| quote! { old.#key })
                            .collect::<Vec<_>>();
                        let source_args = quote! { #(#source_args),* };

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: #target_ty::from((|#source_params| { #converter })(#source_args)),
                            },
                        );
                    }
                    None => {
                        ensure!(
                            source.len() == 1,
                            "Must provide a converter while copying field with multiple source"
                        );

                        let (source_ident, _) = source.iter().next().unwrap();
                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: #target_ty::from(old.#source_ident.clone()),
                            },
                        );
                    }
                }
            }
            MigrationField::RemoveField { value } => {
                let (ident, _) = value;
                struct_fields.remove(&ident);
            }
            MigrationField::RenameField {
                source,
                target,
                converter,
            } => {
                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let source_params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let source_params = quote! { #(#source_params),* };

                        let source_args = source
                            .iter()
                            .map(|(key, _)| quote! { old.#key })
                            .collect::<Vec<_>>();
                        let source_args = quote! { { #(#source_args),* } };

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: #target_ty::from((|#source_params| { #converter })(#source_args)),
                            },
                        );
                    }
                    None => {
                        return Err(anyhow!("Must provide a converter while renaming field"));
                    }
                }
            }
        }
    }

    let from_ident = Ident::new(&from_ident, Span::call_site());
    let to_ident = Ident::new(&to_ident, Span::call_site());
    let struct_fields = struct_fields.values().cloned().collect::<Vec<_>>();
    Ok(quote! {
        impl From<#from_ident> for #to_ident {
            fn from(old: #from_ident) -> Self {
                #(#struct_fields),*
            }
        }
    })
}
