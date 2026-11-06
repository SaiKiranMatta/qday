import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Web Serial needs a secure context; localhost counts as one.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: { port: 5173, host: "localhost" },
});
