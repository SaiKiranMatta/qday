import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { Gallery } from "./Gallery";
import { TxPage } from "./TxPage";
import { txHashFromPath } from "./lib/explorer";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

const txHash = txHashFromPath(location.pathname);

createRoot(root).render(
  <StrictMode>
    {txHash ? (
      <TxPage hash={txHash} />
    ) : import.meta.env.DEV && new URLSearchParams(location.search).has("gallery") ? (
      <Gallery />
    ) : (
      <App />
    )}
  </StrictMode>,
);
