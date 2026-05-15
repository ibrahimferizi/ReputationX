use anchor_lang::prelude::*;

use crate::SetScore;

pub fn handler(ctx: Context<SetScore>, score: u8) -> Result<()> {
    let wallet_score = &mut ctx.accounts.wallet_score;
    let signer = &ctx.accounts.signer;

    // Initialize the WalletScore account
    wallet_score.wallet = signer.key();
    wallet_score.score = score;
    wallet_score.bumped = 0; // we don't use this for now

    Ok(())
}