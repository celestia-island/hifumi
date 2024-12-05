use anyhow::{anyhow, Result};
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse_macro_input;

mod template;
mod tools;
mod utils;

use template::{generate_current_version_struct, generate_impl_from, generate_old_version_structs};
use tools::{DeriveVersion, Migration};
use utils::generate_ident;

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);
    let input: Migration = parse_macro_input!(input);

    let current_version_struct = generate_current_version_struct(attr.clone(), input.clone())
        .expect("Failed to generate current version struct");
    let old_version_structs = generate_old_version_structs(
        input.struct_data.ident.clone(),
        attr.version.clone(),
        input
            .struct_data
            .fields
            .iter()
            .map(|field| match field {
                syn::Field {
                    ident: Some(ident),
                    ty,
                    ..
                } => Ok((ident.clone(), ty.clone())),
                _ => Err(anyhow!("Failed to get field ident")),
            })
            .collect::<Vec<Result<_>>>()
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .expect("Failed to get field ident")
            .into_iter()
            .collect::<HashMap<_, _>>(),
        input
            .extra_macros
            .iter()
            .map(|(key, value)| {
                quote! {
                    #[#key #value]
                }
            })
            .collect::<Vec<_>>(),
        input
            .versions
            .clone()
            .iter()
            .map(|(from, (_, rules))| Ok((from.value(), rules.changes.clone())))
            .collect::<Vec<Result<_>>>()
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .expect("Failed to get versions"),
    )
    .expect("Failed to generate old version structs");

    // Confirm the order of the versions
    let impl_versions = input
        .versions
        .iter()
        .map(|(from, (to, rules))| (from.value(), to.value(), rules.to_owned()))
        .collect::<Vec<_>>();
    let versions = impl_versions
        .iter()
        .map(|(from, to, convert_rules)| {
            Ok(generate_impl_from(
                input
                    .struct_data
                    .fields
                    .iter()
                    .map(|field| match field {
                        syn::Field {
                            ident: Some(ident),
                            ty,
                            ..
                        } => Ok((ident.clone(), ty.clone())),
                        _ => Err(anyhow!("Failed to get field ident")),
                    })
                    .collect::<Vec<Result<_>>>()
                    .into_iter()
                    .collect::<Result<Vec<_>>>()?,
                generate_ident(input.struct_data.ident.clone(), from)?,
                generate_ident(input.struct_data.ident.clone(), to)?,
                convert_rules.changes.clone(),
            )?)
        })
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .expect("Failed to generate impl from");

    quote! {
        #current_version_struct
        #old_version_structs

        #(#versions)*
    }
    .into()
}
