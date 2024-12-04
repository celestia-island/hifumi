use anyhow::Result;
use sqids::Sqids;

pub fn generate_ident(prefix: impl ToString, id: impl ToString) -> Result<String> {
    let sqids = Sqids::builder().min_length(10).build()?;
    let id = sqids.encode(
        id.to_string()
            .as_bytes()
            .iter()
            .map(|&b| b as u64)
            .collect::<Vec<_>>()
            .as_slice(),
    )?;

    Ok(format!("_{}_{}", prefix.to_string(), id.to_string()))
}
