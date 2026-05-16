pub mod helius;
pub mod rpc;
pub mod transactions;
pub mod wallet;

use anyhow::{anyhow, Result};

pub fn validate_address(address: &str) -> Result<()> {
    let trimmed = address.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("empty address"));
    }
    let decoded = bs58::decode(trimmed)
        .into_vec()
        .map_err(|_| anyhow!("invalid base58"))?;
    if decoded.len() != 32 {
        return Err(anyhow!("address must decode to 32 bytes"));
    }
    Ok(())
}
