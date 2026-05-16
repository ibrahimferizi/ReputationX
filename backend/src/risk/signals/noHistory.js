import { defineSignal } from './base.js';

export const noHistorySignal = defineSignal('no_history', (features) => {
  if (features.hasHistory) {
    return {
      id: 'no_history',
      delta: 0,
      reason: 'Wallet has on-chain transaction history',
    };
  }

  return {
    id: 'no_history',
    delta: -120,
    reason: 'No on-chain transactions found',
  };
});
