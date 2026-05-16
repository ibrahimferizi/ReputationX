use super::risk::{MetricAssessment, RiskLevel};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImprovementStep {
    pub metric_id: String,
    pub title: String,
    pub steps: Vec<String>,
}

pub fn build_improvement_steps(metrics: &[MetricAssessment]) -> Vec<ImprovementStep> {
    let mut out = Vec::new();

    for metric in metrics {
        if metric.risk == RiskLevel::Low {
            continue;
        }

        if let Some(step) = step_for_metric(metric) {
            out.push(step);
        }
    }

    out
}

fn step_for_metric(metric: &MetricAssessment) -> Option<ImprovementStep> {
    match metric.id.as_str() {
        "wallet_age" => None,
        "tx_count" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Improve transaction reputation".into(),
            steps: vec![
                "Avoid bot-like burst trading; space activity over days or weeks.".into(),
                "Use the wallet for legitimate transfers, payments, and verified dApps.".into(),
                "Maintain consistent but not extreme daily transaction volume.".into(),
            ],
        }),
        "tx_burst" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Reduce burst-style activity".into(),
            steps: vec![
                "Batch operations where possible instead of dozens of txs in one hour.".into(),
                "If using automation, throttle to human-like intervals.".into(),
                "Separate hot trading wallets from long-term identity wallets.".into(),
            ],
        }),
        "tx_spacing" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Normalize activity patterns".into(),
            steps: vec![
                "Prefer organic intervals between on-chain actions.".into(),
                "Avoid alternating extreme idle periods with massive bursts.".into(),
            ],
        }),
        "balance" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Strengthen wallet funding profile".into(),
            steps: vec![
                "Keep a small SOL reserve for fees (0.1+ SOL recommended).".into(),
                "Fund from known CEX or bridge flows rather than anonymous hops only.".into(),
            ],
        }),
        "nft_count" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Build verifiable on-chain identity".into(),
            steps: vec![
                "Hold reputable collection NFTs or domain names tied to your identity.".into(),
                "Participate in established communities with on-chain credentials.".into(),
            ],
        }),
        "token_risk" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Reduce exposure to risky tokens".into(),
            steps: vec![
                "Revoke unused token approvals via a trusted approval manager.".into(),
                "Avoid swapping into tokens flagged as honeypots or high rug risk.".into(),
                "Verify mint authority renounced and liquidity locked before buying.".into(),
            ],
        }),
        "defi_exposure" => Some(ImprovementStep {
            metric_id: metric.id.clone(),
            title: "Improve DeFi footprint".into(),
            steps: vec![
                "Interact with audited protocols (Jupiter, Marinade, major lending markets).".into(),
                "Start with small positions and grow over time.".into(),
            ],
        }),
        _ => None,
    }
}
