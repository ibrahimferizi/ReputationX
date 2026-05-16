use crate::analysis::improvements::build_improvement_steps;
use crate::analysis::risk::{
    assess_balance, assess_burst, assess_defi, assess_nfts, assess_spacing, assess_token_risk,
    assess_tx_count, assess_wallet_age, compute_reputation_score, score_to_tier, MetricAssessment,
};
use crate::config::Config;
use crate::integrations::{scan_wallet_tokens, ExternalScans};
use crate::models::response::{
    DefiExposure, LegacyReputationResponse, MetricDto, ReputationResponse, TokenRiskDto,
    TxStats, WalletAge,
};
use crate::solana::rpc::SolanaRpc;
use crate::solana::transactions::get_transactions;
use crate::solana::wallet::{get_balance, get_token_holdings};
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

const KNOWN_DEFI_PROGRAMS: &[&str] = &[
    "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4",
    "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8",
    "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrGrhN",
];

pub async fn build_wallet_reputation(
    http: &Client,
    rpc: &SolanaRpc,
    config: &Config,
    address: &str,
) -> Result<ReputationResponse> {
    // Core on-chain data in parallel (typically 2–4s in fast mode).
    let (balance, history, holdings) = tokio::try_join!(
        get_balance(rpc, address),
        get_transactions(rpc, http, config, address),
        get_token_holdings(rpc, address)
    )?;

    let external = if config.skip_external_scans || holdings.mints.is_empty() {
        ExternalScans {
            rugcheck: vec![],
            solsniffer: vec![],
            honeypot_detected: false,
            high_risk_tokens: 0,
        }
    } else {
        tokio::time::timeout(
            Duration::from_secs(8),
            scan_wallet_tokens(http, config, &holdings.mints),
        )
        .await
        .unwrap_or(ExternalScans {
            rugcheck: vec![],
            solsniffer: vec![],
            honeypot_detected: false,
            high_risk_tokens: 0,
        })
    };

    let defi_interactions = estimate_defi_interactions(&history.timestamps_sec);
    let burst_high = history.max_txs_per_hour >= 15 && history.wallet_age_days < 14.0;

    let metrics: Vec<MetricAssessment> = vec![
        assess_wallet_age(history.wallet_age_days, history.count_capped),
        assess_tx_count(
            history.total_count,
            history.count_capped,
            history.wallet_age_days,
        ),
        assess_burst(history.max_txs_per_hour, history.wallet_age_days),
        assess_spacing(history.interval_cv, history.wallet_age_days, burst_high),
        assess_balance(balance.sol),
        assess_nfts(holdings.estimated_nft_count),
        assess_token_risk(external.high_risk_tokens, external.honeypot_detected),
        assess_defi(defi_interactions),
    ];

    let reputation_score = compute_reputation_score(
        history.wallet_age_days,
        history.total_count,
        history.max_txs_per_hour,
        history.interval_cv,
        balance.sol,
        external.honeypot_detected,
        external.high_risk_tokens,
    );

    let (reputation_score, tier) = score_to_tier(reputation_score);
    let improvement_steps = build_improvement_steps(&metrics);

    let legacy = LegacyReputationResponse {
        reputation_score,
        tier: tier.clone(),
        celestila_tier: tier.clone(),
        balance: crate::models::response::Balance { sol: balance.sol },
        tx_stats: TxStats {
            count: history.total_count,
            capped: history.count_capped,
            max_per_hour: history.max_txs_per_hour,
        },
        wallet_age: WalletAge {
            days: history.wallet_age_days.floor() as u64,
            days_precise: history.wallet_age_days,
            first_activity_unix: history.first_activity_unix,
        },
        nft_stats: crate::models::response::NftStats {
            count: holdings.estimated_nft_count as u64,
        },
        defi_exposure: DefiExposure {
            total_usd: 0.0,
            interaction_count: defi_interactions,
        },
    };

    Ok(ReputationResponse {
        address: address.to_string(),
        legacy,
        metrics: metrics.into_iter().map(MetricDto::from).collect(),
        improvement_steps,
        token_risks: map_token_risks(&external),
        integrations: integration_status(config, &history.scan_source, &external),
        api_version: "walletguard-v1".into(),
    })
}

fn estimate_defi_interactions(timestamps: &[i64]) -> usize {
    if timestamps.is_empty() {
        return 0;
    }
    (timestamps.len() / 50).min(200)
}

fn map_token_risks(external: &ExternalScans) -> Vec<TokenRiskDto> {
    external
        .rugcheck
        .iter()
        .map(|r| TokenRiskDto {
            mint: r.mint.clone(),
            rugcheck_score: r.score,
            honeypot: r.is_honeypot,
            flags: r.flags.clone(),
            available: r.available,
        })
        .collect()
}

fn integration_status(
    config: &Config,
    scan_source: &str,
    external: &ExternalScans,
) -> serde_json::Value {
    serde_json::json!({
        "scan_mode": format!("{:?}", config.scan_mode).to_ascii_lowercase(),
        "tx_scan_source": scan_source,
        "external_scans_skipped": config.skip_external_scans,
        "rugcheck": external.rugcheck.iter().any(|r| r.available),
        "solsniffer": external.solsniffer.iter().any(|r| r.available),
        "helius_configured": config.helius_api_key.is_some(),
    })
}
