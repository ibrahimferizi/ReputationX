# WalletGuard frontend

React + TypeScript + Vite dashboard. Requires Node.js 24+; use `nvm use` if available.

```bash
npm ci
npm run dev
```

Open http://localhost:5173 with the Rust API running on port 3001. The development server proxies `/api` and `/health` to the API.

`VITE_API_URL` defaults to the same origin. For a separate API domain, set it before `npm run build` and configure `CORS_ALLOWED_ORIGINS` on the API. See `.env.example`. Never put provider secrets into `VITE_*` variables; they become public browser code.

```bash
npm test
npm run lint
npm run build
npm run preview
```

The tests use Node's built-in runner and TypeScript stripping, with no test-framework dependencies. Preview serves the static build; the Vite development proxy does not apply there.

- `App.tsx`: wallet report, tabs and recent checks.
- `SybilScanner.tsx`: batch input and results; scans deduplicate addresses.
- `ClusterGraph.tsx`: custom Canvas simulation, dragging and hover details.
- `utils/cache.ts`: Five-minute wallet cache and last complete batch response. Batch cache reuse requires the exact address set, preserving server-computed clusters and flagged counts. Partial hits trigger a full batch request.
- `utils/normalizeReport.ts`: normalizes single-wallet API responses.
- `utils/address.ts`: checks that Base58 input decodes to 32 bytes using [bs58](https://github.com/cryptocoinjs/bs58).

See [the project README](../README.md) for setup and [technical documentation](../DOCUMENTATION.md) for report semantics and limitations.

## Current report behavior

The UI uses `walletguard-v2` coverage metadata and `activity-v3` scores. The `ScanCoverage` component is available on both screens, including wallets without flags. `unknown` risks display a neutral question mark. DeFi values remain null rather than being converted to zero. Unknown checks do not count as individual risk flags, but confirmed high-risk metrics still do.

The score cache and session-history namespaces were changed for this scoring version; old results are not reused. Restart the API and refresh the page after updating. Tests include actual coverage/icon server rendering using the existing TypeScript compiler.
