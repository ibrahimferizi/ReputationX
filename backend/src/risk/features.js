const SECONDS_PER_DAY = 86_400;
const MS_PER_HOUR = 3_600_000;

/**
 * @typedef {object} WalletFeatures
 * @property {number} walletAgeDays Age from first on-chain tx to now.
 * @property {number} txCount
 * @property {number} balanceSol
 * @property {boolean} hasHistory
 * @property {boolean} signaturesCapped
 * @property {number} meanTxIntervalHours Mean gap between consecutive txs.
 * @property {number} intervalCoefficientOfVariation Std/mean of inter-tx gaps (0 if not enough txs).
 * @property {number} maxTxsPerHour Peak txs in any 1-hour sliding window.
 * @property {number} maxTxsInFirstDay Txs within the first 24h of wallet activity.
 * @property {boolean} isYoungWallet First tx less than 14 days ago.
 * @property {boolean} isBurstActivity Young wallet with a dense tx cluster.
 * @property {boolean} isEvenlySpacedYoung Young wallet with regular spacing and no burst.
 */

/**
 * @param {import('../solana/walletActivity.js').WalletActivity} activity
 * @returns {WalletFeatures}
 */
export function extractFeatures(activity) {
  const nowSec = Math.floor(Date.now() / 1000);
  const { txTimestampsSec, txCount, balanceSol, signaturesCapped } = activity;

  if (txCount === 0) {
    return {
      walletAgeDays: 0,
      txCount: 0,
      balanceSol,
      hasHistory: false,
      signaturesCapped,
      meanTxIntervalHours: 0,
      intervalCoefficientOfVariation: 0,
      maxTxsPerHour: 0,
      maxTxsInFirstDay: 0,
      isYoungWallet: true,
      isBurstActivity: false,
      isEvenlySpacedYoung: false,
    };
  }

  const firstTxSec = txTimestampsSec[0];
  const walletAgeDays = Math.max(0, (nowSec - firstTxSec) / SECONDS_PER_DAY);
  const intervalsHours = computeIntervalHours(txTimestampsSec);
  const meanTxIntervalHours =
    intervalsHours.length > 0
      ? intervalsHours.reduce((sum, value) => sum + value, 0) / intervalsHours.length
      : 0;
  const intervalCoefficientOfVariation = coefficientOfVariation(intervalsHours);
  const maxTxsPerHour = maxTransactionsInWindow(txTimestampsSec, MS_PER_HOUR);
  const maxTxsInFirstDay = countTransactionsBefore(
    txTimestampsSec,
    firstTxSec + SECONDS_PER_DAY
  );

  const isYoungWallet = walletAgeDays < 14;
  const isBurstActivity =
    isYoungWallet && (maxTxsPerHour >= 15 || maxTxsInFirstDay >= 40);
  const isEvenlySpacedYoung =
    isYoungWallet &&
    txCount >= 5 &&
    !isBurstActivity &&
    intervalCoefficientOfVariation <= 0.45 &&
    meanTxIntervalHours >= 0.5;

  return {
    walletAgeDays,
    txCount,
    balanceSol,
    hasHistory: true,
    signaturesCapped,
    meanTxIntervalHours,
    intervalCoefficientOfVariation,
    maxTxsPerHour,
    maxTxsInFirstDay,
    isYoungWallet,
    isBurstActivity,
    isEvenlySpacedYoung,
  };
}

/** @param {number[]} timestampsSec Sorted ascending. */
function computeIntervalHours(timestampsSec) {
  const intervals = [];
  for (let i = 1; i < timestampsSec.length; i += 1) {
    intervals.push((timestampsSec[i] - timestampsSec[i - 1]) / 3600);
  }
  return intervals;
}

/** @param {number[]} values */
function coefficientOfVariation(values) {
  if (values.length < 2) {
    return 0;
  }
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
  if (mean === 0) {
    return 0;
  }
  const variance =
    values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length;
  return Math.sqrt(variance) / mean;
}

/**
 * Peak number of transactions in any fixed window (timestamps in seconds).
 * @param {number[]} timestampsSec
 * @param {number} windowMs
 */
function maxTransactionsInWindow(timestampsSec, windowMs) {
  if (timestampsSec.length === 0) {
    return 0;
  }

  const windowSec = windowMs / 1000;
  let maxCount = 0;
  let left = 0;

  for (let right = 0; right < timestampsSec.length; right += 1) {
    while (timestampsSec[right] - timestampsSec[left] > windowSec) {
      left += 1;
    }
    maxCount = Math.max(maxCount, right - left + 1);
  }

  return maxCount;
}

/** @param {number[]} timestampsSec @param {number} cutoffSec */
function countTransactionsBefore(timestampsSec, cutoffSec) {
  let count = 0;
  for (const timestamp of timestampsSec) {
    if (timestamp <= cutoffSec) {
      count += 1;
    }
  }
  return count;
}
