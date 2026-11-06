// What the wallet screens show about an account. The chain fills this in a later milestone;
// until then real devices show empty states and `?mock` shows sample data.

/** An amount of one token, already formatted for display. */
export interface TokenAmount {
  tokenId: string;
  value: string;
}

export type ActivityKind = "sent" | "received" | "created" | "refresh" | "refused";

export interface ActivityItem {
  id: string;
  kind: ActivityKind;
  amount?: TokenAmount;
  /** Who or what the other side was, already shortened for display. */
  counterparty?: string;
  /** ISO time. */
  at: string;
  status: "confirmed" | "pending" | "failed";
}

export interface AccountView {
  name: string;
  address: `0x${string}`;
  balance: string;
  /** Balance per token id. Tokens missing here show as 0. */
  balances: Record<string, string>;
  activity: ActivityItem[];
}

/** Dev only: `?mock` shows a sample device and account so the layout can be reviewed without hardware. */
export const MOCK = import.meta.env.DEV && new URLSearchParams(location.search).has("mock");

export const MOCK_ACCOUNT: AccountView = {
  name: "Savings",
  address: "0x7099797051812dc3a010c7d01b50e0d17dc79c8a",
  balance: "1.2840",
  balances: {
    native: "1.2840",
    "0xd1108cbf3c5e7b44d7a58d4b2cda492a0ddf6a72": "2,500.00",
    "0xb212346871a9a08494e165f5aefe655f06a5b75f": "0.0420",
  },
  activity: [
    { id: "a1", kind: "sent", amount: { tokenId: "native", value: "0.25" }, counterparty: "0x8943…a776", at: "2026-11-06T14:12:00Z", status: "pending" },
    { id: "a2", kind: "received", amount: { tokenId: "0xd1108cbf3c5e7b44d7a58d4b2cda492a0ddf6a72", value: "500.00" }, counterparty: "0x66E5…E85f", at: "2026-11-06T11:40:00Z", status: "confirmed" },
    { id: "a3", kind: "sent", amount: { tokenId: "native", value: "0.004" }, counterparty: "0x0b8B…D3D3", at: "2026-11-06T09:02:00Z", status: "confirmed" },
    { id: "a4", kind: "refused", amount: { tokenId: "native", value: "0.64" }, counterparty: "0x7f3a…19c2", at: "2026-11-05T20:31:00Z", status: "failed" },
    { id: "a5", kind: "refresh", at: "2026-11-05T18:05:00Z", status: "confirmed" },
    { id: "a6", kind: "received", amount: { tokenId: "native", value: "1.50" }, counterparty: "0x51B7…3A52", at: "2026-11-05T10:15:00Z", status: "confirmed" },
  ],
};

export const EMPTY_ACCOUNT: AccountView = { name: "Account 1", address: "0x", balance: "—", balances: {}, activity: [] };
