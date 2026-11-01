// Serves web/dist as Cloudflare does: with the headers in web/dist/_headers (the two that give the
// page shared memory, the Content-Security-Policy, caching), so tests see what readers get.
// node web/serve.mjs [port]
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "dist");
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".ttf": "font/ttf" };
const port = Number(process.argv[2] ?? 8080);

// The rules of _headers: a path (where `*` matches anything), then its headers, indented.
function rules() {
  const found = [];
  for (const line of fs.readFileSync(path.join(root, "_headers"), "utf8").split("\n")) {
    if (!line.trim() || line.trimStart().startsWith("#")) continue;
    if (!/^\s/.test(line)) {
      const pattern = line.trim().split("*").map(p => p.replace(/[.+?^${}()|[\]\\]/g, "\\$&")).join(".*");
      found.push({ match: new RegExp(`^${pattern}$`), headers: [] });
    } else {
      const at = line.indexOf(":");
      found.at(-1).headers.push([line.slice(0, at).trim(), line.slice(at + 1).trim()]);
    }
  }
  return found;
}

http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split("?")[0]);
  let file = path.join(root, url);
  if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, "index.html");
  if (!file.startsWith(root) || !fs.existsSync(file)) {
    res.writeHead(404);
    return res.end();
  }
  // Every rule that matches adds its headers; one header from several rules is joined by commas.
  const headers = { "Content-Type": types[path.extname(file)] ?? "application/octet-stream" };
  for (const rule of rules().filter(r => r.match.test(url))) {
    for (const [name, value] of rule.headers) headers[name] = headers[name] ? `${headers[name]}, ${value}` : value;
  }
  res.writeHead(200, headers);
  fs.createReadStream(file).pipe(res);
}).listen(port, () => console.log(`Caprice on http://localhost:${port}/`));
