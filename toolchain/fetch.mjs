// Fetches the release of the investigation's toolchain that toolchain/release.json pins, into
// target/toolchain/release/<tag>, where toolchain/pack.mjs finds it: each file checked against
// its sha256 there, so what is packed is what was pinned, wherever it came from. A file already
// there and right is not fetched again. CI runs it before web/build.sh; on a machine with the
// investigation's checkout beside this one, pack.mjs takes the release from there instead.
//   node toolchain/fetch.mjs
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const pinned = JSON.parse(fs.readFileSync(path.join(here, "release.json"), "utf8"));
const to = path.join(here, "../target/toolchain/release", pinned.tag);
const sha = (b) => createHash("sha256").update(b).digest("hex");
fs.mkdirSync(to, { recursive: true });
for (const [name, want] of Object.entries(pinned.files)) {
  const file = path.join(to, name);
  if (fs.existsSync(file) && sha(fs.readFileSync(file)) === want) continue;
  const res = await fetch(new URL(name, pinned.url));
  if (!res.ok) throw new Error(`${name}: HTTP ${res.status} from ${pinned.url}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  if (sha(bytes) !== want) throw new Error(`${name} is not the file toolchain/release.json pins`);
  fs.writeFileSync(file, bytes);
  console.log(`${name}: ${(bytes.length / 2 ** 20).toFixed(1)} MiB`);
}
console.log(`the toolchain's release ${pinned.tag} is in ${path.relative(process.cwd(), to)}`);
