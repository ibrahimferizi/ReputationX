import { defineSignal } from './base.js';

export const balanceSignal = defineSignal('balance', (features) => {
  const { balanceSol } = features;
  let delta = 0;

  if (balanceSol >= 10) {
    delta = 60;
  } else if (balanceSol >= 1) {
    delta = 35;
  } else if (balanceSol >= 0.1) {
    delta = 15;
  } else if (balanceSol > 0) {
    delta = 5;
  }

  return {
    id: 'balance',
    delta,
    reason: `SOL balance ${balanceSol.toFixed(4)}`,
  };
});
