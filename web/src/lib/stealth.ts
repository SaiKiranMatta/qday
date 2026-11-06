import type { Hex } from "viem";
import { parseAbiItem } from "viem";
import { GAS_PRICE } from "./chain";

/** A stealth address is 4,160 bytes: the receiving key and the published key together. */
const ADDRESS_HEX_DIGITS = 8320;

export function isStealthAddress(text: string): text is Hex {
  return text.length === ADDRESS_HEX_DIGITS + 2 && /^0x[0-9a-fA-F]+$/.test(text);
}

/** A spend used 1.38M gas, or 1.56M to a new address, on the devnet. */
export const SPEND_GAS = 1_700_000n;
/** Repays whoever submits the spend. It comes out of the note, so the claimer needs no ETH. */
export const SPEND_FEE = SPEND_GAS * GAS_PRICE;
/** The note's balance once it has been spent: type(uint256).max in the pool. */
export const SPENT = 2n ** 256n - 1n;

export const announcement = parseAbiItem("event Announcement(bytes32 indexed keyHash, bytes1 viewTag, bytes ciphertext)");

export const poolAbi = [
  { type: "function", name: "notes", stateMutability: "view", inputs: [{ type: "bytes32" }], outputs: [{ type: "uint256" }] },
  {
    type: "function",
    name: "spend",
    stateMutability: "nonpayable",
    inputs: [{ type: "bytes" }, { type: "bytes" }, { type: "address" }, { type: "uint256" }],
    outputs: [],
  },
] as const;
