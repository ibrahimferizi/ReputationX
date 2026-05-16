import { defineSignal } from './base.js';

export const txCountSignal = defineSignal('tx_count', (features) => {
  if (!features.hasHistory) {
    return {
      id: 'tx_count',
      delta: 0,
      reason: 'No transactions on record',
    };
  }

  const { txCount, walletAgeDays } = features;
  const txsPerDay = walletAgeDays > 0 ? txCount / walletAgeDays : txCount;

  let delta;

  if (txCount >= 500) {
    delta = 180;
  } else if (txCount >= 100) {
    delta = 140;
  } else if (txCount >= 25) {
    delta = 90;
  } else if (txCount >= 5) {
    delta = 40;
  } else {
    delta = 10;
  }

  if (features.isYoungWallet && txsPerDay > 50) {
    delta -= 60;
  }

  return {
    id: 'tx_count',
    delta,
    reason: `${txCount} transactions (${txsPerDay.toFixed(1)}/day)`,
  };
});
