import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";

// Single self-contained dist/index.html, embedded verbatim into the daemon
// binary by src/build/client.rs — see that file for the fallback chain.
export default defineConfig({
  base: "/client/",
  plugins: [react(), viteSingleFile()],
  build: {
    outDir: "dist",
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    cssCodeSplit: false,
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/api/schema.gen.ts", "src/test/**", "src/**/*.test.{ts,tsx}"],
    },
  },
});
