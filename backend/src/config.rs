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
    pub scan_mode: ScanMode,
    pub signature_page_size: usize,
    pub max_signature_pages: usize,
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
            scan_mode,
            signature_page_size: env::var("SIGNATURE_PAGE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_signature_pages: scan_mode.max_pages(),
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
        }
    }
}
