use syn::parse::{Parse, ParseStream};

#[derive(Debug, Clone)]
pub struct Migration {}

impl Parse for Migration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {})
    }
}
