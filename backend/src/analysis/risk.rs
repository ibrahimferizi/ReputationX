use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
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
    let capped_note = if unknown {
        " (first activity time unavailable from RPC)"
    } else if age_capped {
        " (age lower bound — raise MAX_SIGNATURE_PAGES or use SCAN_MODE=deep with a dedicated history RPC)"
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
        value: crate::solana::transactions::format_wallet_age(days),
        risk,
        summary,
    }
}

/// WalletGuard trust rating: maps internal raw score (0–1400) to public 1–100 + branded label.
pub struct TrustRating {
    pub score: u32,
    pub label: String,
}

pub fn raw_to_trust_rating(raw: u32) -> TrustRating {
    let raw = raw.min(1400);
    let score = 1 + (raw as u64 * 99 / 1400) as u32;
    let label = match score {
        1..=20 => "Unverified",
        21..=40 => "Caution",
        41..=60 => "Fair",
        61..=80 => "Trusted",
        81..=94 => "Strong",
        _ => "Guardian",
    }
    .to_string();
    TrustRating { score, label }
}

#[cfg(test)]
mod trust_tests {
    use super::raw_to_trust_rating;

    #[test]
    fn trust_score_is_1_to_100() {
        assert_eq!(raw_to_trust_rating(0).score, 1);
        assert_eq!(raw_to_trust_rating(1400).score, 100);
        assert_eq!(raw_to_trust_rating(700).score, 51);
    }

    #[test]
    fn no_planet_tiers() {
        let label = raw_to_trust_rating(1200).label;
        assert!(!label.contains("Mercury"));
        assert!(!label.contains("Sun"));
    }
}

pub fn assess_tx_count(count: u64, capped: bool, age_days: f64) -> MetricAssessment {
    let display = if capped {
        format!("{count}+ (scan capped)")
    } else {
        count.to_string()
    };

    let txs_per_day = if age_days > 0.0 {
        count as f64 / age_days
    } else {
        count as f64
    };

    let (risk, summary) = if count == 0 {
        (
            RiskLevel::High,
            "No successful transactions found.".into(),
        )
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
        (
            RiskLevel::Medium,
            "Low overall activity.".into(),
        )
    } else {
        (
            RiskLevel::Low,
            "Transaction volume looks typical.".into(),
        )
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
            "No concerning transaction bursts.".into(),
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
            "Evenly spaced activity (less bot-like than hard bursts).".into(),
        )
    } else if cv > 1.0 {
        (
            RiskLevel::Low,
            "Organic irregular spacing between transactions.".into(),
        )
    } else {
        (
            RiskLevel::Medium,
            "Mixed spacing pattern.".into(),
        )
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
        (
            RiskLevel::Medium,
            "Very low SOL balance.".into(),
        )
    } else if sol < 0.1 {
        (
            RiskLevel::Medium,
            "Low SOL balance for fees and activity.".into(),
        )
    } else {
        (
            RiskLevel::Low,
            "SOL balance is adequate.".into(),
        )
    };

    MetricAssessment {
        id: "balance".into(),
        label: "SOL Balance".into(),
        value: format!("{sol:.4} SOL"),
        risk,
        summary,
    }
}

pub fn assess_nfts(count: usize) -> MetricAssessment {
    let (risk, summary) = if count == 0 {
        (
            RiskLevel::Medium,
            "No NFT-like token holdings detected.".into(),
        )
    } else {
        (
            RiskLevel::Low,
            "NFT-like holdings present.".into(),
        )
    };

    MetricAssessment {
        id: "nft_count".into(),
        label: "NFT Holdings (est.)".into(),
        value: count.to_string(),
        risk,
        summary,
    }
}

pub fn assess_token_risk(high_risk: usize, honeypot: bool) -> MetricAssessment {
    let (risk, summary) = if honeypot {
        (
            RiskLevel::High,
            "Honeypot or sell-blocked token exposure detected.".into(),
        )
    } else if high_risk > 0 {
        (
            RiskLevel::High,
            format!("{high_risk} scanned token(s) flagged high risk."),
        )
    } else {
        (
            RiskLevel::Low,
            "No high-risk tokens in scanned holdings.".into(),
        )
    };

    MetricAssessment {
        id: "token_risk".into(),
        label: "Token / Honeypot Risk".into(),
        value: if honeypot {
            "Honeypot flagged".into()
        } else if high_risk > 0 {
            format!("{high_risk} token(s) need review")
        } else {
            "Clear".into()
        },
        risk,
        summary,
    }
}

pub fn assess_defi(interactions: usize) -> MetricAssessment {
    let (risk, summary) = if interactions == 0 {
        (
            RiskLevel::Medium,
            "No DeFi program interactions detected in recent history.".into(),
        )
    } else if interactions > 50 {
        (
            RiskLevel::Medium,
            "Heavy DeFi interaction footprint.".into(),
        )
    } else {
        (
            RiskLevel::Low,
            "Moderate DeFi interaction footprint.".into(),
        )
    };

    MetricAssessment {
        id: "defi_exposure".into(),
        label: "DeFi Interactions (est.)".into(),
        value: interactions.to_string(),
        risk,
        summary,
    }
}

pub fn compute_reputation_score(
    age_days: f64,
    tx_count: u64,
    max_per_hour: u32,
    cv: f64,
    sol: f64,
    honeypot: bool,
    high_risk_tokens: usize,
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

    if age_days < 30.0 && cv <= 0.45 && max_per_hour < 10 {
        score += 70;
    }

    if sol >= 1.0 {
        score += 40;
    } else if sol >= 0.1 {
        score += 15;
    }

    if honeypot {
        score -= 250;
    }
    score -= (high_risk_tokens as i32) * 40;

    score.clamp(0, 1400) as u32
}
