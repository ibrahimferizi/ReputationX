use anchor_lang::prelude::*;

declare_id!("CzFJg6mnbgM29hSzMCL1Bvp1gZdfhi6pBq1a3smVtDhx");

#[program]
pub mod walletguard {
    use super::*;

    /// Initialize a wallet's risk score on-chain
    /// 
    /// This creates a new account that stores:
    /// - wallet: The wallet's public key
    /// - score: The risk score (0-1400)
    /// - tier: The reputation tier (e.g., Mercury, Sun)
    /// - risk_level: 0=Low, 1=Medium, 2=High
    /// - last_updated: Timestamp of last update
    pub fn initialize_wallet(ctx: Context<InitializeWallet>, score: u32, tier: String) -> Result<()> {
        let wallet_data = &mut ctx.accounts.wallet_data;
        let signer = &ctx.accounts.signer;

        wallet_data.wallet = signer.key();
        wallet_data.score = score;
        wallet_data.tier = tier;
        wallet_data.risk_level = calculate_risk_level(&score);
        wallet_data.last_updated = Clock::get()?.unix_timestamp;
        wallet_data.initialized = true;

        msg!("Wallet initialized with score: {}", score);
        msg!("Tier: {}", tier);
        msg!("Risk level: {}", wallet_data.risk_level);

        Ok(())
    }

    /// Update an existing wallet's risk score
    pub fn update_wallet_score(ctx: Context<UpdateWallet>, score: u32, tier: String) -> Result<()> {
        let wallet_data = &mut ctx.accounts.wallet_data;
        
        // Only the wallet owner can update their own score
        require!(wallet_data.wallet == ctx.accounts.signer.key(), WalletGuardError::Unauthorized);

        wallet_data.score = score;
        wallet_data.tier = tier;
        wallet_data.risk_level = calculate_risk_level(&score);
        wallet_data.last_updated = Clock::get()?.unix_timestamp;

        msg!("Wallet score updated: {}", score);

        Ok(())
    }

    /// Get a wallet's risk score (public read-only function)
    pub fn get_wallet_score(ctx: Context<GetWalletScore>) -> Result<()> {
        let wallet_data = &ctx.accounts.wallet_data;
        
        msg!("Wallet: {}", wallet_data.wallet);
        msg!("Score: {}", wallet_data.score);
        msg!("Tier: {}", wallet_data.tier);
        msg!("Risk level: {}", wallet_data.risk_level);
        msg!("Last updated: {}", wallet_data.last_updated);

        Ok(())
    }
}

/// Helper function to calculate risk level from score
/// Lower score = higher risk (newer wallet, less reputation)
/// Higher score = lower risk (established wallet, more reputation)
fn calculate_risk_level(score: &u32) -> u8 {
    if *score < 400 {
        2 // High risk
    } else if *score < 900 {
        1 // Medium risk
    } else {
        0 // Low risk
    }
}

/// Account structure: stores wallet risk data on-chain
#[account]
pub struct WalletData {
    pub wallet: Pubkey,         // The wallet address
    pub score: u32,             // Reputation score (0-1400)
    pub tier: String,           // Reputation tier (Mercury, Sun, etc.)
    pub risk_level: u8,         // 0=Low, 1=Medium, 2=High
    pub last_updated: i64,      // Unix timestamp
    pub initialized: bool,      // Whether this account is initialized
}

/// Initialize wallet account (PDA - Program Derived Address)
#[derive(Accounts)]
pub struct InitializeWallet<'info> {
    #[account(
        init,
        payer = signer,
        space = 8 + WalletData::SIZE,
        seeds = [b"wallet", signer.key().as_ref()],
        bump
    )]
    pub wallet_data: Account<'info, WalletData>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Update wallet account
#[derive(Accounts)]
pub struct UpdateWallet<'info> {
    #[account(
        mut,
        seeds = [b"wallet", wallet_data.wallet.as_ref()],
        bump
    )]
    pub wallet_data: Account<'info, WalletData>,
    #[account(mut)]
    pub signer: Signer<'info>,
}

/// Get wallet score (read-only)
#[derive(Accounts)]
pub struct GetWalletScore<'info> {
    #[account(
        seeds = [b"wallet", wallet_data.wallet.as_ref()],
        bump
    )]
    pub wallet_data: Account<'info, WalletData>,
}

/// Custom errors
#[error_code]
pub enum WalletGuardError {
    #[msg("Unauthorized: only the wallet owner can update their score")]
    Unauthorized,
}

impl WalletData {
    pub const SIZE: usize = 32 + 4 + 64 + 1 + 8 + 1; // wallet + score + tier + risk_level + last_updated + initialized
}