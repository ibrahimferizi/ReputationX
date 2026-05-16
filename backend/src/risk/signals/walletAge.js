import { defineSignal } from './base.js';

export const walletAgeSignal = defineSignal('wallet_age', (features) => {
  if (!features.hasHistory) {
    return {
      id: 'wallet_age',
      delta: 0,
      reason: 'No on-chain history to measure age',
    };
  }

  const { walletAgeDays } = features;
  let delta;

  if (walletAgeDays >= 365) {
    delta = 320;
  } else if (walletAgeDays >= 180) {
    delta = 260;
  } else if (walletAgeDays >= 90) {
    delta = 200;
  } else if (walletAgeDays >= 30) {
    delta = 120;
  } else if (walletAgeDays >= 7) {
    delta = 60;
  } else {
    delta = 20;
  }

  return {
    id: 'wallet_age',
    delta,
    reason: `Wallet age ${walletAgeDays.toFixed(1)} days`,
  };
});
