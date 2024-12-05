use anyhow::Result;
use proc_macro2::Span;
use sqids::Sqids;
use syn::Ident;

pub fn generate_ident(prefix: impl ToString, id: impl ToString) -> Result<Ident> {
    let sqids = Sqids::builder().min_length(10).build()?;
    let id = sqids.encode(
        id.to_string()
            .as_bytes()
            .iter()
            .map(|&b| b as u64)
            .collect::<Vec<_>>()
            .as_slice(),
    )?;

    Ok(Ident::new(
        &format!("_{}_{}", prefix.to_string(), id.to_string()),
        Span::call_site(),
    ))
}
