use anyhow::Result;
use sqids::Sqids;

pub fn generate_id(prefix: impl ToString, id: impl ToString) -> Result<String> {
    let id = format!("{}_{}", prefix.to_string(), id.to_string());
    let sqids = Sqids::builder().min_length(10).build()?;
    let id = sqids.encode(
        id.as_bytes()
            .iter()
            .map(|&b| b as u64)
            .collect::<Vec<_>>()
            .as_slice(),
    )?;

    Ok(id)
}
