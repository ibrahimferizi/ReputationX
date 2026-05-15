use anchor_lang::prelude::*;

#[account]
pub struct WalletScore {
    pub wallet: Pubkey,   // which wallet this score is for
    pub score: u8,        // risk score: 0–100
    pub bumped: u8,       // used by Anchor for AccountInfo
}