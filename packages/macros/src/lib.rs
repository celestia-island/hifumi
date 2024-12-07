use anyhow::{anyhow, Result};
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use syn::parse_macro_input;

mod template;
mod tools;
mod utils;

use template::{
    generate_current_version_struct, generate_impl_froms, generate_old_version_structs,
};
use tools::{DeriveVersion, Migration};

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);
    let input: Migration = parse_macro_input!(input);

    let final_struct_fields = input
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
        .collect::<HashMap<_, _>>();
    let versions = input
        .versions
        .clone()
        .iter()
        .map(|(from, (_, rules))| Ok((from.value(), rules.changes.clone())))
        .collect::<Vec<Result<_>>>()
        .into_iter()
        .collect::<Result<Vec<_>>>()
        .expect("Failed to get versions");

    let old_version_structs = generate_old_version_structs(
        input.struct_data.ident.clone(),
        attr.version.clone(),
        final_struct_fields.clone(),
        input
            .extra_macros
            .iter()
            .map(|(key, value)| {
                quote! {
                    #[#key #value]
                }
            })
            .collect::<Vec<_>>(),
        versions.clone(),
    )
    .expect("Failed to generate old version structs");

    // Confirm the order of the versions
    let impl_versions = generate_impl_froms(
        input.struct_data.ident.clone(),
        attr.version.clone(),
        final_struct_fields.clone(),
        input
            .versions
            .iter()
            .map(|(_, (_, item))| item.clone())
            .collect(),
    )
    .expect("Failed to generate impl froms");

    let current_version_struct = generate_current_version_struct(
        attr.clone(),
        input.clone(),
        input.struct_data.ident.clone(),
        attr.version.clone(),
        final_struct_fields,
        versions,
    )
    .expect("Failed to generate current version struct");

    quote! {
        #current_version_struct
        #old_version_structs
        #impl_versions
    }
    .into()
}
