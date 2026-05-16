import { PublicKey } from '@solana/web3.js';
import {
  MAX_SIGNATURES,
  SIGNATURE_PAGE_SIZE,
} from '../config.js';
import { getConnection } from './client.js';

/**
 * @typedef {object} WalletActivity
 * @property {string} address
 * @property {number} balanceLamports
 * @property {number} balanceSol
 * @property {import('@solana/web3.js').ConfirmedSignatureInfo[]} signatures
 * @property {number[]} txTimestampsSec Sorted ascending (unix seconds).
 * @property {number} txCount
 * @property {boolean} signaturesCapped True when fetch hit MAX_SIGNATURES.
 */

/**
 * @param {string} address Base58 wallet address.
 * @returns {Promise<WalletActivity>}
 */
export async function fetchWalletActivity(address) {
  const connection = getConnection();
  const pubkey = new PublicKey(address);

  const [balanceLamports, signatures] = await Promise.all([
    connection.getBalance(pubkey),
    fetchAllSignatures(connection, pubkey),
  ]);

  const txTimestampsSec = signatures
    .map((entry) => entry.blockTime)
    .filter((time) => typeof time === 'number')
    .sort((a, b) => a - b);

  return {
    address,
    balanceLamports,
    balanceSol: balanceLamports / 1e9,
    signatures,
    txTimestampsSec,
    txCount: txTimestampsSec.length,
    signaturesCapped: signatures.length >= MAX_SIGNATURES,
  };
}

/**
 * @param {import('@solana/web3.js').Connection} connection
 * @param {PublicKey} pubkey
 */
async function fetchAllSignatures(connection, pubkey) {
  const collected = [];
  let before;

  while (collected.length < MAX_SIGNATURES) {
    const limit = Math.min(SIGNATURE_PAGE_SIZE, MAX_SIGNATURES - collected.length);
    const batch = await connection.getSignaturesForAddress(pubkey, {
      before,
      limit,
    });

    if (batch.length === 0) {
      break;
    }

    collected.push(...batch);
    before = batch[batch.length - 1].signature;

    if (batch.length < limit) {
      break;
    }
  }

  return collected;
}
