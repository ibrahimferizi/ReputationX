use crate::models::response::ReputationResponse;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const SECONDS_PER_DAY: i64 = 86_400;
const CREATION_WINDOW_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize)]
pub struct WalletCluster {
    pub cluster_id: String,
    pub funding_address: String,
    pub members: Vec<String>,
    pub suspicion: String,
    pub reasons: Vec<String>,
}

/// Group wallets that share the same `funding_source.source_address`.
pub fn build_clusters(results: &[(String, ReputationResponse)]) -> Vec<WalletCluster> {
    let mut by_funder: HashMap<String, Vec<String>> = HashMap::new();
    let report_by_address: HashMap<&str, &ReputationResponse> = results
        .iter()
        .map(|(addr, report)| (addr.as_str(), report))
        .collect();

    for (address, report) in results {
        if let Some(funder) = report.funding_source.source_address.as_deref() {
            by_funder
                .entry(funder.to_string())
                .or_default()
                .push(address.clone());
        }
    }

    let mut clusters = Vec::new();

    for (funding_address, mut members) in by_funder {
        if members.len() < 2 {
            continue;
        }

        members.sort_unstable();

        let size = members.len();
        let mut reasons = vec![format!("{size} wallets share the same funding source")];

        let member_reports: Vec<&ReputationResponse> = members
            .iter()
            .filter_map(|m| report_by_address.get(m.as_str()).copied())
            .collect();

        let mut suspicion = base_suspicion(size).to_string();

        if wallets_in_same_7_day_window(&member_reports) {
            reasons.push("wallets created within same 7-day window".into());
            suspicion = bump_suspicion(&suspicion);
        }

        clusters.push(WalletCluster {
            cluster_id: cluster_id_for(&funding_address),
            funding_address,
            members,
            suspicion,
            reasons,
        });
    }

    clusters.sort_by(|a, b| b.members.len().cmp(&a.members.len()));
    clusters
}

fn cluster_id_for(funding_address: &str) -> String {
    let digest = Sha256::digest(funding_address.as_bytes());
    let hex = format!("{:x}", digest);
    hex.chars().take(8).collect()
}

fn base_suspicion(size: usize) -> &'static str {
    if size >= 10 {
        "high"
    } else if size >= 3 {
        "medium"
    } else {
        "low"
    }
}

fn bump_suspicion(current: &str) -> String {
    match current {
        "low" => "medium".into(),
        _ => "high".into(),
    }
}

fn wallets_in_same_7_day_window(reports: &[&ReputationResponse]) -> bool {
    let timestamps: Vec<i64> = reports
        .iter()
        .filter_map(|r| r.legacy.wallet_age.first_activity_unix)
        .collect();

    if timestamps.len() < 2 {
        return false;
    }

    let min_ts = timestamps.iter().min().copied().unwrap();
    let max_ts = timestamps.iter().max().copied().unwrap();
    max_ts - min_ts <= CREATION_WINDOW_DAYS * SECONDS_PER_DAY
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::response::{
        Balance, DefiExposure, LegacyReputationResponse, NftStats, TxStats, WalletAge,
    };
    use crate::solana::FundingSource;

    fn sample_report(address: &str, funder: Option<&str>, first_unix: Option<i64>) -> ReputationResponse {
        ReputationResponse {
            address: address.into(),
            legacy: LegacyReputationResponse {
                reputation_score: 50,
                tier: "Fair".into(),
                trust_label: "Fair".into(),
                balance: Balance { sol: 0.0 },
                tx_stats: TxStats {
                    count: 0,
                    capped: false,
                    max_per_hour: 0,
                },
                wallet_age: WalletAge {
                    days: 30,
                    days_precise: 30.0,
                    first_activity_unix: first_unix,
                    age_capped: false,
                },
                nft_stats: NftStats { count: 0 },
                defi_exposure: DefiExposure {
                    total_usd: 0.0,
                    interaction_count: 0,
                },
            },
            metrics: vec![],
            improvement_steps: vec![],
            token_risks: vec![],
            integrations: serde_json::json!({}),
            funding_source: FundingSource {
                source_type: if funder.is_some() {
                    "wallet".into()
                } else {
                    "unknown".into()
                },
                source_address: funder.map(str::to_string),
                confidence: "medium".into(),
            },
            api_version: "walletguard-v1".into(),
        }
    }

    #[test]
    fn groups_shared_funder() {
        let funder = "Funder1111111111111111111111111111111111111";
        let results = vec![
            ("a".into(), sample_report("a", Some(funder), Some(1_000))),
            ("b".into(), sample_report("b", Some(funder), Some(1_100))),
        ];
        let clusters = build_clusters(&results);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members.len(), 2);
        assert_eq!(clusters[0].suspicion, "low");
    }

    #[test]
    fn high_suspicion_for_large_cluster() {
        let funder = "Funder2222222222222222222222222222222222222";
        let results: Vec<_> = (0..10)
            .map(|i| {
                let addr = format!("wallet{i}");
                (addr.clone(), sample_report(&addr, Some(funder), Some(1_000)))
            })
            .collect();
        let clusters = build_clusters(&results);
        assert_eq!(clusters[0].suspicion, "high");
    }

    #[test]
    fn bumps_suspicion_for_creation_window() {
        let funder = "Funder3333333333333333333333333333333333333";
        let base = 1_700_000_000_i64;
        let results = vec![
            ("a".into(), sample_report("a", Some(funder), Some(base))),
            ("b".into(), sample_report("b", Some(funder), Some(base + 86_400))),
            ("c".into(), sample_report("c", Some(funder), Some(base + 2 * 86_400))),
        ];
        let clusters = build_clusters(&results);
        assert_eq!(clusters[0].suspicion, "medium");
        assert!(clusters[0]
            .reasons
            .iter()
            .any(|r| r.contains("7-day")));
    }
}
