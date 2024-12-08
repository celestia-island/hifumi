use anyhow::{anyhow, Result};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::{Ident, Type};

use crate::{
    tools::{MigrationComment, MigrationField},
    utils::generate_ident,
};

use super::old_version_structs::infer_older_version_struct;

fn generate_older_version_impl(
    old_struct_fields: HashMap<Ident, Type>,
    convert_rules: Vec<MigrationField>,
) -> Result<HashMap<Ident, TokenStream>> {
    let mut struct_fields = old_struct_fields
        .keys()
        .map(|key| {
            (
                key.clone(),
                quote! {
                    #key: __old.#key.to_owned()
                },
            )
        })
        .collect::<HashMap<_, _>>();

    for rule in convert_rules.iter() {
        match rule {
            MigrationField::Add { value, converter } => {
                let (key, ty) = value;

                match converter {
                    Some(converter) => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: { #converter }
                            },
                        );
                    }
                    None => {
                        struct_fields.insert(
                            key.clone(),
                            quote! {
                                #key: #ty::default()
                            },
                        );
                    }
                }
            }
            MigrationField::Copy {
                source,
                target,
                converter,
            } => {
                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let converter = quote! {
                            #[allow(unused_variables)]
                            fn _converter(#(#params),*) -> #target_ty {
                                #converter
                            }
                        };
                        let args = source
                            .iter()
                            .map(|(key, _)| quote! { __old.#key.clone() })
                            .collect::<Vec<_>>();

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: {
                                    #converter
                                    _converter(#(#args),*)
                                }
                            },
                        );
                    }
                    None => {
                        if source.len() == 1 {
                            let (source_ident, source_ty) = source.iter().next().unwrap();

                            if source_ty == target_ty {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.clone()
                                    },
                                );
                            } else {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.clone().into()
                                    },
                                );
                            }
                        } else {
                            return Err(anyhow!(
                                "Must provide a converter while copying field with multiple source"
                            ));
                        }
                    }
                }
            }
            MigrationField::Remove { value } => {
                let (ident, _) = value;
                struct_fields.remove(ident);
            }
            MigrationField::Rename {
                source,
                target,
                converter,
            } => {
                for (ident, _) in source.iter() {
                    struct_fields.remove(ident);
                }

                let (target_ident, target_ty) = target;

                match converter {
                    Some(converter) => {
                        let params = source
                            .iter()
                            .map(|(key, ty)| quote! { #key: #ty })
                            .collect::<Vec<_>>();
                        let converter = quote! {
                            #[allow(unused_variables)]
                            fn _converter(#(#params),*) -> #target_ty {
                                #converter
                            }
                        };
                        let args = source
                            .iter()
                            .map(|(key, _)| quote! { __old.#key.clone() })
                            .collect::<Vec<_>>();

                        struct_fields.insert(
                            target_ident.clone(),
                            quote! {
                                #target_ident: {
                                    #converter
                                    _converter(#(#args),*)
                                }
                            },
                        );
                    }
                    None => {
                        if source.len() == 1 {
                            let (source_ident, source_ty) = source.iter().next().unwrap();

                            if source_ty == target_ty {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident
                                    },
                                );
                            } else {
                                struct_fields.insert(
                                    target_ident.clone(),
                                    quote! {
                                        #target_ident: __old.#source_ident.into()
                                    },
                                );
                            }
                        } else {
                            return Err(anyhow!("Must provide a converter while renaming field"));
                        }
                    }
                }
            }
        }
    }

    Ok(struct_fields.into_iter().collect())
}

pub(crate) fn generate_impl_froms(
    ident: Ident,
    final_version: String,
    final_struct_fields: HashMap<Ident, Type>,
    versions: Vec<MigrationComment>,
) -> Result<TokenStream> {
    let mut temp_struct_fields = final_struct_fields.to_owned();
    let mut temp_version = final_version.clone();
    let mut impl_froms = vec![];

    while let Some(item) = versions.iter().find(|item| item.to.value() == temp_version) {
        temp_version = item.from.value();
        temp_struct_fields =
            infer_older_version_struct(temp_struct_fields.clone(), item.changes.clone())?;

        let from_ident = generate_ident(&ident, item.from.value())?;
        let to_ident = if item.to.value() == final_version {
            ident.clone()
        } else {
            generate_ident(&ident, item.to.value())?
        };

        let temp_struct_impl =
            generate_older_version_impl(temp_struct_fields.clone(), item.changes.clone())?;
        let temp_struct_impl_nearly = temp_struct_impl.values();

        impl_froms.push(quote! {
            impl From<#from_ident> for #to_ident {
                fn from(__old: #from_ident) -> Self {
                    Self {
                        #(#temp_struct_impl_nearly),*
                    }
                }
            }
        });

        if item.to.value() != final_version {
            impl_froms.push(quote! {
                impl From<#from_ident> for #ident {
                    fn from(__old: #from_ident) -> Self {
                        #to_ident::from(__old).into()
                    }
                }
            });
        }
    }

    let final_ident = generate_ident(&ident, &final_version)?;
    let final_fields = final_struct_fields
        .keys()
        .map(|ident| {
            quote! {
                #ident: __old.#ident.to_owned()
            }
        })
        .collect::<Vec<_>>();

    impl_froms.push(quote! {
        impl From<#final_ident> for #ident {
            fn from(__old: #final_ident) -> Self {
                Self {
                    #(#final_fields),*
                }
            }
        }
    });
    impl_froms.push(quote! {
        impl From<#ident> for #final_ident {
            fn from(__old: #ident) -> Self {
                Self {
                    #(#final_fields),*
                }
            }
        }
    });

    Ok(quote! {
        #(#impl_froms)*
    })
}
