import { noHistorySignal } from './noHistory.js';
import { walletAgeSignal } from './walletAge.js';
import { txCountSignal } from './txCount.js';
import { burstActivitySignal } from './burstActivity.js';
import { evenSpacingSignal } from './evenSpacing.js';
import { balanceSignal } from './balance.js';

/** Register new signals here to extend risk scoring. */
export const riskSignals = [
  noHistorySignal,
  walletAgeSignal,
  txCountSignal,
  burstActivitySignal,
  evenSpacingSignal,
  balanceSignal,
];
