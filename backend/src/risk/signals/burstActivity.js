import { defineSignal } from './base.js';

export const burstActivitySignal = defineSignal('burst_activity', (features) => {
  if (!features.hasHistory) {
    return {
      id: 'burst_activity',
      delta: 0,
      reason: 'No activity to analyze for bursts',
    };
  }

  if (!features.isYoungWallet) {
    return {
      id: 'burst_activity',
      delta: 0,
      reason: 'Established wallet; burst patterns weighted lightly',
    };
  }

  if (features.isBurstActivity) {
    const severity = Math.min(
      220,
      features.maxTxsPerHour * 8 + features.maxTxsInFirstDay * 2
    );
    return {
      id: 'burst_activity',
      delta: -severity,
      reason: `Young wallet rapid-fire activity (${features.maxTxsPerHour} txs/hour peak, ${features.maxTxsInFirstDay} in first day)`,
    };
  }

  return {
    id: 'burst_activity',
    delta: 30,
    reason: 'Young wallet without dense burst clusters',
  };
});
