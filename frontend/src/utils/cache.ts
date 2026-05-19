import type { ReputationReport, SybilScanResponse } from "../types/api";

const CACHE_KEY = "walletguard_scan_cache";
const CACHE_DURATION_MS = 24 * 60 * 60 * 1000; // 24 hours

interface CachedWalletData {
  report: ReputationReport;
  timestamp: number;
}

interface CachedScanData {
  wallets: Record<string, CachedWalletData>;
  clusters: any[];
  timestamp: number;
}

export function getCachedWallet(address: string): ReputationReport | null {
  try {
    const cacheStr = localStorage.getItem(CACHE_KEY);
    if (!cacheStr) return null;

    const cache: CachedScanData = JSON.parse(cacheStr);
    const cached = cache.wallets[address];

    if (!cached) return null;

    const age = Date.now() - cached.timestamp;
    if (age > CACHE_DURATION_MS) {
      // Cache expired, remove it
      delete cache.wallets[address];
      localStorage.setItem(CACHE_KEY, JSON.stringify(cache));
      return null;
    }

    return cached.report;
  } catch (error) {
    console.error("Error reading cache:", error);
    return null;
  }
}

export function setCachedWallet(report: ReputationReport): void {
  try {
    const cacheStr = localStorage.getItem(CACHE_KEY);
    const cache: CachedScanData = cacheStr
      ? JSON.parse(cacheStr)
      : { wallets: {}, clusters: [], timestamp: Date.now() };

    cache.wallets[report.address] = {
      report,
      timestamp: Date.now(),
    };

    localStorage.setItem(CACHE_KEY, JSON.stringify(cache));
  } catch (error) {
    console.error("Error writing cache:", error);
  }
}

export function getCachedScan(addresses: string[]): {
  cached: Map<string, ReputationReport>;
  uncached: string[];
} {
  const cached = new Map<string, ReputationReport>();
  const uncached: string[] = [];

  try {
    const cacheStr = localStorage.getItem(CACHE_KEY);
    if (!cacheStr) {
      return { cached, uncached: addresses };
    }

    const cache: CachedScanData = JSON.parse(cacheStr);
    const now = Date.now();

    addresses.forEach((address) => {
      const cachedData = cache.wallets[address];
      if (!cachedData) {
        uncached.push(address);
        return;
      }

      const age = now - cachedData.timestamp;
      if (age > CACHE_DURATION_MS) {
        // Cache expired
        delete cache.wallets[address];
        uncached.push(address);
      } else {
        cached.set(address, cachedData.report);
      }
    });

    // Clean up expired entries
    localStorage.setItem(CACHE_KEY, JSON.stringify(cache));
  } catch (error) {
    console.error("Error reading cache:", error);
    return { cached: new Map(), uncached: addresses };
  }

  return { cached, uncached };
}

export function setCachedScan(response: SybilScanResponse): void {
  try {
    const cacheStr = localStorage.getItem(CACHE_KEY);
    const cache: CachedScanData = cacheStr
      ? JSON.parse(cacheStr)
      : { wallets: {}, clusters: [], timestamp: Date.now() };

    response.wallets.forEach((wallet) => {
      cache.wallets[wallet.address] = {
        report: wallet,
        timestamp: Date.now(),
      };
    });

    cache.clusters = response.clusters;
    cache.timestamp = Date.now();

    localStorage.setItem(CACHE_KEY, JSON.stringify(cache));
  } catch (error) {
    console.error("Error writing cache:", error);
  }
}

export function clearCache(): void {
  try {
    localStorage.removeItem(CACHE_KEY);
  } catch (error) {
    console.error("Error clearing cache:", error);
  }
}

export function getCacheTimestamp(address: string): number | null {
  try {
    const cacheStr = localStorage.getItem(CACHE_KEY);
    if (!cacheStr) return null;

    const cache: CachedScanData = JSON.parse(cacheStr);
    const cached = cache.wallets[address];

    if (!cached) return null;

    return cached.timestamp;
  } catch (error) {
    console.error("Error reading cache timestamp:", error);
    return null;
  }
}

export function formatCacheTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHours = Math.floor(diffMs / (1000 * 60 * 60));

  if (diffHours < 1) {
    const diffMinutes = Math.floor(diffMs / (1000 * 60));
    return `${diffMinutes} minute${diffMinutes !== 1 ? "s" : ""} ago`;
  } else if (diffHours < 24) {
    return `${diffHours} hour${diffHours !== 1 ? "s" : ""} ago`;
  } else {
    return date.toLocaleDateString();
  }
}
