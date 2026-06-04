mod analysis;
mod config;
mod error;
mod integrations;
mod models;
mod routes;
mod solana;

use std::sync::Arc;

use axum::{routing::{get, post}, Router};
use tokio::sync::Semaphore;
use reqwest::Client;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::solana::rpc::SolanaRpc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub rpc: Arc<SolanaRpc>,
    /// Dedicated RPC for signature pagination (wallet age / deep history).
    pub history_rpc: Arc<SolanaRpc>,
    pub http: Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env (cwd is usually `backend/`; also try repo-root path).
    let _ = dotenvy::from_path("backend/.env");
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walletguard_api=info".parse()?))
        .init();

    let config = Arc::new(Config::from_env());
    let rpc_semaphore = Arc::new(Semaphore::new(config.rpc_global_concurrency));
    let rpc = Arc::new(
        SolanaRpc::new(
            config.solana_rpc_url.clone(),
            config.rpc_max_retries,
            config.rpc_retry_base_ms,
        )
        .with_concurrency(rpc_semaphore.clone()),
    );
    let history_rpc = Arc::new(
        SolanaRpc::new(
            config.solana_history_rpc_url.clone(),
            config.rpc_max_retries,
            config.rpc_retry_base_ms,
        )
        .with_concurrency(rpc_semaphore),
    );
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let state = AppState {
        config: config.clone(),
        rpc,
        history_rpc,
        http,
    };

    use tower_http::cors::AllowOrigin;
    
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|_origin, _request_head| true));

    let app = Router::new()
        .route("/api/reputation", get(routes::wallet::get_reputation))
        .route("/api/sybil-scan", post(routes::sybil::post_sybil_scan))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
        .layer(cors);

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("WalletGuard API listening on http://{}", addr);
    tracing::info!(
        scan_mode = ?config.scan_mode,
        max_signature_pages = config.max_signature_pages,
        max_age_signature_pages = config.max_age_signature_pages,
        signature_page_size = config.signature_page_size,
        history_gtfa = crate::solana::helius::history_rpc_supports_gtfa(&config.solana_history_rpc_url),
        sybil_scan_concurrency = config.sybil_scan_concurrency,
        rpc_global_concurrency = config.rpc_global_concurrency,
        "scan config"
    );
    tracing::info!("Solana RPC: configured");
    if config.solana_history_rpc_url != config.solana_rpc_url {
        tracing::info!("History RPC: configured");
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
