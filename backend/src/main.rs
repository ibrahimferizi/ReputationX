mod analysis;
mod config;
mod error;
mod integrations;
mod models;
mod routes;
mod service;
mod solana;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use reqwest::Client;
use tokio::sync::Semaphore;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::solana::rpc::SolanaRpc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub service: Arc<service::ScanService>,
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
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("walletguard_api=info".parse()?),
        )
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

    let service = Arc::new(service::ScanService::new(
        service::ServiceConfig::from_env()?
    ));
    let state = AppState {
        service: service.clone(),
        config: config.clone(),
        rpc,
        history_rpc,
        http,
    };

    let origins: Vec<axum::http::HeaderValue> = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    let mut app = Router::new()
        .route("/api/reputation", get(routes::wallet::get_reputation))
        .route("/api/sybil-scan", post(routes::sybil::post_sybil_scan))
        .layer(axum::extract::DefaultBodyLimit::max(8192))
        .route_layer(axum::middleware::from_fn_with_state(
            service,
            service::protect,
        ))
        .route("/health", get(|| async { "ok" }))
        .route(
            "/api/*path",
            get(|| async {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    axum::Json(serde_json::json!({"error":"Unknown API endpoint"})),
                )
            }),
        )
        .with_state(state)
        .layer(cors);

    if let Ok(dir) = std::env::var("STATIC_DIR") {
        anyhow::ensure!(
            std::path::Path::new(&dir).join("index.html").is_file(),
            "STATIC_DIR must contain a built frontend index.html"
        );
        app = app.fallback_service(tower_http::services::ServeDir::new(&dir));
    }
    app = app.layer(axum::middleware::from_fn(security_headers));

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
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = interrupt => {}, _ = terminate => {} }
    tracing::info!("Shutdown requested; draining bounded requests");
}

async fn security_headers(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("referrer-policy", "no-referrer".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()".parse().unwrap(),
    );
    headers.insert("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'".parse().unwrap());
    headers.insert("cache-control", "no-store".parse().unwrap());
    response
}
