// After Trunk: adds the Content-Security-Policy to web/dist/_headers. Trunk writes inline scripts
// into index.html (the one that starts the app names its hashed files, so it changes with every
// build), and so do we (the loading text, and its style): each is allowed by its SHA-256, so the
// policy needs no 'unsafe-inline'. Run by web/build.sh: node web/csp.mjs [dist].
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dist = process.argv[2] ?? path.join(path.dirname(fileURLToPath(import.meta.url)), "dist");
const html = fs.readFileSync(path.join(dist, "index.html"), "utf8");
const hashes = tag => [...html.matchAll(new RegExp(`<${tag}(?:\\s[^>]*)?>([\\s\\S]*?)</${tag}>`, "g"))]
  .filter(m => !/\ssrc=/.test(m[0].slice(0, m[0].indexOf(">"))))
  .map(m => `'sha256-${crypto.createHash("sha256").update(m[1]).digest("base64")}'`);
const scripts = hashes("script"), styles = hashes("style");

const policy = [
  "default-src 'self'",
  // WebAssembly is compiled from the app's own file; nothing else is evaluated.
  `script-src 'self' 'wasm-unsafe-eval' ${scripts.join(" ")}`,
  `style-src 'self' ${styles.join(" ")}`,
  // The workers start from worker.js and import the app's script, both from 'self'.
  "worker-src 'self'",
  // Pictures, the PDF and downloads are made in the page, as blob: URLs.
  "img-src 'self' blob: data:",
  "font-src 'self'",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'none'",
  "frame-ancestors 'none'",
  "form-action 'none'",
].join("; ");

const file = path.join(dist, "_headers");
const headers = fs.readFileSync(file, "utf8");
if (!headers.includes("\n/*\n")) throw new Error(`${file} has no /* rule to add the policy to`);
fs.writeFileSync(file, headers.replace("\n/*\n", `\n/*\n  Content-Security-Policy: ${policy}\n`));
console.log(`Content-Security-Policy: ${scripts.length} inline scripts, ${styles.length} styles allowed by hash`);
