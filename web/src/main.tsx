import { StrictMode, Suspense, lazy } from "react";
import { createRoot } from "react-dom/client";
import { txHashFromPath } from "./lib/explorer";
import "./styles.css";

// The landing page lives at /, the wallet at /app, and the explorer at /tx/<hash>. Each loads on its
// own, so the landing page never pulls in the wallet and the wallet never pulls in the 3D bench.
const Landing = lazy(() => import("./landing/Landing"));
const App = lazy(() => import("./App").then((m) => ({ default: m.App })));
const TxPage = lazy(() => import("./TxPage").then((m) => ({ default: m.TxPage })));
const Gallery = lazy(() => import("./Gallery").then((m) => ({ default: m.Gallery })));

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

const path = location.pathname.replace(/\/+$/, "") || "/";
const txHash = txHashFromPath(location.pathname);
const inApp = path === "/app" || path.startsWith("/app/");

function Page() {
  if (txHash) return <TxPage hash={txHash} />;
  if (inApp && import.meta.env.DEV && new URLSearchParams(location.search).has("gallery")) return <Gallery />;
  if (inApp) return <App />;
  return <Landing />;
}

createRoot(root).render(
  <StrictMode>
    <Suspense fallback={null}>
      <Page />
    </Suspense>
  </StrictMode>,
);
