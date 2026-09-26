use crate::analysis::improvements::build_improvement_steps;
use crate::analysis::risk::{
    assess_balance, assess_burst, assess_defi, assess_nfts, assess_spacing, assess_token_risk,
    assess_tx_count, assess_wallet_age, compute_reputation_score, raw_to_trust_rating,
    unknown_metric, MetricAssessment, SCORING_VERSION,
};
use crate::config::Config;
use crate::integrations::{scan_wallet_tokens, ExternalScans};
use crate::models::response::{
    DefiExposure, LegacyReputationResponse, MetricDto, ReputationResponse, ScanCoverage,
    TokenRiskDto, TxStats, WalletAge,
};
use crate::solana::funding::get_funding_source;
use crate::solana::rpc::SolanaRpc;
use crate::solana::transactions::get_transactions;
use crate::solana::wallet::{get_balance, get_token_holdings};
use anyhow::Result;
use reqwest::Client;

pub async fn build_wallet_reputation(
    http: &Client,
    rpc: &SolanaRpc,
    history_rpc: &SolanaRpc,
    config: &Config,
    address: &str,
) -> Result<ReputationResponse> {
    // Core on-chain data in parallel (typically 2–4s in fast mode).
    let (balance, history, holdings, funding_source) = tokio::join!(
        get_balance(rpc, address),
        get_transactions(rpc, history_rpc, http, config, address),
        get_token_holdings(rpc, address),
        get_funding_source(address, history_rpc, config),
    );

    let (balance, history, holdings) = (balance?, history?, holdings?);

    let external = if config.skip_external_scans || holdings.mints.is_empty() {
        ExternalScans::default()
    } else {
        scan_wallet_tokens(http, config, &holdings.mints).await
    };
    let token_coverage = external.coverage(
        holdings.mints.len(),
        config.max_token_scans,
        config.skip_external_scans,
        config.solsniffer_api_key.is_some(),
    );
    let sampled = history.count_capped || history.scan_source == "helius";
    let spacing_cv = (history.timestamps_sec.len() >= 3).then_some(history.interval_cv);
    let burst_high = history.max_txs_per_hour >= 15 && history.wallet_age_days < 14.0;

    let metrics: Vec<MetricAssessment> = vec![
        assess_wallet_age(
            history.wallet_age_days,
            history.age_capped,
            history.first_activity_unix.is_none(),
        ),
        assess_tx_count(history.total_count, sampled, history.wallet_age_days),
        if history.timestamps_sec.len() < 2
            || history.first_activity_unix.is_none()
            || history.age_capped
        {
            unknown_metric("tx_burst", "Transaction Burst Pattern", "Insufficient data", "The age-based burst rule needs resolved wallet age and at least two timed transactions.")
        } else {
            assess_burst(history.max_txs_per_hour, history.wallet_age_days)
        },
        if let Some(cv) = spacing_cv {
            assess_spacing(cv, history.wallet_age_days, burst_high)
        } else {
            unknown_metric("tx_spacing", "Transaction Spacing", "Insufficient data", "At least three successful transactions with timestamps are needed to assess spacing.")
        },
        assess_balance(balance.sol),
        assess_nfts(holdings.estimated_nft_count),
        assess_token_risk(
            external.high_risk_tokens,
            external.honeypot_detected,
            &token_coverage,
        ),
        assess_defi(),
    ];

    let reputation_score = compute_reputation_score(
        history.wallet_age_days,
        history.total_count,
        history.max_txs_per_hour,
        spacing_cv,
        balance.sol,
    );

    let trust = raw_to_trust_rating(reputation_score);
    let improvement_steps = build_improvement_steps(&metrics);

    let legacy = LegacyReputationResponse {
        reputation_score: trust.score,
        tier: trust.label.clone(),
        trust_label: trust.label.clone(),
        balance: crate::models::response::Balance { sol: balance.sol },
        tx_stats: TxStats {
            count: history.total_count,
            capped: history.count_capped,
            sampled,
            max_per_hour: history.max_txs_per_hour,
        },
        wallet_age: WalletAge {
            days: history.wallet_age_days.floor() as u64,
            days_precise: history.wallet_age_days,
            first_activity_unix: history.first_activity_unix,
            age_capped: history.age_capped,
        },
        nft_stats: crate::models::response::NftStats {
            count: holdings.estimated_nft_count as u64,
        },
        defi_exposure: DefiExposure {
            total_usd: None,
            interaction_count: None,
        },
    };

    Ok(ReputationResponse {
        address: address.to_string(),
        generated_at: None,
        legacy,
        metrics: metrics.into_iter().map(MetricDto::from).collect(),
        improvement_steps,
        token_risks: map_token_risks(&external),
        integrations: integration_status(
            config,
            &history.scan_source,
            &history.age_source,
            &external,
        ),
        funding_source,
        api_version: "walletguard-v2".into(),
        scoring_version: SCORING_VERSION.into(),
        coverage: ScanCoverage {
            history_source: history.scan_source.clone(),
            timed_transactions: history.timestamps_sec.len(),
            sample_start_unix: history.timestamps_sec.first().copied(),
            sample_end_unix: history.timestamps_sec.last().copied(),
            token_scans: token_coverage,
        },
    })
}

fn map_token_risks(external: &ExternalScans) -> Vec<TokenRiskDto> {
    external
        .rugcheck
        .iter()
        .map(|rug| {
            let sniff = external
                .solsniffer
                .iter()
                .find(|scan| scan.mint == rug.mint);
            TokenRiskDto {
                mint: rug.mint.clone(),
                rugcheck_score: rug.score,
                solsniffer_score: sniff.and_then(|scan| scan.snifscore),
                solsniffer_checked_at: sniff.and_then(|scan| scan.checked_at.clone()),
                honeypot: rug.is_honeypot,
                flags: rug.flags.clone(),
                available: rug.available || sniff.is_some_and(|scan| scan.available),
            }
        })
        .collect()
}

fn integration_status(
    config: &Config,
    tx_scan_source: &str,
    age_source: &str,
    external: &ExternalScans,
) -> serde_json::Value {
    serde_json::json!({
        "scan_mode": format!("{:?}", config.scan_mode).to_ascii_lowercase(),
        "max_signature_pages": config.max_signature_pages,
        "max_age_signature_pages": config.max_age_signature_pages,
        "history_rpc_gtfa": crate::solana::helius::gtfa_rpc_url(config).is_some(),
        "age_source": age_source,
        "signature_page_size": config.signature_page_size,
        "tx_scan_source": tx_scan_source,
        "external_scans_skipped": config.skip_external_scans,
        "rugcheck": external.rugcheck.iter().any(|r| r.available),
        "solsniffer": external.solsniffer.iter().any(|r| r.available),
        "helius_configured": config.helius_api_key.is_some(),
    })
}
