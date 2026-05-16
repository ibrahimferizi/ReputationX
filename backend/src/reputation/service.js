import { PublicKey } from '@solana/web3.js';
import { fetchWalletActivity } from '../solana/walletActivity.js';
import { calculateRisk } from '../risk/calculator.js';

/**
 * Build the API response expected by the frontend (Identity Prism shape).
 * @param {string} address
 */
export async function getWalletReputation(address) {
  validateAddress(address);

  const activity = await fetchWalletActivity(address);
  const assessment = calculateRisk(activity);
  const { features } = assessment;

  return {
    address,
    reputation_score: assessment.reputation_score,
    tier: assessment.tier,
    celestila_tier: assessment.celestila_tier,
    balance: {
      sol: round(activity.balanceSol, 4),
    },
    tx_stats: {
      count: features.txCount,
    },
    wallet_age: {
      days: Math.floor(features.walletAgeDays),
    },
    nft_stats: {
      count: 0,
    },
    defi_exposure: {
      total_usd: 0,
    },
    _meta: {
      source: 'on-chain',
      signatures_capped: features.signaturesCapped,
      signals: assessment.signals,
    },
  };
}

function validateAddress(address) {
  try {
    new PublicKey(address);
  } catch {
    throw new Error('Invalid Solana wallet address');
  }
}

function round(value, decimals) {
  const factor = 10 ** decimals;
  return Math.round(value * factor) / factor;
}
