# WalletGuard API (Rust)

On-chain Solana wallet reputation API with optional RugCheck / Solsniffer integrations.

## Run

```bash
cd backend
cp -n .env.example .env
# Set a reliable SOLANA_RPC_URL (public mainnet RPC rate-limits heavily)
cargo run --locked
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/reputation?address=` | Full wallet report (legacy + metrics) |
| POST | `/api/sybil-scan` | Scan multiple wallets for sybil & cluster activity |
| GET | `/health` | Health check |

### POST /api/sybil-scan

Request body:
```json
{
  "addresses": ["11111111111111111111111111111112", "...]
}
```

Response:
```json
{
  "wallets": [...],
  "clusters": [...],
  "total_scanned": 2,
  "flagged": 0
}
```

Max 10 entries per request. Duplicate addresses are scanned once.

By default, cross-origin browser access is disabled. Set `CORS_ALLOWED_ORIGINS` to a comma-separated list of frontend origins for separate frontend/API hosting. The local Vite proxy needs no CORS configuration.

## Build

```bash
cargo build --release --locked
./target/release/walletguard-api
```

## Docker

```bash
docker build -t walletguard-api .
docker run -p 3001:3001 -e SOLANA_RPC_URL=... walletguard-api
```

See [technical documentation](../DOCUMENTATION.md) for limitations and [the root README](../README.md) for the full setup.

## Report contract

Responses identify `api_version: walletguard-v2` and `scoring_version: activity-v3`. `risk` now includes `unknown`; `tx_stats.sampled` distinguishes sampled history; `coverage` reports timestamp/provider coverage; `defi_exposure.total_usd` and `interaction_count` are null until implemented. Scores use the attainable raw maximum of 710, so they must not be compared directly to v1 scores.

Token checks use a shared deadline and retain completed results. Incomplete coverage produces an unknown token metric; an actual provider risk flag takes precedence and produces a high-risk metric. Completed scans are restricted to the detected held legacy SPL mints and enabled providers.

```bash
cargo build --locked
python3 tests/api_smoke.py
```

The smoke tests run only local mock providers; no API credentials are needed.


## Token provider setup and diagnostics

Set `SKIP_EXTERNAL_SCANS=false` to enable scans. RugCheck uses its public summary endpoint when `RUGCHECK_API_KEY` is empty. Create the key under **RugCheck → API Keys** in FluxRPC, separately from Solana RPC keys. Plain keys are sent as `X-API-KEY`; a value explicitly prefixed with `Bearer ` uses JWT authentication. See [FluxRPC RugCheck setup](https://fluxrpc.com/docs/rugcheck/getting-started). If the official endpoint rejects the credential with HTTP 401, the request retries once without authentication and reports `public_fallback`. Custom endpoints do not receive this fallback.

SolSniffer requires `SOLSNIFFER_API_KEY`. Its default base is `https://solsniffer.com/api/v2`, requests use `/token/{mint}` and `X-API-KEY`, and scores come from `tokenData.score`. Contracts: [RugCheck OpenAPI](https://api.rugcheck.xyz/swagger/doc.json), [SolSniffer API docs](https://solsniffer.com/api/docs/).

`coverage.token_scans.issues` exposes safe per-provider diagnostics, including HTTP status, missing reports, rate limits and timeouts. The UI displays these without exposing credentials or raw provider responses. A successful public fallback can coexist with complete coverage. A scan capped below the number of held mints remains partial even when every selected mint succeeds.

An optional live contract test loads `backend/.env` and spends provider quota on one example token. Both providers must be available and SolSniffer must be configured:

```bash
cargo test --locked live_provider_contract_smoke -- --ignored --nocapture
```

## Public hosting

Use the repository-root Dockerfile for a complete app, and [deployment instructions](../README.md#deployment) for the free Render preset. The API applies request/body/batch limits, bounded caching, scan budgets and deadlines. `generated_at` records scan completion; token findings no longer penalize the activity score in v3.

Build the frontend before running `python3 tests/public_smoke.py`; it checks static hosting and service controls against local mocks.

SolSniffer Free currently allows [100 API calls/month](https://www.solsniffer.com/api-service). Results are cached per mint for one hour, with `solsniffer_checked_at` and a cache diagnostic. `SOLSNIFFER_MAX_CALLS_PER_RUN` defaults to 90 attempts, max 100; this counter resets on restart and is not a persistent monthly budget. Access/quota errors pause new calls for ten minutes. Other providers' results remain usable. The deployment blueprint now accepts a SolSniffer key.
