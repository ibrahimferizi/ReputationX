# WalletGuard Frontend (React + Vite)

Interactive dashboard for analyzing Solana wallet reputation and sybil risk.

## Features

- **Sybil Scanner**: Scan multiple wallets for cluster activity and suspicious behavior
- **Reputation Analysis**: Detailed wallet metrics (token holdings, transaction patterns, DeFi interaction)
- **Cluster Visualization**: Graph view of detected sybil clusters and relationships
- **Response Caching**: 24-hour cache to reduce backend load on repeated scans
- **Error Boundaries**: Graceful error handling with clear user feedback

## Run

```bash
cd frontend
npm install
npm run dev
```

The dev server proxies `/api/*` to the backend (http://localhost:3001). To point to a different backend, set:

```bash
VITE_API_URL=https://api.example.com npm run dev
```

## Build

```bash
npm run build
npm run preview
```

## Environment Variables

- `VITE_API_URL` – Backend API base URL (optional; defaults to same origin)

## Key Components

- **SybilScanner** – Main component for batch wallet scanning with progress tracking
- **ClusterGraph** – D3-based visualization of sybil clusters
- **ReputationDashboard** – Detailed metrics and alerts for a single wallet
- **Cache** – localStorage-based caching layer (24-hour TTL per wallet)

