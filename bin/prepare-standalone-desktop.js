#!/usr/bin/env node
"use strict";

// Prepares the Next.js standalone output for Tauri desktop bundling.
// Copies public/ and .next/static/ into the standalone directory so the
// bundled server can serve them without the full node_modules.

// eslint-disable-next-line @typescript-eslint/no-require-imports
const fs = require("fs");
// eslint-disable-next-line @typescript-eslint/no-require-imports
const path = require("path");

const root = path.join(__dirname, "..");
const standaloneDir = path.join(root, ".next", "standalone");
const standaloneServer = path.join(standaloneDir, "server.js");

// --- 1. Verify standalone output exists ---
if (!fs.existsSync(standaloneServer)) {
  console.error(
    "Error: .next/standalone/server.js not found.\n" +
    "Run `next build` first (standalone output is required)."
  );
  process.exit(1);
}

// --- 2. Copy public/ ---
const srcPublic = path.join(root, "public");
const destPublic = path.join(standaloneDir, "public");

if (fs.existsSync(srcPublic)) {
  fs.cpSync(srcPublic, destPublic, { recursive: true });
  console.log("Copied public/ -> .next/standalone/public/");
} else {
  console.warn("Warning: public/ not found, skipping.");
}

// --- 3. Copy .next/static/ ---
const srcStatic = path.join(root, ".next", "static");
const destStatic = path.join(standaloneDir, ".next", "static");

if (fs.existsSync(srcStatic)) {
  fs.cpSync(srcStatic, destStatic, { recursive: true });
  console.log("Copied .next/static/ -> .next/standalone/.next/static/");
} else {
  console.warn("Warning: .next/static/ not found, skipping.");
}

console.log("Standalone desktop preparation complete.");
