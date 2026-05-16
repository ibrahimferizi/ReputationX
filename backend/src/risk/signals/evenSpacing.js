import { defineSignal } from './base.js';

export const evenSpacingSignal = defineSignal('even_spacing', (features) => {
  if (!features.hasHistory || !features.isYoungWallet) {
    return {
      id: 'even_spacing',
      delta: 0,
      reason: 'Even-spacing bonus only applies to young wallets',
    };
  }

  if (features.isBurstActivity) {
    return {
      id: 'even_spacing',
      delta: 0,
      reason: 'Burst activity present; spacing pattern not trusted',
    };
  }

  if (features.isEvenlySpacedYoung) {
    return {
      id: 'even_spacing',
      delta: 90,
      reason: `Evenly spaced young-wallet activity (CV ${features.intervalCoefficientOfVariation.toFixed(2)})`,
    };
  }

  if (features.txCount >= 5 && features.intervalCoefficientOfVariation > 0.8) {
    return {
      id: 'even_spacing',
      delta: 40,
      reason: 'Irregular organic spacing between transactions',
    };
  }

  return {
    id: 'even_spacing',
    delta: 0,
    reason: 'Insufficient spacing pattern signal',
  };
});
