use crate::analysis::risk::{MetricAssessment, RiskLevel};
use crate::solana::FundingSource;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Balance {
    pub sol: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TxStats {
    pub count: u64,
    pub capped: bool,
    pub sampled: bool,
    pub max_per_hour: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletAge {
    pub days: u64,
    pub days_precise: f64,
    pub first_activity_unix: Option<i64>,
    /// True when signature pagination hit MAX_SIGNATURE_PAGES before reaching genesis.
    pub age_capped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NftStats {
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DefiExposure {
    pub total_usd: Option<f64>,
    pub interaction_count: Option<usize>,
}

/// Fields the existing React UI already reads.
#[derive(Debug, Clone, Serialize)]
pub struct LegacyReputationResponse {
    /// WalletGuard activity score (1–100).
    pub reputation_score: u32,
    /// Activity band; legacy field names are retained for compatibility.
    pub tier: String,
    pub trust_label: String,
    pub balance: Balance,
    pub tx_stats: TxStats,
    pub wallet_age: WalletAge,
    pub nft_stats: NftStats,
    pub defi_exposure: DefiExposure,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct TokenRiskDto {
    pub mint: String,
    pub rugcheck_score: Option<i64>,
    pub solsniffer_score: Option<f64>,
    pub solsniffer_checked_at: Option<String>,
    pub honeypot: bool,
    pub flags: Vec<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReputationResponse {
    pub address: String,
    pub generated_at: Option<String>,
    #[serde(flatten)]
    pub legacy: LegacyReputationResponse,
    pub metrics: Vec<MetricDto>,
    pub improvement_steps: Vec<crate::analysis::improvements::ImprovementStep>,
    pub token_risks: Vec<TokenRiskDto>,
    pub integrations: serde_json::Value,
    pub funding_source: FundingSource,
    pub api_version: String,
    pub scoring_version: String,
    pub coverage: ScanCoverage,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ScanCoverage {
    pub history_source: String,
    pub timed_transactions: usize,
    pub sample_start_unix: Option<i64>,
    pub sample_end_unix: Option<i64>,
    pub token_scans: crate::integrations::TokenScanCoverage,
}
