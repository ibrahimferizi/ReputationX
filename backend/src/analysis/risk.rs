use crate::integrations::{TokenScanCoverage, TokenScanStatus};
use serde::{Deserialize, Serialize};

// Maximum attainable score: base 200 + age 320 + tx count 150 + balance 40.
// The spacing bonus only applies to younger wallets, whose age bonus is smaller.
pub const MAX_RAW_SCORE: u32 = 710;
pub const SCORING_VERSION: &str = "activity-v3";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricAssessment {
    pub id: String,
    pub label: String,
    pub value: String,
    pub risk: RiskLevel,
    pub summary: String,
}

pub fn assess_wallet_age(days: f64, age_capped: bool, unknown: bool) -> MetricAssessment {
    if unknown {
        return unknown_metric(
            "wallet_age",
            "Wallet Age",
            "Unknown",
            "First activity could not be determined from the available history.",
        );
    }
    if age_capped && days < 30.0 {
        return unknown_metric(
            "wallet_age",
            "Wallet Age",
            &format!(
                "At least {}",
                crate::solana::transactions::format_wallet_age(days)
            ),
            "History is truncated. The wallet may be older than the first activity observed.",
        );
    }
    let capped_note = if unknown {
        " (first activity time unavailable from RPC)"
    } else if age_capped {
        " (lower bound from the available history)"
    } else {
        ""
    };

    let (risk, summary) = if unknown {
        (
            RiskLevel::Medium,
            format!("Could not determine wallet age from on-chain data.{capped_note}"),
        )
    } else if days < 7.0 {
        (
            RiskLevel::High,
            format!("Wallet is very new; limited on-chain history.{capped_note}"),
        )
    } else if days < 30.0 {
        (
            RiskLevel::Medium,
            format!("Wallet is relatively young.{capped_note}"),
        )
    } else {
        (
            RiskLevel::Low,
            format!("Wallet has established on-chain history.{capped_note}"),
        )
    };

    MetricAssessment {
        id: "wallet_age".into(),
        label: "Wallet Age".into(),
        value: if age_capped {
            format!(
                "At least {}",
                crate::solana::transactions::format_wallet_age(days)
            )
        } else {
            crate::solana::transactions::format_wallet_age(days)
        },
        risk,
        summary,
    }
}

/// Versioned heuristic activity rating, scaled to the attainable range; not a safety probability.
pub struct TrustRating {
    pub score: u32,
    pub label: String,
}

pub fn raw_to_trust_rating(raw: u32) -> TrustRating {
    let raw = raw.min(MAX_RAW_SCORE);
    let score = 1 + (raw as u64 * 99 / MAX_RAW_SCORE as u64) as u32;
    let label = match score {
        1..=20 => "Very limited activity",
        21..=40 => "Limited activity",
        41..=60 => "Moderate activity",
        61..=80 => "Established activity",
        _ => "Extensive activity",
    }
    .to_string();
    TrustRating { score, label }
}

pub fn assess_tx_count(count: u64, capped: bool, age_days: f64) -> MetricAssessment {
    let display = if capped {
        format!("{count} sampled")
    } else {
        format!("{count} observed")
    };

    let txs_per_day = if age_days > 0.0 {
        count as f64 / age_days
    } else {
        count as f64
    };

    let (risk, summary) = if count == 0 {
        (RiskLevel::Unknown, "No successful transactions observed in the returned history; this does not establish malicious activity.".into())
    } else if age_days < 14.0 && txs_per_day > 100.0 {
        (
            RiskLevel::High,
            "Very high transaction velocity for a young wallet.".into(),
        )
    } else if age_days < 14.0 && txs_per_day > 30.0 {
        (
            RiskLevel::Medium,
            "Elevated activity for a young wallet.".into(),
        )
    } else if count < 5 {
        (RiskLevel::Medium, "Low overall activity.".into())
    } else {
        (RiskLevel::Low, "No transaction-count flag in the observed history. This is not a verified lifetime total.".into())
    };

    MetricAssessment {
        id: "tx_count".into(),
        label: "Transaction Count".into(),
        value: display,
        risk,
        summary,
    }
}

pub fn assess_burst(max_per_hour: u32, age_days: f64) -> MetricAssessment {
    let (risk, summary) = if age_days < 14.0 && max_per_hour >= 20 {
        (
            RiskLevel::High,
            format!("Burst detected: up to {max_per_hour} txs in one hour."),
        )
    } else if age_days < 14.0 && max_per_hour >= 10 {
        (
            RiskLevel::Medium,
            format!("Moderate burst: {max_per_hour} txs/hour peak."),
        )
    } else {
        (
            RiskLevel::Low,
            "No burst flag under the current age-based rule in the observed sample.".into(),
        )
    };

    MetricAssessment {
        id: "tx_burst".into(),
        label: "Transaction Burst Pattern".into(),
        value: format!("{max_per_hour} txs / hour peak"),
        risk,
        summary,
    }
}

pub fn assess_spacing(cv: f64, age_days: f64, burst_high: bool) -> MetricAssessment {
    let (risk, summary) = if age_days < 30.0 && burst_high {
        (
            RiskLevel::High,
            "Rapid clustering dominates spacing pattern.".into(),
        )
    } else if age_days < 30.0 && cv <= 0.45 {
        (
            RiskLevel::Low,
            "Even spacing in the sample; spacing alone does not establish whether activity is automated.".into(),
        )
    } else if cv > 1.0 {
        (
            RiskLevel::Low,
            "Irregular spacing in the sample; this does not establish human ownership.".into(),
        )
    } else {
        (RiskLevel::Medium, "Mixed spacing pattern.".into())
    };

    MetricAssessment {
        id: "tx_spacing".into(),
        label: "Transaction Spacing".into(),
        value: format!("CV {cv:.2}"),
        risk,
        summary,
    }
}

pub fn assess_balance(sol: f64) -> MetricAssessment {
    let (risk, summary) = if sol < 0.01 {
        (RiskLevel::Medium, "Very low SOL balance.".into())
    } else if sol < 0.1 {
        (
            RiskLevel::Medium,
            "Low SOL balance for fees and activity.".into(),
        )
    } else {
        (RiskLevel::Low, "SOL balance is adequate.".into())
    };

    MetricAssessment {
        id: "balance".into(),
        label: "SOL Balance".into(),
        value: format!("{sol:.4} SOL"),
        risk,
        summary,
    }
}

pub fn unknown_metric(id: &str, label: &str, value: &str, summary: &str) -> MetricAssessment {
    MetricAssessment {
        id: id.into(),
        label: label.into(),
        value: value.into(),
        risk: RiskLevel::Unknown,
        summary: summary.into(),
    }
}

pub fn assess_nfts(count: usize) -> MetricAssessment {
    unknown_metric("nft_count", "NFT-like Holdings (est.)", &count.to_string(),
        "Counts legacy SPL holdings with zero decimals and one token. Collection identity is unverified; Token-2022 and compressed NFTs are not covered.")
}

pub fn assess_token_risk(
    high_risk: usize,
    honeypot: bool,
    coverage: &TokenScanCoverage,
) -> MetricAssessment {
    let (risk, value, summary) = if honeypot || high_risk > 0 {
        (RiskLevel::High, if honeypot { "Honeypot flagged".into() } else { format!("{high_risk} token(s) flagged") },
            "A token provider reported a risk signal. Receiving a token does not establish malicious wallet ownership.".into())
    } else if coverage.status == TokenScanStatus::Complete {
        (RiskLevel::Low, "No flags detected".into(), "Enabled providers returned usable results for all detected held legacy SPL mints. This is not a safety guarantee.".into())
    } else {
        let value = match coverage.status {
            TokenScanStatus::Skipped => "Not checked",
            TokenScanStatus::NoHoldings => "No held SPL tokens",
            TokenScanStatus::TimedOut => "Scan timed out",
            TokenScanStatus::Unavailable => "Unavailable",
            _ => "Partial coverage",
        };
        (RiskLevel::Unknown, value.into(), format!("{} of {} detected held mints have a usable provider result. Unchecked holdings cannot be assessed.", coverage.checked_mints, coverage.held_mints))
    };
    MetricAssessment {
        id: "token_risk".into(),
        label: "Token / Honeypot Risk".into(),
        value,
        risk,
        summary,
    }
}

pub fn assess_defi() -> MetricAssessment {
    unknown_metric("defi_exposure", "DeFi Interactions", "Not assessed",
        "Transaction instructions are not analyzed for DeFi activity. Interaction count and USD exposure are unavailable.")
}

pub fn compute_reputation_score(
    age_days: f64,
    tx_count: u64,
    max_per_hour: u32,
    cv: Option<f64>,
    sol: f64,
) -> u32 {
    let mut score: i32 = 200;

    if age_days >= 365.0 {
        score += 320;
    } else if age_days >= 90.0 {
        score += 200;
    } else if age_days >= 30.0 {
        score += 120;
    } else if age_days >= 7.0 {
        score += 60;
    } else if age_days > 0.0 {
        score += 20;
    } else {
        score -= 100;
    }

    if tx_count >= 500 {
        score += 150;
    } else if tx_count >= 100 {
        score += 100;
    } else if tx_count >= 10 {
        score += 40;
    } else if tx_count == 0 {
        score -= 80;
    }

    if age_days < 14.0 && max_per_hour >= 15 {
        score -= 180;
    } else if age_days < 14.0 && max_per_hour >= 8 {
        score -= 80;
    }

    if age_days < 30.0
        && tx_count >= 3
        && cv.is_some_and(|value| value <= 0.45)
        && max_per_hour < 10
    {
        score += 70;
    }

    if sol >= 1.0 {
        score += 40;
    } else if sol >= 0.1 {
        score += 15;
    }

    score.clamp(0, MAX_RAW_SCORE as i32) as u32
}

#[cfg(test)]
mod trust_tests {
    use super::*;

    #[test]
    fn trust_score_is_1_to_100() {
        assert_eq!(raw_to_trust_rating(0).score, 1);
        assert_eq!(raw_to_trust_rating(1400).score, 100);
        assert_eq!(raw_to_trust_rating(355).score, 50);
    }

    #[test]
    fn no_planet_tiers() {
        let label = raw_to_trust_rating(1200).label;
        assert!(!label.contains("Mercury"));
        assert!(!label.contains("Sun"));
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn score_range_is_attainable_with_activity_bands() {
        let best = compute_reputation_score(365.0, 500, 1, Some(1.0), 1.0);
        assert_eq!(best, MAX_RAW_SCORE);
        assert_eq!(raw_to_trust_rating(best).score, 100);
        assert_eq!(raw_to_trust_rating(best).label, "Extensive activity");
        let established_wallet = compute_reputation_score(1600.0, 100, 21, Some(1.27), 1.0);
        assert_eq!(raw_to_trust_rating(established_wallet).score, 93);

        assert_eq!(
            raw_to_trust_rating(compute_reputation_score(0.0, 0, 0, None, 0.0)).score,
            3
        );
    }

    #[test]
    fn score_is_bounded_and_missing_spacing_cannot_earn_a_bonus() {
        for age in [0.0, 1.0, 7.0, 14.0, 30.0, 90.0, 365.0, 3000.0] {
            for count in [0, 1, 3, 10, 100, 500, 10000] {
                for cv in [None, Some(0.0), Some(1.27)] {
                    let raw = compute_reputation_score(age, count, 0, cv, 100.0);
                    assert!(raw <= MAX_RAW_SCORE);
                    assert!((1..=100).contains(&raw_to_trust_rating(raw).score));
                }
            }
        }
        assert_eq!(
            compute_reputation_score(1.0, 0, 0, None, 0.0),
            compute_reputation_score(1.0, 0, 0, Some(0.0), 0.0)
        );
    }

    #[test]
    fn missing_or_partial_token_checks_are_never_clear() {
        for status in [
            TokenScanStatus::Skipped,
            TokenScanStatus::NoHoldings,
            TokenScanStatus::TimedOut,
            TokenScanStatus::Unavailable,
            TokenScanStatus::Partial,
        ] {
            let coverage = TokenScanCoverage {
                status,
                ..Default::default()
            };
            assert_eq!(
                assess_token_risk(0, false, &coverage).risk,
                RiskLevel::Unknown
            );
            assert_eq!(assess_token_risk(1, true, &coverage).risk, RiskLevel::High);
        }
        let coverage = TokenScanCoverage {
            status: TokenScanStatus::Complete,
            ..Default::default()
        };
        assert_eq!(
            assess_token_risk(0, false, &coverage).value,
            "No flags detected"
        );
    }

    #[test]
    fn unmeasured_metrics_and_missing_history_stay_unknown() {
        assert_eq!(assess_defi().value, "Not assessed");
        assert_eq!(assess_defi().risk, RiskLevel::Unknown);
        assert_eq!(assess_nfts(67).risk, RiskLevel::Unknown);
        assert_eq!(assess_tx_count(0, false, 0.0).risk, RiskLevel::Unknown);
        assert_eq!(assess_wallet_age(0.0, false, true).value, "Unknown");
        assert_eq!(assess_wallet_age(2.0, true, false).risk, RiskLevel::Unknown);
        assert_eq!(assess_tx_count(100, true, 1600.0).value, "100 sampled");
    }
}
