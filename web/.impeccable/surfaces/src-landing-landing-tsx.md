---
version: 1
slug: "src-landing-landing-tsx"
primary_target: "src/landing/Landing.tsx"
related_targets: ["src/main.tsx","src/landing/landing.css","src/landing/bench.ts"]
---

# Landing page (/)

Scope: the public landing page at `/`; the wallet moves to `/app`, explorer stays at `/tx/<hash>`.
Mode: Persuade. Audience: ETHGlobal judges and reviewers. Action: open the wallet (demo needs no device).
Copy: the team's own product copy (hero, why, how it works, schemes, one-time keys, private payments,
measurements with EIP-8141 frames, hardware, open source, limits, final call to action), checked for
accuracy. Type: Schibsted Grotesk for display and reading, Fragment Mono for hashes, gas and code; the
qday wordmark stays Inter Black Italic.
Proof on hand: real case STLs and OpenSCAD source, device screen renders, board-signed gas numbers,
live hosted chain, test counts. Constraints: wallet theme, scoped honest claims, no invented photos.

## Direction contract

THESIS: The wallet is a physical object you can watch being built: the page opens on QDay's real printed parts laid out on a bench and assembles them into a working device that signs. It refuses the category hero of a floating product render over a gradient with a feature-card grid below.

OWN-WORLD: Obsidian Chrome. Onyx #0a0a0a ground, Blue Slate #536878 light, Alabaster Grey #e5e4e2 ink; the stage is a chrome sweep laid out like the wallet's .chrome-wide cards (onyx under the text, slate behind the device, alabaster light in the far corner) with the brand's film grain, and the parts are rendered in the same three colours on a floor that only catches their shadows. Surfaces #141517, hairlines from alabaster at 10%. Schibsted Grotesk for display and reading, tabular numerals for numbering and dates; Fragment Mono only for hex, gas, block numbers and code; Inter Black Italic for the qday wordmark alone.

STORY: Visitors see real hardware, understand that it signs Ethereum with post-quantum keys and no ECDSA, check the numbers, and open the wallet.

FIRST VIEWPORT: A text column on the left, about 42% wide: the headline "Your keys shouldn't depend on cryptography that quantum computers can break.", a lede, three tags with the live devnet block, Open the wallet (primary) with Explore the code and How it works as text links. The bench mat with the numbered parts fills the right and fades in from the column's edge.

FORM: Assembly bench, position 7 of 7 on the ordered list, seed key 312ace5c. Signature interaction: scroll assembles the parts, the screen powers on, a review appears, key 4 presses, Signed.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
