import { ArrowRight, ArrowUpRight, Github } from "lucide-react";
import { type CSSProperties, type ReactNode, useEffect, useRef, useState } from "react";
import { Wordmark } from "../components/ui";
import Lenis from "lenis";
import "lenis/dist/lenis.css";
import type { Bench } from "./bench";
import "./landing.css";

const REPO = "https://github.com/SaiKiranMatta/qday";
const blob = (path: string) => `${REPO}/blob/main/${path}`;
const tree = (path: string) => `${REPO}/tree/main/${path}`;
const APP = "/app";

/** The parts laid out on the bench, numbered as on a parts sheet. */
const PARTS = [
  { id: "bezel", n: "01", name: "Bezel" },
  { id: "tray", n: "02", name: "Tray" },
  { id: "esp32", n: "03", name: "ESP32-S3" },
  { id: "screen", n: "04", name: "2.8-inch screen" },
  { id: "keys", n: "05", name: "Keys" },
  { id: "breadboard", n: "06", name: "Breadboard" },
  { id: "spacer", n: "07", name: "Key spacer" },
  { id: "stylus", n: "08", name: "Stylus" },
] as const;

/** How it works, told on the bench as the page scrolls; each step owns a slice of the progress. */
const STEPS: { n?: string; title: string; body: string; from: number; to: number }[] = [
  {
    title: "A hardware signer. An Ethereum account. A different trust model.",
    body: "QDay is a physical device: an ESP32-S3 with a touchscreen and four buttons, in a case printed from code. The device signs. An Ethereum account verifies.",
    from: 0.07,
    to: 0.47,
  },
  {
    n: "01",
    title: "Create",
    body: "Your device generates its keys from a seed. Signing happens on the hardware, not through a key stored in your browser.",
    from: 0.47,
    to: 0.56,
  },
  {
    n: "02",
    title: "Review",
    body: "Check the transaction on the device's own screen, then approve it with the physical buttons.",
    from: 0.56,
    to: 0.67,
  },
  {
    n: "03",
    title: "Sign",
    body: "After your passcode, the device produces a post-quantum signature and sends it to the wallet over USB.",
    from: 0.67,
    to: 0.84,
  },
  {
    n: "04",
    title: "Verify",
    body: "The Ethereum account verifies the signature on-chain and executes the transaction if it's valid. No ECDSA signature is involved at any point.",
    from: 0.84,
    to: 1.01,
  },
];

const clamp01 = (t: number) => Math.min(1, Math.max(0, t));

function useLiveBlock(): number | null {
  const [block, setBlock] = useState<number | null>(null);
  useEffect(() => {
    let stopped = false;
    let timer = 0;
    const read = async () => {
      try {
        const { client } = await import("../lib/chain");
        const next = Number(await client.getBlockNumber());
        if (!stopped) setBlock(next);
      } catch {
        if (!stopped) setBlock(null);
      }
      if (!stopped) timer = window.setTimeout(read, 6_000);
    };
    void read();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }, []);
  return block;
}

/** Marks an element once it has scrolled into view, so its diagrams can draw themselves in. */
function useInView<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry?.isIntersecting) {
          el.dataset.inview = "true";
          observer.disconnect();
        }
      },
      { rootMargin: "0px 0px -15% 0px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return ref;
}

function PrimaryLink({ href, children, tone = "light" }: { href: string; children: ReactNode; tone?: "light" | "dark" }) {
  return (
    <a href={href} className={`lp-focus lp-btn ${tone === "light" ? "lp-btn-primary" : "lp-btn-dark"}`}>
      {children}
      <ArrowRight aria-hidden />
    </a>
  );
}

function TextLink({ href, children, external = true }: { href: string; children: ReactNode; external?: boolean }) {
  return (
    <a
      href={href}
      {...(external ? { target: "_blank", rel: "noreferrer" } : {})}
      className="lp-textlink lp-focus inline-flex items-center gap-1"
    >
      {children}
      {external && <ArrowUpRight className="size-3.5 opacity-60" aria-hidden />}
    </a>
  );
}

function Nav() {
  const links: [string, string][] = [
    ["The problem", "#problem"],
    ["What exists today", "#today"],
    ["What QDay solves", "#solves"],
    ["How it works", "#how"],
  ];
  return (
    <header className="fixed inset-x-0 top-0 z-30">
      <div className="lp-nav-scrim pointer-events-none absolute inset-x-0 top-0 h-24" />
      <nav className="lp-wrap relative flex h-16 items-center gap-8" aria-label="Main">
        <a href="/" className="lp-focus rounded-md" aria-label="QDay home">
          <Wordmark className="text-[24px] text-alabaster" />
        </a>
        <div className="hidden items-center gap-6 text-[13.5px] text-alabaster/70 lg:flex">
          {links.map(([label, href]) => (
            <a key={href} className="lp-link" href={href}>
              {label}
            </a>
          ))}
        </div>
        <div className="ml-auto flex items-center gap-5">
          <a href={APP} className="lp-focus lp-btn lp-btn-primary lp-btn-sm">
            Open the wallet
            <ArrowRight aria-hidden />
          </a>
        </div>
      </nav>
    </header>
  );
}

/** The opening: the headline beside QDay's parts on a bench, then how it works as they assemble and sign. */
function BenchStage({ block }: { block: number | null }) {
  const section = useRef<HTMLElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const hero = useRef<HTMLDivElement>(null);
  const rail = useRef<HTMLDivElement>(null);
  const shade = useRef<HTMLDivElement>(null);
  const labels = useRef<Record<string, HTMLDivElement | null>>({});
  const steps = useRef<(HTMLDivElement | null)[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");

  useEffect(() => {
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    let bench: Bench | null = null;
    let cancelled = false;

    const progressNow = () => {
      const el = section.current;
      if (!el) return 0;
      const rect = el.getBoundingClientRect();
      const total = rect.height - window.innerHeight;
      return total > 0 ? clamp01(-rect.top / total) : 0;
    };

    const update = () => {
      const p = progressNow();
      bench?.setProgress(p);
      const out = clamp01((p - 0.015) / 0.05);
      if (hero.current) {
        hero.current.style.opacity = String(1 - out);
        hero.current.style.transform = `translateY(${(-24 * out).toFixed(1)}px)`;
        hero.current.style.visibility = out >= 1 ? "hidden" : "visible";
      }
      let current = -1;
      STEPS.forEach((step, i) => {
        const el = steps.current[i];
        if (!el) return;
        const fadeIn = clamp01((p - step.from) / 0.025);
        const fadeOut = i === STEPS.length - 1 ? 1 : 1 - clamp01((p - (step.to - 0.025)) / 0.025);
        const alpha = Math.min(fadeIn, fadeOut);
        el.style.opacity = String(alpha);
        el.style.transform = `translateY(${((1 - fadeIn) * 16 - (1 - fadeOut) * 16).toFixed(1)}px)`;
        el.style.visibility = alpha <= 0 ? "hidden" : "visible";
        if (p >= step.from) current = i;
      });
      // Phones: the shade under the step text arrives with the steps, so the bench stays clear before.
      shade.current?.style.setProperty("--lp-steps", String(clamp01((p - 0.035) / 0.04)));
      if (rail.current) {
        rail.current.style.opacity = String(clamp01((p - 0.05) / 0.03));
        rail.current.style.setProperty("--lp-step", String(current));
      }
    };

    let ticking = false;
    const onScroll = () => {
      if (ticking) return;
      ticking = true;
      requestAnimationFrame(() => {
        ticking = false;
        update();
      });
    };

    const focus = (p: number) => {
      // Wider screens give the bench its own area, right of the text: centre the scene in it.
      if (window.innerWidth >= 768) return { x: 0.57, y: 0.52 };
      // Phones: below the headline at first, then above the step text.
      return { x: 0.5, y: p < 0.06 ? 0.88 : 0.4 };
    };

    import("./bench")
      .then(({ createBench }) => {
        if (cancelled || !canvas.current) return;
        bench = createBench({
          canvas: canvas.current,
          labels: PARTS.map((part) => ({ id: part.id, element: labels.current[part.id]! })),
          focus,
          reducedMotion,
          onReady: () => setState("ready"),
          onError: () => setState("failed"),
        });
        update();
      })
      .catch(() => setState("failed"));

    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll);
    update();
    return () => {
      cancelled = true;
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
      bench?.dispose();
    };
  }, []);

  return (
    <section ref={section} className="relative h-[560svh]" aria-label="QDay, assembled">
      {/* Where the walkthrough begins: the first step of the assembly. */}
      <div id="walkthrough" className="absolute left-0 top-[34.5svh] h-px w-px" aria-hidden />
      <div className="lp-stage sticky top-0 h-svh overflow-hidden">
        {/* The bench owns the right of the stage on wider screens, so nothing renders behind the text. */}
        <div className="lp-art absolute inset-0">
          <canvas
            ref={canvas}
            className={`absolute inset-0 size-full transition-opacity duration-700 ${state === "failed" ? "opacity-0" : "opacity-100"}`}
            aria-hidden
          />
          {state === "failed" && (
            <img
              src="/landing/case-exploded.png"
              alt="The QDay case pulled apart: bezel, screen, board, keys, breadboard and tray"
              className="absolute left-1/2 top-1/2 w-[min(88%,760px)] -translate-x-1/2 -translate-y-1/2 rounded-3xl opacity-90"
            />
          )}

          {/* Part labels, placed over the parts by the bench. */}
          <div className="pointer-events-none absolute inset-0" aria-hidden>
            {PARTS.map((part) => (
              <div
                key={part.id}
                ref={(el) => {
                  labels.current[part.id] = el;
                }}
                className="lp-label absolute left-0 top-0 opacity-0"
              >
                <span className="text-[11px] tabular-nums text-alabaster/55">{part.n}</span>
                <span className="text-[13px] font-medium text-alabaster">{part.name}</span>
              </div>
            ))}
          </div>

          <div ref={rail} className="lp-rail absolute right-5 top-1/2 z-10 hidden -translate-y-1/2 opacity-0 md:flex" aria-hidden>
            {STEPS.map((step, i) => (
              <span key={step.title} style={{ "--i": i } as CSSProperties} />
            ))}
          </div>

          {state === "loading" && (
            <p className="absolute bottom-6 right-6 z-10 text-xs text-alabaster/50" role="status">
              Laying out the parts…
            </p>
          )}
        </div>
        <div className="lp-grain pointer-events-none absolute inset-0" aria-hidden />
        <div ref={shade} className="lp-stage-shade pointer-events-none absolute inset-0 md:hidden" />

        {/* The text column: the headline first, then how it works, one step at a time. */}
        <div className="lp-copy absolute inset-y-0 z-10">
          <div ref={hero} className="flex h-full flex-col pb-7 pt-24 md:pt-28">
            <div className="md:my-auto">
              <h1 className="lp-display lp-hero-title">A post-quantum hardware wallet for Ethereum.</h1>
              <p className="lp-lede mt-6">
                QDay signs Ethereum transactions using post-quantum cryptography, verified directly on-chain.
              </p>
              <ul className="lp-tags mt-7">
                <li>Post-quantum signatures</li>
                <li>Hardware signing</li>
                <li className={block !== null ? "lp-tag-live" : undefined}>
                  Ethereum devnet
                  {block !== null && (
                    <span className="ml-1.5 text-alabaster/55" title="The hosted devnet's latest block">
                      block <span className="font-mono tabular-nums">{block.toLocaleString("en-US")}</span>
                    </span>
                  )}
                </li>
              </ul>
              <div className="mt-8 flex flex-wrap items-center gap-x-6 gap-y-4">
                <PrimaryLink href={APP}>Open the wallet</PrimaryLink>
                <div className="flex items-center gap-5">
                  <TextLink href={REPO}>Explore the code</TextLink>
                  <TextLink href="#how" external={false}>
                    How it works
                  </TextLink>
                </div>
              </div>
            </div>
          </div>

          <div className="pointer-events-none absolute inset-0">
            {STEPS.map((step, i) => (
              <div
                key={step.title}
                ref={(el) => {
                  steps.current[i] = el;
                }}
                className="invisible absolute inset-x-0 bottom-10 opacity-0 md:bottom-auto md:top-1/2 md:-translate-y-1/2"
              >
                <h2 className={step.n ? "lp-step-title" : "lp-step-title lp-step-title-lead"}>
                  {step.n && <span className="lp-step-n">{step.n}</span>}
                  {step.title}
                </h2>
                <p className="lp-body mt-4">{step.body}</p>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

/** One chapter of the story below the walkthrough: heading pinned on the left, content on the right. */
function Chapter({
  id,
  title,
  lede,
  intro,
  children,
}: {
  id: string;
  title: string;
  lede: string;
  intro?: ReactNode;
  children: ReactNode;
}) {
  const ref = useInView<HTMLElement>();
  return (
    <section ref={ref} id={id} className="lp-chapter">
      <div className="lp-wrap lp-grid">
        <div className="lp-intro">
          <h2 className="lp-chapter-title">{title}</h2>
          <p className="lp-statement mt-5">{lede}</p>
          {intro && <div className="mt-5 grid gap-4">{intro}</div>}
        </div>
        <div className="lp-content">{children}</div>
      </div>
    </section>
  );
}

function Problem() {
  const timeline = [
    { when: "March 2026", what: "Google researchers estimate about 1,200 logical qubits are enough to break secp256k1." },
    { when: "August 2026", what: "Ethereum core developers schedule EIP-8141 frame transactions for the Hegotá upgrade." },
    { when: "2027", what: "Hegotá is expected to ship, and with it, accounts that choose their own signature scheme." },
    { when: "2029", what: "The Ethereum Foundation's target for core post-quantum infrastructure." },
  ];
  return (
    <Chapter
      id="problem"
      title="The problem"
      lede="The cryptography securing your wallet has an expiration date."
      intro={
        <p className="lp-body">
          Ethereum accounts sign with ECDSA. A large enough quantum computer running Shor's algorithm could derive a private
          key from its public key, and an account's public key is revealed by its first transaction. Assets secured by
          today's signatures may not stay secure against tomorrow's computers.
        </p>
      }
    >
      <h3 className="lp-h3">Where things stand</h3>
      <ol className="lp-timeline mt-6">
        {timeline.map((item) => (
          <li key={item.when}>
            <span className="lp-meta">{item.when}</span>
            <p className="lp-body mt-1">{item.what}</p>
          </li>
        ))}
      </ol>
      <p className="lp-meta mt-8">
        Sources: <TextLink href={blob("docs/fact-check.md")}>QDay's fact-check</TextLink>
      </p>
    </Chapter>
  );
}

function Today() {
  const options: [string, string, string][] = [
    ["Hardware wallets", "Keep the private key off your computer.", "The key they protect is still an ECDSA key."],
    ["EIP-7702 delegation", "Gives an existing account smart-contract features.", "The original ECDSA key can still sign."],
    [
      "ERC-4337 smart accounts",
      "Can verify a post-quantum signature in contract code.",
      "A bundler's ECDSA transaction carries each operation, and post-quantum accounts so far keep an ECDSA key beside the new one.",
    ],
    [
      "Post-quantum chains",
      "Have offered hash-based signatures on hardware wallets for years.",
      "On their own chains, not Ethereum.",
    ],
    [
      "Post-quantum stealth addresses",
      "Hide who was paid from a quantum attacker (ERC-8441).",
      "Spending a payment still takes an ECDSA key.",
    ],
  ];
  return (
    <Chapter
      id="today"
      title="What exists today"
      lede="Today's options either keep an ECDSA key in charge, or live outside Ethereum."
    >
      <table className="lp-table w-full">
        <thead>
          <tr>
            <th scope="col">Approach</th>
            <th scope="col">What it gives you</th>
            <th scope="col">What's still exposed</th>
          </tr>
        </thead>
        <tbody>
          {options.map(([name, gives, gap]) => (
            <tr key={name}>
              <th scope="row">{name}</th>
              <td>{gives}</td>
              <td className="lp-gap">{gap}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="lp-meta mt-5">
        Prior art and sources: <TextLink href={blob("docs/fact-check.md")}>items 18, 22 and 23</TextLink>
      </p>
    </Chapter>
  );
}

function Solves() {
  const pillars: [string, string][] = [
    [
      "No ECDSA key anywhere",
      "An EIP-8141 account checks a post-quantum signature itself and pays its own gas. There's no ECDSA owner, and no delegation left behind.",
    ],
    [
      "Signing stays in hardware",
      "An ESP32-S3 decodes every transaction, shows it on its own screen, and signs only after you approve it and enter your passcode.",
    ],
    [
      "Two post-quantum schemes, one seed",
      "Hash-based XMSS, about 120k gas to verify, or ML-DSA-44, NIST's FIPS 204 standard, about 1.25M gas. You switch on the device.",
    ],
    [
      "Private payments, end to end",
      "Share one address. Each payment lands under a new one-time key, and finding it and spending it are both post-quantum.",
    ],
  ];
  return (
    <Chapter
      id="solves"
      title="What QDay solves"
      lede="QDay takes ECDSA out of the path: the device signs with post-quantum keys, and the account accepts nothing else."
    >
      <ol className="lp-pillars">
        {pillars.map(([title, body]) => (
          <li key={title}>
            <h3 className="lp-h3">{title}</h3>
            <p className="lp-body mt-2">{body}</p>
          </li>
        ))}
      </ol>
    </Chapter>
  );
}

const MEASURES: [string, string][] = [
  ["207,094", "29.5 USDC transfer"],
  ["464,530", "Account deployment"],
  ["1.31M", "ML-DSA ETH transfer"],
  ["1.38M", "Private payment claim"],
];

const FRAMES = [
  { frame: "Verify", purpose: "Check the post-quantum signature", gas: 111_607 },
  { frame: "Sender", purpose: "Consume the one-time key", gas: 10_861 },
  { frame: "Sender", purpose: "Execute the transfer", gas: 30_357 },
];

function CodeBlock({ file, href, children }: { file: string; href: string; children: string }) {
  return (
    <figure className="overflow-hidden rounded-2xl bg-surface ring-1 ring-line">
      <figcaption className="flex items-center justify-between gap-4 border-b border-line px-4 py-2.5">
        <span className="font-mono text-[12px] text-alabaster/60">{file}</span>
        <TextLink href={href}>Source</TextLink>
      </figcaption>
      <pre className="overflow-x-auto px-4 py-4 font-mono text-[12.5px] leading-[1.75] text-alabaster/85">
        <code>{children}</code>
      </pre>
    </figure>
  );
}

function How() {
  const largest = Math.max(...FRAMES.map((f) => f.gas));
  return (
    <Chapter
      id="how"
      title="How it works"
      lede="One signature from the device authorizes a whole EIP-8141 frame transaction, and the account verifies it on-chain before anything runs."
    >
      <table className="lp-frames w-full">
        <thead>
          <tr>
            <th scope="col">Frame</th>
            <th scope="col">Purpose</th>
            <th scope="col" className="text-right">
              Gas
            </th>
          </tr>
        </thead>
        <tbody>
          {FRAMES.map((row) => (
            <tr key={row.purpose}>
              <th scope="row">{row.frame}</th>
              <td>
                {row.purpose}
                <span className="lp-gasbar" style={{ "--share": row.gas / largest } as CSSProperties} aria-hidden />
              </td>
              <td className="text-right font-mono tabular-nums">{row.gas.toLocaleString("en-US")}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="lp-meta mt-4">A 29.5 USDC transfer signed on the board. Total transaction gas: 207,094.</p>

      <h3 className="lp-h3 mt-14">A key never signs twice</h3>
      <p className="lp-body mt-3">
        XMSS keys are one-time. The device moves its key counter before it signs, and the account refuses any key below the
        next valid index, even from a cloned device.
      </p>
      <div className="mt-5 grid gap-4">
        <CodeBlock file="On the device · crates/qday-wallet/src/wallet.rs" href={blob("crates/qday-wallet/src/wallet.rs")}>
          {`let counter = self
    .counter
    .advance()
    .map_err(|_| WalletError::CounterFailure)?;`}
        </CodeBlock>
        <CodeBlock file="On Ethereum · contracts/src/QDayFrameAccount.sol" href={blob("contracts/src/QDayFrameAccount.sol")}>
          {`if (index < nextIndex) revert KeyAlreadyUsed();`}
        </CodeBlock>
      </div>

      <h3 className="lp-h3 mt-14">Measured on the devnet</h3>
      <dl className="lp-measures mt-5">
        {MEASURES.map(([value, label]) => (
          <div key={label}>
            <dt>Gas · {label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
      </dl>
      <p className="lp-meta mt-4 leading-relaxed">
        Signed on the device, on QDay's EIP-8141 devnets. 654 Rust, 124 contract and 67 web tests back it. A research
        prototype: not audited, devnet only, and the ESP32-S3 isn't a secure element.
      </p>
      <div className="mt-6 flex flex-wrap gap-x-6 gap-y-3">
        <TextLink href={blob("docs/devnet.md")}>Devnet measurements</TextLink>
        <TextLink href={blob("AGENTS.md")}>Reviewer guide</TextLink>
        <TextLink href={blob("docs/ROADMAP.md")}>Limits and roadmap</TextLink>
      </div>
    </Chapter>
  );
}

function Close() {
  return (
    <section className="lp-wrap pb-10 pt-[clamp(4rem,8vw,6rem)]">
      <div className="chrome lp-close relative overflow-hidden rounded-3xl px-6 py-8 md:px-10 md:py-10">
        <div className="relative z-10 flex flex-col gap-6 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 className="lp-h2 text-white">Try QDay in your browser.</h2>
            <p className="mt-2 text-[14px] text-white/80">No device needed. Test networks only, not audited.</p>
          </div>
          <div className="flex flex-wrap items-center gap-3">
            <PrimaryLink href={APP} tone="dark">
              Open the wallet
            </PrimaryLink>
            <a href={REPO} target="_blank" rel="noreferrer" className="lp-focus lp-btn lp-btn-glass">
              <Github aria-hidden />
              View source
            </a>
          </div>
        </div>
      </div>
      <footer className="mt-8 flex flex-col gap-6 border-t border-line pt-8 text-[13px] text-alabaster/55 md:flex-row md:items-center">
        <div className="flex items-baseline gap-4">
          <Wordmark className="text-2xl text-alabaster" />
          <span>Built at ETHGlobal Mumbai · November 2026</span>
        </div>
        <nav className="flex flex-wrap gap-x-6 gap-y-2 md:ml-auto" aria-label="Footer">
          <a className="lp-link" href={REPO} target="_blank" rel="noreferrer">
            GitHub
          </a>
          <a className="lp-link" href={blob("README.md")} target="_blank" rel="noreferrer">
            README
          </a>
          <a className="lp-link" href={blob("docs/ROADMAP.md")} target="_blank" rel="noreferrer">
            Roadmap
          </a>
          <a className="lp-link" href={APP}>
            Wallet
          </a>
        </nav>
      </footer>
    </section>
  );
}

/** Inertial scrolling: the page eases after the wheel rather than jumping with it. */
function useSmoothScroll() {
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const lenis = new Lenis({ lerp: 0.085, wheelMultiplier: 0.9, anchors: true, autoRaf: true });
    return () => lenis.destroy();
  }, []);
}

export default function Landing() {
  useSmoothScroll();
  const block = useLiveBlock();
  useEffect(() => {
    document.title = "QDay · A post-quantum hardware wallet for Ethereum";
  }, []);
  return (
    <div className="lp min-h-full">
      <a href="#problem" className="lp-skip">
        Skip the walkthrough
      </a>
      <Nav />
      <main>
        <BenchStage block={block} />
        <div className="lp-chapters">
          <Problem />
          <Today />
          <Solves />
          <How />
        </div>
        <Close />
      </main>
    </div>
  );
}
