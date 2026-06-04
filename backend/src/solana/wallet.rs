use crate::solana::rpc::SolanaRpc;
use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct WalletBalance {
    pub lamports: u64,
    pub sol: f64,
}

pub async fn get_balance(rpc: &SolanaRpc, address: &str) -> Result<WalletBalance> {
    let lamports: u64 = rpc
        .call("getBalance", json!([address, { "commitment": "confirmed" }]))
        .await?;
    Ok(WalletBalance {
        lamports,
        sol: lamports as f64 / 1_000_000_000.0,
    })
}

#[derive(Debug, Clone)]
pub struct TokenHoldings {
    pub spl_token_accounts: usize,
    pub estimated_nft_count: usize,
    pub mints: Vec<String>,
}

pub async fn get_token_holdings(rpc: &SolanaRpc, address: &str) -> Result<TokenHoldings> {
    const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

    let response: TokenAccountsResponse = rpc
        .call(
            "getTokenAccountsByOwner",
            json!([
                address,
                { "programId": TOKEN_PROGRAM },
                { "encoding": "jsonParsed" }
            ]),
        )
        .await?;

    let mut mints_set = HashSet::new();
    let mut estimated_nft_count = 0usize;
    let spl_token_accounts = response.value.len();

    for account in response.value {
        let Some(parsed) = account.account.data.parsed else {
            continue;
        };
        let info = parsed.info;
        mints_set.insert(info.mint.clone());

        let amount = info
            .tokenAmount
            .uiAmount
            .unwrap_or(0.0);
        let decimals = info.tokenAmount.decimals;

        // Common NFT heuristic: 0 decimals and amount == 1
        if decimals == 0 && (amount - 1.0).abs() < f64::EPSILON {
            estimated_nft_count += 1;
        }
    }

    let mints = mints_set.into_iter().collect::<Vec<_>>();

    Ok(TokenHoldings {
        spl_token_accounts,
        estimated_nft_count,
        mints,
    })
}

#[derive(Debug, Deserialize)]
struct TokenAccountsResponse {
    value: Vec<TokenAccountValue>,
}

#[derive(Debug, Deserialize)]
struct TokenAccountValue {
    account: TokenAccountData,
}

#[derive(Debug, Deserialize)]
struct TokenAccountData {
    data: TokenParsedData,
}

#[derive(Debug, Deserialize)]
struct TokenParsedData {
    parsed: Option<TokenParsedInfo>,
}

#[derive(Debug, Deserialize)]
struct TokenParsedInfo {
    info: TokenInfo,
}

#[derive(Debug, Deserialize)]
struct TokenInfo {
    mint: String,
    tokenAmount: TokenAmount,
}

#[derive(Debug, Deserialize)]
struct TokenAmount {
    decimals: u8,
    uiAmount: Option<f64>,
}
