#!/usr/bin/env node
/**
 * libsodium-wrappers-sumo's ESM build imports `./libsodium-sumo.mjs` as a
 * sibling, but the file lives in the separate `libsodium-sumo` package.
 * Vitest's node resolver doesn't follow vite plugins, so we materialize a
 * symlink at the expected location. Re-run on every install.
 */

import { createRequire } from "node:module";
import { existsSync, symlinkSync, unlinkSync, statSync } from "node:fs";
import path from "node:path";

const require_ = createRequire(import.meta.url);

function findWrappersDist() {
  // `require_.resolve("libsodium-wrappers-sumo")` returns the CJS entry
  // inside `dist/modules-sumo/`. Walk up to `dist/`.
  const wrappersEntry = require_.resolve("libsodium-wrappers-sumo");
  return path.dirname(path.dirname(wrappersEntry));
}

function findSumoEsm() {
  const sumoEntry = require_.resolve("libsodium-sumo");
  const sumoDir = path.dirname(sumoEntry);
  // sumoEntry is typically modules-sumo/libsodium-sumo.js; we want the
  // ESM cousin two dirs over.
  return path.join(sumoDir, "..", "modules-sumo-esm", "libsodium-sumo.mjs");
}

try {
  const wrappersDist = findWrappersDist();
  const esmDir = path.join(wrappersDist, "modules-sumo-esm");
  const target = path.join(esmDir, "libsodium-sumo.mjs");

  if (existsSync(target)) {
    const st = statSync(target);
    if (st.size > 0) {
      // Already present (real file or working symlink).
      process.exit(0);
    }
    unlinkSync(target);
  }

  const sumo = findSumoEsm();
  if (!existsSync(sumo)) {
    console.warn(`postinstall: libsodium-sumo ESM file not found at ${sumo}`);
    process.exit(0);
  }
  symlinkSync(sumo, target);
  console.log("postinstall: linked libsodium-sumo.mjs");
} catch (err) {
  console.warn("postinstall: libsodium link skipped —", err.message);
}
