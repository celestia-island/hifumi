use syn::{
    braced,
    parse::{Parse, ParseStream},
    token, Ident, LitStr, Token,
};

use super::MigrationField;

#[derive(Debug, Clone)]
pub struct MigrationComment {
    pub from: LitStr,
    pub to: LitStr,
    pub changes: Vec<MigrationField>,
}

impl Parse for MigrationComment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // "Ver1" => "Ver2" { ... }

        let from = input.parse::<LitStr>()?;
        input.parse::<Token![=>]>()?;
        let to = input.parse::<LitStr>()?;

        if input.peek(token::Brace) {
            let content;
            braced!(content in input);

            let mut changes = vec![];
            while !content.is_empty() {
                let change = content.parse::<MigrationField>()?;
                changes.push(change);

                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
            }

            Ok(Self { from, to, changes })
        } else {
            Ok(Self {
                from,
                to,
                changes: vec![],
            })
        }
    }
}
