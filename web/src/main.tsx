import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { Gallery } from "./Gallery";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");

createRoot(root).render(
  <StrictMode>
    {import.meta.env.DEV && new URLSearchParams(location.search).has("gallery") ? <Gallery /> : <App />}
  </StrictMode>,
);
