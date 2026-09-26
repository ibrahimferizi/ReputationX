//! Bounded, process-local admission and scan cache for a single API instance.
use crate::{
    analysis::reputation::build_wallet_reputation, error::ApiError,
    models::response::ReputationResponse, AppState,
};
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

#[derive(Clone)]
pub struct ServiceConfig {
    pub cache_ttl: Duration,
    pub cache_capacity: usize,
    pub wallet_timeout: Duration,
    pub request_timeout: Duration,
    pub request_concurrency: usize,
    pub scan_concurrency: usize,
    pub ip_units_per_minute: usize,
    pub fresh_scans_per_hour: usize,
    pub trusted_proxies: Vec<IpAddr>,
}
impl ServiceConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        fn setting(name: &str, default: usize, max: usize) -> anyhow::Result<usize> {
            let value = match std::env::var(name) {
                Ok(value) => value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("{name} must be an integer"))?,
                Err(_) => default,
            };
            anyhow::ensure!(
                (1..=max).contains(&value),
                "{name} must be between 1 and {max}"
            );
            Ok(value)
        }
        Ok(Self {
            cache_ttl: Duration::from_secs(setting("CACHE_TTL_SECONDS", 300, 3600)? as u64),
            cache_capacity: setting("CACHE_MAX_WALLETS", 256, 4096)?,
            wallet_timeout: Duration::from_secs(setting("WALLET_TIMEOUT_SECONDS", 45, 90)? as u64),
            request_timeout: Duration::from_secs(
                setting("REQUEST_TIMEOUT_SECONDS", 120, 150)? as u64
            ),
            request_concurrency: setting("MAX_ACTIVE_REQUESTS", 8, 64)?,
            scan_concurrency: setting("MAX_ACTIVE_SCANS", 4, 32)?,
            ip_units_per_minute: setting("IP_SCAN_UNITS_PER_MINUTE", 30, 1000)?,
            fresh_scans_per_hour: setting("FRESH_SCANS_PER_HOUR", 120, 10000)?,
            trusted_proxies: std::env::var("TRUSTED_PROXY_IPS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::parse)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Default)]
struct Window {
    start: Option<Instant>,
    units: usize,
}
impl Window {
    fn charge(
        &mut self,
        now: Instant,
        period: Duration,
        limit: usize,
        cost: usize,
    ) -> Result<(), ApiError> {
        if self
            .start
            .is_none_or(|start| now.duration_since(start) >= period)
        {
            self.start = Some(now);
            self.units = 0;
        }
        if self.units.saturating_add(cost) > limit {
            let remaining = period.saturating_sub(now.duration_since(self.start.unwrap()));
            return Err(ApiError::RateLimited(remaining.as_secs().max(1)));
        }
        self.units += cost;
        Ok(())
    }
}

pub struct ScanService {
    pub config: ServiceConfig,
    requests: Semaphore,
    scans: Semaphore,
    ip_windows: Mutex<HashMap<IpAddr, Window>>,
    fresh_window: Mutex<Window>,
    cache: Mutex<HashMap<String, (Instant, ReputationResponse)>>,
    // Fixed stripes coalesce duplicate misses without an unbounded lock map.
    locks: Vec<AsyncMutex<()>>,
}
impl ScanService {
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            requests: Semaphore::new(config.request_concurrency),
            scans: Semaphore::new(config.scan_concurrency),
            config,
            ip_windows: Mutex::new(HashMap::new()),
            fresh_window: Mutex::new(Window::default()),
            cache: Mutex::new(HashMap::new()),
            locks: (0..64).map(|_| AsyncMutex::new(())).collect(),
        }
    }
    fn cached(&self, address: &str) -> Option<ReputationResponse> {
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|_, (at, _)| at.elapsed() < self.config.cache_ttl);
        cache.get(address).map(|(_, report)| report.clone())
    }
    fn charge_ip(&self, ip: IpAddr, cost: usize) -> Result<(), ApiError> {
        let now = Instant::now();
        let period = Duration::from_secs(60);
        let mut windows = self.ip_windows.lock().unwrap();
        windows.retain(|_, w| w.start.is_some_and(|at| now.duration_since(at) < period));
        if !windows.contains_key(&ip) && windows.len() >= 4096 {
            return Err(ApiError::RateLimited(60));
        }
        windows
            .entry(ip)
            .or_default()
            .charge(now, period, self.config.ip_units_per_minute, cost)
    }
    pub async fn report(
        &self,
        state: &AppState,
        address: &str,
    ) -> Result<ReputationResponse, ApiError> {
        if let Some(report) = self.cached(address) {
            return Ok(report);
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        address.hash(&mut hasher);
        let _lock = self.locks[hasher.finish() as usize % self.locks.len()]
            .lock()
            .await;
        if let Some(report) = self.cached(address) {
            return Ok(report);
        }
        let _permit = self.scans.acquire().await.map_err(|_| ApiError::Busy)?;
        self.fresh_window.lock().unwrap().charge(
            Instant::now(),
            Duration::from_secs(3600),
            self.config.fresh_scans_per_hour,
            1,
        )?;
        let mut report = tokio::time::timeout(
            self.config.wallet_timeout,
            build_wallet_reputation(
                &state.http,
                &state.rpc,
                &state.history_rpc,
                &state.config,
                address,
            ),
        )
        .await
        .map_err(|_| ApiError::Timeout)?
        .map_err(ApiError::internal)?;
        report.generated_at = Some(chrono::Utc::now().to_rfc3339());
        let mut cache = self.cache.lock().unwrap();
        if cache.len() >= self.config.cache_capacity {
            if let Some(oldest) = cache
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
            {
                cache.remove(&oldest);
            }
        }
        cache.insert(address.into(), (Instant::now(), report.clone()));
        Ok(report)
    }
}

fn client_ip(peer: IpAddr, header: Option<&str>, trusted: &[IpAddr]) -> IpAddr {
    if trusted.contains(&peer) {
        header.and_then(|s| s.parse().ok()).unwrap_or(peer)
    } else {
        peer
    }
}

pub async fn protect(
    State(service): State<Arc<ScanService>>,
    request: Request,
    next: Next,
) -> Response {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip());
    let Some(peer) = peer else {
        return ApiError::Busy.into_response();
    };
    let ip = client_ip(
        peer,
        request
            .headers()
            .get("x-walletguard-client-ip")
            .and_then(|s| s.to_str().ok()),
        &service.config.trusted_proxies,
    );
    // Charge the maximum batch size before accepting or parsing a body.
    let cost = if request.uri().path() == "/api/sybil-scan" {
        10
    } else {
        1
    };
    if let Err(error) = service.charge_ip(ip, cost) {
        return error.into_response();
    }
    let Ok(_permit) = service.requests.try_acquire() else {
        return ApiError::Busy.into_response();
    };
    let start = Instant::now();
    let mut response =
        match tokio::time::timeout(service.config.request_timeout, next.run(request)).await {
            Ok(response) => response,
            Err(_) => ApiError::Timeout.into_response(),
        };
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    tracing::info!(
        status = response.status().as_u16(),
        elapsed_ms = start.elapsed().as_millis() as u64,
        "scan request completed"
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_rejects_until_reset() {
        let now = Instant::now();
        let mut w = Window::default();
        let period = Duration::from_secs(60);
        assert!(w.charge(now, period, 10, 10).is_ok());
        assert!(matches!(
            w.charge(now, period, 10, 1),
            Err(ApiError::RateLimited(60))
        ));
        assert!(w.charge(now + period, period, 10, 1).is_ok());
    }
    #[test]
    fn forwarding_headers_require_trusted_socket_peer() {
        let peer = "127.0.0.1".parse().unwrap();
        let supplied = "192.0.2.1";
        assert_eq!(client_ip(peer, Some(supplied), &[]), peer);
        assert_eq!(
            client_ip(peer, Some(supplied), &[peer]),
            supplied.parse::<IpAddr>().unwrap()
        );
        assert_eq!(client_ip(peer, Some("bad, spoofed"), &[peer]), peer);
    }
}
