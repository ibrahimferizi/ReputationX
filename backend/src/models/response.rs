use crate::analysis::risk::{MetricAssessment, RiskLevel};
use crate::solana::FundingSource;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Balance {
    pub sol: f64,
}

#[derive(Debug, Serialize)]
pub struct TxStats {
    pub count: u64,
    pub capped: bool,
    pub max_per_hour: u32,
}

#[derive(Debug, Serialize)]
pub struct WalletAge {
    pub days: u64,
    pub days_precise: f64,
    pub first_activity_unix: Option<i64>,
    /// True when signature pagination hit MAX_SIGNATURE_PAGES before reaching genesis.
    pub age_capped: bool,
}

#[derive(Debug, Serialize)]
pub struct NftStats {
    pub count: u64,
}

#[derive(Debug, Serialize)]
pub struct DefiExposure {
    pub total_usd: f64,
    pub interaction_count: usize,
}

/// Fields the existing React UI already reads.
#[derive(Debug, Serialize)]
pub struct LegacyReputationResponse {
    /// WalletGuard trust score (1–100).
    pub reputation_score: u32,
    /// Branded trust label (e.g. Verified, Guardian).
    pub tier: String,
    pub trust_label: String,
    pub balance: Balance,
    pub tx_stats: TxStats,
    pub wallet_age: WalletAge,
    pub nft_stats: NftStats,
    pub defi_exposure: DefiExposure,
}

#[derive(Debug, Serialize)]
pub struct MetricDto {
    pub id: String,
    pub label: String,
    pub value: String,
    pub risk: RiskLevel,
    pub summary: String,
}

impl From<MetricAssessment> for MetricDto {
    fn from(m: MetricAssessment) -> Self {
        Self {
            id: m.id,
            label: m.label,
            value: m.value,
            risk: m.risk,
            summary: m.summary,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ImprovementStepDto {
    pub metric_id: String,
    pub title: String,
    pub steps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenRiskDto {
    pub mint: String,
    pub rugcheck_score: Option<i64>,
    pub honeypot: bool,
    pub flags: Vec<String>,
    pub available: bool,
}

#[derive(Debug, Serialize)]
pub struct ReputationResponse {
    pub address: String,
    #[serde(flatten)]
    pub legacy: LegacyReputationResponse,
    pub metrics: Vec<MetricDto>,
    pub improvement_steps: Vec<crate::analysis::improvements::ImprovementStep>,
    pub token_risks: Vec<TokenRiskDto>,
    pub integrations: serde_json::Value,
    pub funding_source: FundingSource,
    pub api_version: String,
}
