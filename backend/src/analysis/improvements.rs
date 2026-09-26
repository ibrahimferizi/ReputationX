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
        if matches!(metric.risk, RiskLevel::Low | RiskLevel::Unknown) {
            continue;
        }

        if let Some(step) = step_for_metric(metric) {
            out.push(step);
        }
    }

    out
}

fn step_for_metric(metric: &MetricAssessment) -> Option<ImprovementStep> {
    let (title, steps) = match metric.id.as_str() {
        "tx_count" | "tx_burst" | "tx_spacing" => ("Review the observed activity", vec![
            "Check the scan's time window and sample size before interpreting this signal.",
            "Automation, trading and service wallets can produce unusual patterns without implying abuse.",
            "This report does not identify who controls the wallet.",
        ]),
        "balance" => ("Interpret the balance in context", vec![
            "This is a snapshot of SOL held at scan time, not a measure of identity or safety.",
            "Other assets and ownership of related wallets are not assessed.",
        ]),
        "token_risk" => ("Review the token finding", vec![
            "Compare the provider results for the exact mint listed in this report.",
            "A token may have been received without the wallet owner's involvement.",
            "Token approvals, purchase intent and liquidity positions are not assessed by this wallet scan.",
        ]),
        _ => return None,
    };
    Some(ImprovementStep {
        metric_id: metric.id.clone(),
        title: title.into(),
        steps: steps.into_iter().map(str::to_string).collect(),
    })
}
