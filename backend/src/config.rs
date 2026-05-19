use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanMode {
    Fast,
    Balanced,
    Deep,
}

impl ScanMode {
    pub fn from_env() -> Self {
        match env::var("SCAN_MODE")
            .unwrap_or_else(|_| "fast".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "balanced" => ScanMode::Balanced,
            "deep" => ScanMode::Deep,
            _ => ScanMode::Fast,
        }
    }

    pub fn max_pages(&self) -> usize {
        match self {
            ScanMode::Fast => 1,
            ScanMode::Balanced => 3,
            ScanMode::Deep => env::var("MAX_SIGNATURE_PAGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15),
        }
    }

    pub fn page_delay_ms(&self) -> u64 {
        match self {
            ScanMode::Fast => 0,
            ScanMode::Balanced => 150,
            ScanMode::Deep => env::var("SIGNATURE_PAGE_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub solana_rpc_url: String,
    /// Used for signature pagination (wallet age / tx depth). Falls back to `SOLANA_RPC_URL`.
    pub solana_history_rpc_url: String,
    pub scan_mode: ScanMode,
    pub signature_page_size: usize,
    pub max_signature_pages: usize,
    /// Separate cap for wallet-age pagination fallback (can be higher than tx scan).
    pub max_age_signature_pages: usize,
    pub signature_page_delay_ms: u64,
    pub rpc_max_retries: u32,
    pub rpc_retry_base_ms: u64,
    pub skip_external_scans: bool,
    pub helius_api_key: Option<String>,
    pub helius_tx_limit: u32,
    pub rugcheck_base_url: String,
    pub rugcheck_api_key: Option<String>,
    pub solsniffer_base_url: String,
    pub solsniffer_api_key: Option<String>,
    pub tokensniffer_base_url: String,
    pub tokensniffer_api_key: Option<String>,
    pub max_token_scans: usize,
    /// Max concurrent wallets during `POST /api/sybil-scan`.
    pub sybil_scan_concurrency: usize,
    /// Global cap on simultaneous JSON-RPC calls (all methods / wallets).
    pub rpc_global_concurrency: usize,
    /// Signature pages when walking to oldest tx for funding trace (fallback).
    pub funding_max_signature_pages: usize,
    pub funding_page_delay_ms: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let scan_mode = ScanMode::from_env();
        let skip_external = env::var("SKIP_EXTERNAL_SCANS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(scan_mode == ScanMode::Fast);

        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3001),
            solana_rpc_url: env::var("SOLANA_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into())
                .trim()
                .trim_matches('"')
                .to_string(),
            solana_history_rpc_url: {
                let key = env::var("HELIUS_API_KEY").ok().filter(|s| !s.is_empty());
                let raw = env::var("SOLANA_HISTORY_RPC_URL")
                    .or_else(|_| env::var("SOLANA_RPC_URL"))
                    .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());
                append_helius_api_key(raw.trim().trim_matches('"'), key.as_deref())
            },
            scan_mode,
            signature_page_size: env::var("SIGNATURE_PAGE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_signature_pages: scan_mode.max_pages(),
            // Wallet age is independent of SCAN_MODE; pagination fallback defaults to 50 pages.
            max_age_signature_pages: env::var("MAX_AGE_SIGNATURE_PAGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            signature_page_delay_ms: scan_mode.page_delay_ms(),
            rpc_max_retries: env::var("RPC_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            rpc_retry_base_ms: env::var("RPC_RETRY_BASE_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(800),
            skip_external_scans: skip_external,
            helius_api_key: env::var("HELIUS_API_KEY").ok().filter(|s| !s.is_empty()),
            helius_tx_limit: env::var("HELIUS_TX_LIMIT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            rugcheck_base_url: env::var("RUGCHECK_BASE_URL")
                .unwrap_or_else(|_| "https://api.rugcheck.xyz".into()),
            rugcheck_api_key: env::var("RUGCHECK_API_KEY").ok().filter(|s| !s.is_empty()),
            solsniffer_base_url: env::var("SOLSNIFFER_BASE_URL")
                .unwrap_or_else(|_| "https://api.solsniffer.com".into()),
            solsniffer_api_key: env::var("SOLSNIFFER_API_KEY").ok().filter(|s| !s.is_empty()),
            tokensniffer_base_url: env::var("TOKENSNIFFER_BASE_URL")
                .unwrap_or_else(|_| "https://tokensniffer.com/api/v2".into()),
            tokensniffer_api_key: env::var("TOKENSNIFFER_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            max_token_scans: env::var("MAX_TOKEN_SCANS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            sybil_scan_concurrency: env::var("SYBIL_SCAN_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2)
                .max(1),
            rpc_global_concurrency: env::var("RPC_GLOBAL_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6)
                .max(1),
            funding_max_signature_pages: env::var("FUNDING_MAX_SIGNATURE_PAGES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(50),
            funding_page_delay_ms: env::var("FUNDING_PAGE_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(250),
        }
    }
}

fn append_helius_api_key(url: &str, api_key: Option<&str>) -> String {
    if url.contains("api-key=") || url.contains("api_key=") {
        return url.to_string();
    }
    let Some(key) = api_key.filter(|k| !k.is_empty()) else {
        return url.to_string();
    };
    if url.contains('?') {
        format!("{url}&api-key={key}")
    } else {
        format!("{url}?api-key={key}")
    }
}
