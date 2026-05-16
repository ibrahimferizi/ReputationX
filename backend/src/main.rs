mod analysis;
mod config;
mod error;
mod integrations;
mod models;
mod routes;
mod solana;

use std::sync::Arc;

use axum::{routing::get, Router};
use reqwest::Client;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::solana::rpc::SolanaRpc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub rpc: Arc<SolanaRpc>,
    pub http: Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("walletguard_api=info".parse()?))
        .init();

    let config = Arc::new(Config::from_env());
    let rpc = Arc::new(SolanaRpc::new(
        config.solana_rpc_url.clone(),
        config.rpc_max_retries,
        config.rpc_retry_base_ms,
    ));
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let state = AppState {
        config: config.clone(),
        rpc,
        http,
    };

    let app = Router::new()
        .route("/api/reputation", get(routes::wallet::get_reputation))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let addr = format!("0.0.0.0:{}", config.port);
    tracing::info!("WalletGuard API listening on http://{}", addr);
    tracing::info!("Solana RPC: {}", config.solana_rpc_url);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
