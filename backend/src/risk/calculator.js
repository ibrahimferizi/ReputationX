import { extractFeatures } from './features.js';
import { riskSignals } from './signals/index.js';
import { scoreToTier } from './tiers.js';

const BASE_SCORE = 200;

/**
 * @typedef {object} RiskAssessment
 * @property {number} reputation_score
 * @property {string} tier
 * @property {string} celestila_tier
 * @property {import('./signals/base.js').RiskSignalResult[]} signals
 * @property {import('./features.js').WalletFeatures} features
 */

/**
 * @param {import('../solana/walletActivity.js').WalletActivity} activity
 * @returns {RiskAssessment}
 */
export function calculateRisk(activity) {
  const features = extractFeatures(activity);
  const signalResults = riskSignals.map((signal) => signal.evaluate(features));
  const rawScore =
    BASE_SCORE + signalResults.reduce((sum, result) => sum + result.delta, 0);
  const { score, tier, celestila_tier } = scoreToTier(rawScore);

  return {
    reputation_score: score,
    tier,
    celestila_tier,
    signals: signalResults,
    features,
  };
}
