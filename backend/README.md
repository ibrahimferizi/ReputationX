# WalletGuard API (Rust)

On-chain Solana wallet reputation API with optional RugCheck / Solsniffer integrations.

## Run

```bash
cd backend
cp .env.example .env
# Set a reliable SOLANA_RPC_URL (public mainnet RPC rate-limits heavily)
cargo run
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/reputation?address=` | Full wallet report (legacy + metrics) |
| GET | `/health` | Health check |

## Build

```bash
cargo build --release
./target/release/walletguard-api
```

## Docker

```bash
docker build -t walletguard-api .
docker run -p 3001:3001 -e SOLANA_RPC_URL=... walletguard-api
```
