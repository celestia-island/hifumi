use std::collections::HashMap;

use proc_macro2::TokenStream;
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    Ident, ItemStruct, LitStr, Token,
};

use super::MigrationComment;

#[derive(Debug, Clone)]
pub struct Migration {
    pub versions: HashMap<LitStr, (LitStr, MigrationComment)>,
    pub extra_macros: Vec<(Ident, TokenStream)>,
    pub struct_data: ItemStruct,
}

impl Parse for Migration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut versions = HashMap::new();
        let mut extra_macros = vec![];

        while input.peek(Token![#]) {
            input.parse::<Token![#]>()?;

            let content;
            bracketed!(content in input);
            let key = content.parse::<Ident>()?;

            if key.to_string() == "migration" {
                let inner_content;
                parenthesized!(inner_content in content);
                let item = inner_content.parse::<MigrationComment>()?;

                versions.insert(item.from.clone(), (item.to.clone(), item));
            } else {
                let tokens: TokenStream = content.parse()?;

                extra_macros.push((key, tokens));
            }
        }

        let struct_data: ItemStruct = input.parse()?;

        Ok(Self {
            versions,
            extra_macros,
            struct_data,
        })
    }
}
