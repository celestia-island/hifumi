use syn::{
    parse::{Parse, ParseStream},
    Expr,
};

#[derive(Debug, Clone)]
pub struct DeriveVersion {
    pub version: String,
}

impl Parse for DeriveVersion {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            version: match input.parse::<Expr>()? {
                Expr::Lit(lit) => match lit.lit {
                    syn::Lit::Str(s) => s.value(),
                    _ => return Err(syn::Error::new(input.span(), "Expected a string literal")),
                },
                _ => return Err(syn::Error::new(input.span(), "Expected a string literal")),
            },
        })
    }
}
