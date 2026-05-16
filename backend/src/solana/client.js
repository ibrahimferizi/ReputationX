import { Connection } from '@solana/web3.js';
import { SOLANA_RPC_URL } from '../config.js';

let connection;

export function getConnection() {
  if (!connection) {
    connection = new Connection(SOLANA_RPC_URL, 'confirmed');
  }
  return connection;
}
