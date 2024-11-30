use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod template;
mod tools;

use tools::DeriveVersion;

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);

    quote! {}.into()
}

#[proc_macro]
pub fn migration(input: TokenStream) -> TokenStream {
    todo!("implement migration")
}
