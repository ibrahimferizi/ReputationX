import { useCallback, useState } from "react";
import type { HistoryEntry } from "../types/api";

const STORAGE_KEY = "walletguard_history_activity_v3";
const MAX_ENTRIES = 5;

function readHistory(): HistoryEntry[] {
  try {
    const raw = sessionStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as HistoryEntry[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function useWalletHistory() {
  const [history, setHistory] = useState<HistoryEntry[]>(readHistory);


  const addEntry = useCallback((entry: Omit<HistoryEntry, "checkedAt">) => {
    setHistory((prev) => {
      const next: HistoryEntry[] = [
        { ...entry, checkedAt: new Date().toISOString() },
        ...prev.filter((h) => h.address !== entry.address),
      ].slice(0, MAX_ENTRIES);
      sessionStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      return next;
    });
  }, []);

  const clearHistory = useCallback(() => {
    sessionStorage.removeItem(STORAGE_KEY);
    setHistory([]);
  }, []);

  return { history, addEntry, clearHistory };
}
