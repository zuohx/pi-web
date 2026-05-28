#!/usr/bin/env node
"use strict";

// Tauri desktop launcher for pi-web.
// Supports two modes:
//   1. Standalone (preferred) — runs .next/standalone/server.js directly
//   2. Legacy fallback — runs `next start`
// Never opens the system browser. Intended to be spawned by Tauri.

// eslint-disable-next-line @typescript-eslint/no-require-imports
const { spawn } = require("child_process");
// eslint-disable-next-line @typescript-eslint/no-require-imports
const path = require("path");
// eslint-disable-next-line @typescript-eslint/no-require-imports
const fs = require("fs");
// eslint-disable-next-line @typescript-eslint/no-require-imports
const { parseArgs } = require("util");

const pkgDir = path.join(__dirname, "..");
const nextDir = path.join(pkgDir, ".next");
const standaloneDir = path.join(nextDir, "standalone");
const standaloneServer = path.join(standaloneDir, "server.js");

const { values: cliArgs } = parseArgs({
  options: {
    port: { type: "string", short: "p" },
    hostname: { type: "string", short: "H" },
  },
  strict: false,
});

const port = cliArgs.port ?? process.env.PORT ?? "30141";
const hostname = cliArgs.hostname ?? process.env.HOSTNAME ?? "127.0.0.1";

if (!fs.existsSync(nextDir)) {
  console.error("Build artifacts not found. Please report this issue.");
  process.exit(1);
}

// --- Choose launch mode ---
const useStandalone = fs.existsSync(standaloneServer);

let child;

if (useStandalone) {
  // Standalone mode: run server.js directly with required env vars.
  // The standalone server expects PORT and HOSTNAME env vars.
  console.log(`[pi-web] Starting standalone server on ${hostname}:${port}`);
  const env = {
    ...process.env,
    PORT: port,
    HOSTNAME: hostname,
    NODE_ENV: process.env.NODE_ENV ?? "production",
    PI_WEB_DESKTOP: "1",
  };

  child = spawn(process.execPath, [standaloneServer], {
    cwd: standaloneDir,
    stdio: "inherit",
    env,
    windowsHide: true,
  });
} else {
  // Legacy fallback: use `next start`.
  console.log(`[pi-web] Standalone not found, falling back to next start on ${hostname}:${port}`);
  let nextBin;
  try {
    nextBin = require.resolve("next/dist/bin/next", { paths: [pkgDir] });
  } catch {
    try {
      const nextPkg = require.resolve("next/package.json", { paths: [pkgDir] });
      nextBin = path.join(path.dirname(nextPkg), "dist", "bin", "next");
    } catch {
      nextBin = path.join(pkgDir, "node_modules", "next", "dist", "bin", "next");
    }
  }

  const nextArgs = ["start", "-p", port, "-H", hostname];

  child = spawn(process.execPath, [nextBin, ...nextArgs], {
    cwd: pkgDir,
    stdio: "inherit",
    env: { ...process.env, PI_WEB_DESKTOP: "1" },
    windowsHide: true,
  });
}

const forwardSignal = (signal) => {
  if (!child.killed) {
    try {
      child.kill(signal);
    } catch {
      // ignore shutdown race
    }
  }
};

process.on("SIGINT", () => forwardSignal("SIGINT"));
process.on("SIGTERM", () => forwardSignal("SIGTERM"));
child.on("exit", (code) => process.exit(code ?? 0));
