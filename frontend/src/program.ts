import { Connection, PublicKey } from "@solana/web3.js";

export const PROGRAM_ID = new PublicKey(import.meta.env.VITE_PROGRAM_ID);

export function get_connection(): Connection {
  return new Connection(import.meta.env.VITE_RPC_URL || "http://127.0.0.1:8899");
}