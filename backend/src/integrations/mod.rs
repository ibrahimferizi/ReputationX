pub mod rugcheck;
pub mod solsniffer;

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
        let sniff = solsniffer::scan_token_mint(client, config, mint).await;

        if rug.is_honeypot {
            honeypot_detected = true;
        }

        // Count a mint as high-risk only once, even if flagged by both integrations
        let is_rug_risk = rug.score.unwrap_or(0) > 4000 || rug.flags.iter().any(|f| f.contains("danger"));
        let is_sniff_risk = sniff.snifscore.unwrap_or(100.0) < 40.0;
        if is_rug_risk || is_sniff_risk {
            high_risk_tokens += 1;
        }

        rugcheck.push(rug);
        solsniffer.push(sniff);
    }

    ExternalScans {
        rugcheck,
        solsniffer,
        honeypot_detected,
        high_risk_tokens,
    }
}
