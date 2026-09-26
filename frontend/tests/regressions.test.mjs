import assert from "node:assert/strict";
import { beforeEach, test } from "node:test";
import { getCachedScan, getCachedWallet, setCachedScan, setCachedWallet } from "../src/utils/cache.ts";
import { normalizeReport } from "../src/utils/normalizeReport.ts";
import { isSolanaAddress } from "../src/utils/address.ts";

let storage;
beforeEach(() => {
  storage = new Map();
  globalThis.localStorage = {
    getItem: (key) => storage.get(key) ?? null,
    setItem: (key, value) => storage.set(key, value),
    removeItem: (key) => storage.delete(key),
  };
});
const report = (address) => normalizeReport({ address, reputation_score: 80 });
const scan = () => ({
  wallets: [report("a"), report("b")],
  clusters: [{ cluster_id: "shared", funding_address: "f", members: ["a", "b"], suspicion: "low", reasons: [] }],
  total_scanned: 2, flagged: 2,
});

test("same batch in another order preserves clusters and server flag count", () => {
  const response = scan();
  setCachedScan(response);
  assert.deepEqual(getCachedScan(["b", "a"]), JSON.parse(JSON.stringify(response)));
  assert.equal(getCachedWallet("a").reputation_score, 80);
});

test("subset, superset and unrelated batches require a complete fresh scan", () => {
  setCachedScan(scan());
  for (const addresses of [["a"], ["a", "b", "c"], ["c", "d"]]) {
    assert.equal(getCachedScan(addresses), null);
  }
});

test("individually cached wallets cannot stand in for a cluster analysis", () => {
  setCachedWallet(report("a"));
  setCachedWallet(report("b"));
  assert.equal(getCachedScan(["a", "b"]), null);
});

test("refreshing a batch member invalidates its batch snapshot", () => {
  setCachedScan(scan());
  setCachedWallet(report("a"));
  assert.equal(getCachedScan(["a", "b"]), null);
});

test("expired batches are not reused even if wallet timestamps are fresh", () => {
  setCachedScan(scan());
  const [key, value] = [...storage.entries()][0];
  const cache = JSON.parse(value);
  cache.scan.timestamp = Date.now() - 24 * 60 * 60 * 1000;
  storage.set(key, JSON.stringify(cache));
  assert.equal(getCachedScan(["a", "b"]), null);
});

test("normalization handles malformed nested values and preserves funding", () => {
  const normalized = normalizeReport({
    address: "a", metrics: [null, { id: "x", risk: "unexpected" }],
    improvement_steps: [null, { steps: ["useful", 9] }],
    token_risks: [null, { mint: "m", flags: ["danger", 4] }],
    tx_stats: { max_per_hour: "12" }, wallet_age: { days_precise: "bad" },
    funding_source: { source_type: "wallet", source_address: "f", confidence: "medium" },
  });
  assert.equal(normalized.metrics[0].risk, "unknown");
  assert.deepEqual(normalized.improvement_steps[0].steps, ["useful"]);
  assert.deepEqual(normalized.token_risks[0].flags, ["danger"]);
  assert.equal(normalized.tx_stats.max_per_hour, 12);
  assert.equal(normalized.wallet_age.days_precise, undefined);
  assert.equal(normalized.funding_source.source_address, "f");
});

test("address validation requires Base58 encoding of exactly 32 bytes", () => {
  assert.equal(isSolanaAddress("11111111111111111111111111111111"), true);
  assert.equal(isSolanaAddress("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"), true);
  for (const value of ["", "0".repeat(32), "1".repeat(31), "1".repeat(33), "x".repeat(10000)]) {
    assert.equal(isSolanaAddress(value), false);
  }
});


test("coverage metadata and unmeasured DeFi values survive normalization", () => {
  const response = normalizeReport({ address: "a", api_version: "walletguard-v2", scoring_version: "activity-v2",
    tx_stats: { count: 100, sampled: true, capped: true },
    defi_exposure: { total_usd: null, interaction_count: null },
    metrics: [{ id: "token_risk", risk: "unknown", value: "Not checked" }],
    coverage: { history_source: "helius", timed_transactions: 100, sample_start_unix: 1000, sample_end_unix: 2000,
      token_scans: { status: "skipped", held_mints: 67, checked_mints: 0, selected_mints: 0, fully_checked_mints: 0,
        rugcheck_checked: 0, solsniffer_checked: 0, solsniffer_enabled: false } },
  });
  assert.equal(response.defi_exposure.total_usd, null);
  assert.equal(response.defi_exposure.interaction_count, null);
  assert.equal(response.tx_stats.sampled, true);
  assert.equal(response.coverage.token_scans.status, "skipped");
  assert.equal(response.coverage.token_scans.held_mints, 67);
  assert.equal(response.scoring_version, "activity-v2");
  assert.equal(response.metrics[0].risk, "unknown");
  setCachedWallet(response);
  assert.deepEqual(getCachedWallet("a").coverage, response.coverage);
});

test("previous scoring caches are not reused", () => {
  for (const key of ["walletguard_scan_cache", "walletguard_scan_cache_v2"]) {
    storage.set(key, JSON.stringify({wallets:{a:{report:report("a"), timestamp:Date.now()}}}));
  }
  assert.equal(getCachedWallet("a"), null);
  assert.equal(getCachedScan(["a"]), null);
});


test("provider diagnostics and SolSniffer scores survive normalization", () => {
  const report = normalizeReport({token_risks:[{mint:"m", available:true, solsniffer_score:47}],coverage:{
    token_scans:{status:"partial",issues:[{provider:"rugcheck",mint:"m",code:"public_fallback",http_status:401,message:"Retried public summary."}]}}});
  assert.equal(report.token_risks[0].solsniffer_score, 47);
  assert.equal(report.coverage.token_scans.issues[0].http_status, 401);
  assert.equal(report.coverage.token_scans.issues[0].code, "public_fallback");
});
