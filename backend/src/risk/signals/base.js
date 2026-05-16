/**
 * @typedef {import('../features.js').WalletFeatures} WalletFeatures
 *
 * @typedef {object} RiskSignalResult
 * @property {string} id Unique signal identifier.
 * @property {number} delta Score adjustment (positive = safer / more established).
 * @property {string} reason Human-readable explanation for debugging.
 */

/**
 * @typedef {object} RiskSignal
 * @property {string} id
 * @property {(features: WalletFeatures) => RiskSignalResult} evaluate
 */

/** @param {string} id @param {(features: WalletFeatures) => RiskSignalResult} evaluate */
export function defineSignal(id, evaluate) {
  return { id, evaluate };
}
