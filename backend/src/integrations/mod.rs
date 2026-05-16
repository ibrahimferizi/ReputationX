pub mod rugcheck;
pub mod solsniffer;
pub mod tokensniffer;

use crate::config::Config;
use reqwest::Client;

use self::rugcheck::RugCheckResult;
use self::solsniffer::SolsnifferResult;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExternalScans {
    pub rugcheck: Vec<RugCheckResult>,
    pub solsniffer: Vec<SolsnifferResult>,
    pub honeypot_detected: bool,
    pub high_risk_tokens: usize,
}

pub async fn scan_wallet_tokens(
    client: &Client,
    config: &Config,
    mints: &[String],
) -> ExternalScans {
    let mut rugcheck = Vec::new();
    let mut solsniffer = Vec::new();
    let mut honeypot_detected = false;
    let mut high_risk_tokens = 0usize;

    for mint in mints.iter().take(config.max_token_scans) {
        let rug = rugcheck::scan_token_mint(client, config, mint).await;
        if rug.is_honeypot {
            honeypot_detected = true;
        }
        if rug.score.unwrap_or(0) > 4000 || rug.flags.iter().any(|f| f.contains("danger")) {
            high_risk_tokens += 1;
        }
        rugcheck.push(rug);

        let sniff = solsniffer::scan_token_mint(client, config, mint).await;
        if sniff.snifscore.unwrap_or(100.0) < 40.0 {
            high_risk_tokens += 1;
        }
        solsniffer.push(sniff);
    }

    ExternalScans {
        rugcheck,
        solsniffer,
        honeypot_detected,
        high_risk_tokens,
    }
}
