export const PORT = Number(process.env.PORT) || 3001;

export const SOLANA_RPC_URL =
  process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';

/** Cap signature fetches to avoid RPC timeouts on very active wallets. */
export const MAX_SIGNATURES = Number(process.env.MAX_SIGNATURES) || 2000;

export const SIGNATURE_PAGE_SIZE = 1000;
