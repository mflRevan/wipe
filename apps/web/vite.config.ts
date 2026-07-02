import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Set `base` to e.g. "/wipe/" to deploy under a subpath. Default "/" is fine.
export default defineConfig({
  base: "/",
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
