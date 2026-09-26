# WalletGuard

A public-beta project originating at a hackathon for investigating Solana wallet activity. Paste one address for a reputation report, or up to 10 for shared-funder cluster analysis.

The app reads Solana mainnet data and computes heuristic scores off-chain. It does not connect wallets, sign transactions, or deploy a smart contract. There is no database or login.

- **Frontend:** React 19, TypeScript, Vite 8, CSS and a custom Canvas graph.
- **Backend:** Rust, Axum 0.7, Tokio, reqwest; REST API on port 3001.
- **Data:** Solana JSON-RPC; optional Helius history, RugCheck and SolSniffer.
- **Browser storage:** Five-minute report/batch cache in localStorage; last five wallet checks in sessionStorage.

[DOCUMENTATION.md](DOCUMENTATION.md) describes the architecture, scoring rules, coverage and service limits.

## Run locally

Use Node.js 24+ (`frontend/.nvmrc` selects 24) and Rust 1.95 (the tested compiler and Docker builder).

In one terminal:

```bash
cd backend
# First setup only; preserve your existing .env if present:
cp -n .env.example .env
# Set SOLANA_RPC_URL; configure HELIUS_API_KEY if using Helius.
cargo run --locked
```

In another terminal:

```bash
cd frontend
npm ci
npm run dev
```

Open http://localhost:5173. Vite proxies `/api` and `/health` to http://127.0.0.1:3001. Leave `VITE_API_URL` unset for this setup.

The backend example contains placeholder Helius credentials: replace them or use a different RPC. Without any configuration, the backend defaults to the public Solana mainnet endpoint. Scan speed and coverage depend on the provider and rate limits.

## Reports and coverage

Reports use `api_version: walletguard-v2` and `scoring_version: activity-v3`. The heuristic activity score is scaled to the implemented maximum (710 raw points), making the full 1–100 range reachable. It is not a safety probability or a validated identity assessment.

Both screens show transaction sampling, the observed time window, token-check coverage and funding availability. Unknown checks use a neutral `?` icon. Token checks distinguish skipped, timed-out, unavailable, partial and completed scans. DeFi interaction count and USD exposure are **null / Not assessed** until instruction-level analysis is implemented.

Version 3 separates token findings from the score and uses activity bands instead of trust labels. A low score alone does not flag a wallet. Reports include generation timestamps; server caching can reuse a recent report even after Refresh.

Old browser caches and recent-score history are isolated from this scoring version. After updating, restart the backend with `cargo run --locked` and refresh the frontend.

## API

| Method | Route | Purpose |
|---|---|---|
| GET | `/health` | Process health; does not verify upstream providers |
| GET | `/api/reputation?address=...` | Wallet metrics, score and funding source |
| POST | `/api/sybil-scan` | `{"addresses":["..."]}`; maximum 10 entries, duplicates scanned once |

`SCAN_MODE=fast` is the default. External token checks default to disabled in fast mode. Set `SKIP_EXTERNAL_SCANS=false` to enable them. Wallet-age and funding pagination have separate limits; fast mode does not guarantee a short request.

## Checks and builds

```bash
cd frontend
npm test
npm run lint
npm run build
npm audit
```

```bash
cd backend
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

For an API smoke test against local mock RPC/token providers (no credentials or credits):

```bash
cd backend
cargo build --locked
python3 tests/api_smoke.py
```

Frontend build output is `frontend/dist/`. Backend output is `backend/target/release/walletguard-api`.

## Deployment

The root [Dockerfile](Dockerfile) builds the frontend and API into one non-root container. [render.yaml](render.yaml) defines a single Render Free service with managed HTTPS, no database, and automatic deployments disabled. Free-tier sleeping and usage limits apply; see [Render's documentation](https://render.com/docs/free).

1. Create a Render Blueprint from the branch containing these changes.
2. Supply `SOLANA_RPC_URL` and `HELIUS_API_KEY` as secret environment variables. Add `SOLSNIFFER_API_KEY` to enable SolSniffer; leave it empty to omit that provider. RugCheck can use its public endpoint or an optional `RUGCHECK_API_KEY`.
3. Leave `VITE_API_URL` unset for the combined container. Render supplies `PORT`; the API serves the built frontend from `STATIC_DIR`.
4. Deploy after the repository checks pass, then verify `/health`, a wallet report and a small batch at the public URL.

Provider credentials stay on the backend. Environment files and local notes are excluded from Git and container builds. Configure free provider plans and account usage controls separately: process-local budgets reset on restart and do not enforce account-wide monthly quotas. SolSniffer's [Free plan](https://www.solsniffer.com/api-service) includes 100 calls/month.

See [service limits](DOCUMENTATION.md#service-limits) and [provider configuration](backend/README.md#token-provider-setup-and-diagnostics). The [Checks workflow](.github/workflows/checks.yml) runs unit tests, mock API/browser checks, and a container smoke test without live provider credentials.
