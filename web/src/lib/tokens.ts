import ethLogo from "cryptocurrency-icons/svg/color/eth.svg";
import usdcLogo from "cryptocurrency-icons/svg/color/usdc.svg";
import wbtcLogo from "cryptocurrency-icons/svg/color/wbtc.svg";

export type Address = `0x${string}`;

export interface Token {
  /** Stable key: "native" for ETH, otherwise the lowercase contract address. */
  id: string;
  symbol: string;
  name: string;
  decimals: number;
  /** Null for the chain's native coin. */
  address: Address | null;
  /** Bundled image URL. Tokens without one get a lettered badge. */
  logo?: string;
  /** Added by the user rather than shipped with the app. */
  custom?: boolean;
}

export const NATIVE_ID = "native";

/**
 * Tokens the app knows on devnet 1337. The test tokens are deployed through the CREATE2 factory
 * by tools/devnet/deploy-tokens.sh, so these addresses survive devnet resets.
 */
export const KNOWN_TOKENS: Token[] = [
  { id: NATIVE_ID, symbol: "ETH", name: "Ether", decimals: 18, address: null, logo: ethLogo },
  {
    id: "0xd1108cbf3c5e7b44d7a58d4b2cda492a0ddf6a72",
    symbol: "USDC",
    name: "USD Coin",
    decimals: 6,
    address: "0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72",
    logo: usdcLogo,
  },
  {
    id: "0xb212346871a9a08494e165f5aefe655f06a5b75f",
    symbol: "WBTC",
    name: "Wrapped Bitcoin",
    decimals: 8,
    address: "0xb212346871a9a08494e165F5AeFe655f06A5B75F",
    logo: wbtcLogo,
  },
];

export function isAddress(text: string): text is Address {
  return /^0x[0-9a-fA-F]{40}$/.test(text);
}
