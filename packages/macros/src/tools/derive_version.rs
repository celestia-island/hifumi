use syn::parse::{Parse, ParseStream};

#[derive(Debug, Clone)]
pub struct DeriveVersion {}

impl Parse for DeriveVersion {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {})
    }
}
