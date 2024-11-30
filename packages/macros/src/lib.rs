use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

mod template;
mod tools;

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    todo!("implement version")
}

#[proc_macro]
pub fn migration(input: TokenStream) -> TokenStream {
    todo!("implement migration")
}
