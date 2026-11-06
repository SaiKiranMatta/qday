// Signature schemes the app knows about. Screens read these descriptors instead of assuming XMSS,
// so adding a scheme means adding an entry here (and its signer and verifier elsewhere).

import type { SchemeId } from "@qday/device";

export type { SchemeId };

export interface SchemeDescriptor {
  id: SchemeId;
  /** Shown in the UI. */
  name: string;
  /** Plain-language family, for badges. */
  family: "hash-based" | "lattice";
  /** Stateful schemes use one-time keys that must never be reused. */
  stateful: boolean;
  /** One-time keys per tree, for stateful schemes. */
  keysPerTree?: number;
  /** Keys at the end of each tree kept for rotation. */
  reservedPerTree?: number;
  signatureBytes: number;
}

export const SCHEMES: Record<SchemeId, SchemeDescriptor> = {
  "xmss-keccak": {
    id: "xmss-keccak",
    name: "XMSS (keccak256)",
    family: "hash-based",
    stateful: true,
    keysPerTree: 1024,
    reservedPerTree: 16,
    signatureBytes: 2500,
  },
  "ml-dsa-44": {
    id: "ml-dsa-44",
    name: "ML-DSA-44",
    family: "lattice",
    stateful: false,
    signatureBytes: 2420,
  },
};
