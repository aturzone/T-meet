import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { createRequire } from "node:module";
import path from "node:path";

// libsodium-wrappers-sumo imports `./libsodium-sumo.mjs` as a sibling, but
// the file is shipped in the separate `libsodium-sumo` package. Vite/vitest
// otherwise can't resolve the relative import. This plugin redirects the
// request to the right path on disk.
function libsodiumSumoFix(): Plugin {
  const require_ = createRequire(import.meta.url);
  const sumoEntry = require_.resolve("libsodium-sumo");
  const sumoEsm = path.join(
    path.dirname(sumoEntry),
    "..",
    "modules-sumo-esm",
    "libsodium-sumo.mjs",
  );
  return {
    name: "libsodium-sumo-fix",
    enforce: "pre",
    resolveId(source, importer) {
      if (
        source.endsWith("/libsodium-sumo.mjs") &&
        importer &&
        importer.includes("libsodium-wrappers-sumo")
      ) {
        return sumoEsm;
      }
      return null;
    },
  };
}

export default defineConfig({
  plugins: [libsodiumSumoFix(), react()],
  build: {
    outDir: "dist",
    sourcemap: false,
    target: "es2022",
    // libsodium-wrappers-sumo is ~1 MB (300 KB gzipped) — the bulk is the
    // libsodium WASM. This is intentional for E2E chat.
    chunkSizeWarningLimit: 1200,
    rollupOptions: {
      output: {
        manualChunks: {
          react: ["react", "react-dom"],
          libsodium: ["libsodium-wrappers-sumo"],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  test: {
    environment: "jsdom",
    globals: true,
  },
});
