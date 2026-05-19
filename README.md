# WalletGuard

Solana wallet reputation analysis tool with sybil detection capabilities.

## Overview

WalletGuard provides on-chain Solana wallet reputation scoring and sybil attack detection through a Rust-based API and React frontend.

## Features

- **Wallet Reputation Scoring**: Analyzes wallet trustworthiness based on on-chain activity
- **Sybil Detection**: Identifies clusters of wallets sharing funding sources
- **Risk Metrics**: Detailed analysis of wallet age, transaction patterns, and token interactions
- **Caching**: 24-hour local storage cache to reduce API calls
- **Interactive Graph**: Visual representation of wallet clusters and risk levels

## Project Structure

```
.
├── backend/          # Rust API server
├── frontend/         # React + TypeScript + Vite frontend
└── README.md         # This file
```

## Prerequisites

- Rust (for backend)
- Node.js (for frontend)
- Solana RPC URL (public RPCs are rate-limited, consider using a paid RPC)

## Backend Setup

```bash
cd backend
cp .env.example .env
# Edit .env and set SOLANA_RPC_URL
cargo run
```

The API will be available at `http://localhost:3001`

### Backend Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/reputation?address=` | Full wallet reputation report |
| POST | `/api/sybil-scan` | Batch wallet sybil analysis |
| GET | `/health` | Health check |

### Build Backend

```bash
cd backend
cargo build --release
./target/release/walletguard-api
```

### Docker (Backend)

```bash
cd backend
docker build -t walletguard-api .
docker run -p 3001:3001 -e SOLANA_RPC_URL=... walletguard-api
```

## Frontend Setup

```bash
cd frontend
npm install
npm run dev
```

The frontend will be available at `http://localhost:5173`

### Build Frontend

```bash
cd frontend
npm run build
```

## Sample Test Addresses

Use these addresses to test the wallet reputation and sybil scanning features:

```
HgR2ifA5t7EMMfciZr65YZVaYxDQvUbLDrpd9tmkQT61
53unSgGWqEWANcPYRF35B2Bgf8BkszUtcccKiXwGGLyr
7sGdNQSvUGpahh6qyXB3g5gsdK9FAzZM299KyCXspump
42RLPACwZPx3vYYmxSueqsogfynBDqXK298EDsNoyoHi
7zqc5Zsqk4HPKrKZg9AhUkBQQej3HxRhqrQaRP1qoyY6
3JPYL9xEPFjefV3tccrUwhLzME1mMq2dQSDeDebgzQi6
CdFHmaj37EtjgRqvyt6vZqoA9tuMSvKLSmbgpuV6ejaP
hyDQ4Nz1eYyegS6JfenyKwKzYxRsCWCriYSAjtzP4Vg
HfMbPyDdZH6QMaDDUokjYCkHxzjoGBMpgaUvpLWGbF5p
25JUPL6ksapY1iCLWkFcSaXaA6Ar3W7JDCdPjeSApump
```

## Usage

### Single Wallet Check

1. Navigate to the "Wallet Check" tab
2. Enter a Solana wallet address
3. Click "Check wallet" to analyze
4. Results are cached for 24 hours

### Sybil Scan

1. Navigate to the "Sybil Scan" tab
2. Paste multiple wallet addresses (one per line)
3. Click "Scan" to analyze batch
4. View cluster graph and risk analysis
5. Use "Force Refresh" to bypass cache

## Caching

Both single wallet checks and sybil scans use localStorage caching:
- Cache duration: 24 hours
- Automatic cache expiration
- "Force Refresh" button to bypass cache
- Visual indicators show cached results with timestamps

## Development

### Backend Development

The backend is built with Rust and uses:
- Solana RPC for on-chain data
- Optional RugCheck/Solsniffer integrations
- Custom reputation scoring algorithm

### Frontend Development

The frontend uses:
- React + TypeScript
- Vite for build tooling
- Canvas-based cluster graph visualization
- localStorage for caching

## License

[Add your license here]
