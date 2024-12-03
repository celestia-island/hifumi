use proc_macro::TokenStream;
use syn::parse_macro_input;

mod template;
mod tools;

use template::generate_current_version_struct;
use tools::{DeriveVersion, Migration};

#[proc_macro_attribute]
pub fn version(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr: DeriveVersion = parse_macro_input!(attr);
    let input: Migration = parse_macro_input!(input);

    generate_current_version_struct(attr, input)
        .expect("Failed to generate current version struct")
        .into()
}
