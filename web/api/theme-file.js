// A theme file as the gallery takes it in (ARCHITECTURE.md R36, `web/api/gallery.js`): a zip of its
// manifest, module, source and picture, as the app writes it (`src/bundle.rs`), read and checked as
// far as a Worker's 10 ms of CPU allow, never running the module. On its own, with no text
// imported, so that the smoke test can check what a real tab sends with the very code the Worker runs.
import { checkModule } from "./wasm-check.js";

// The names of a palette's colours, as the app's `own::with_colours` reads them.
export const COLOURS = ["desk_top", "desk", "dock", "dock_edge", "button", "button_hover", "field", "text", "text_dim", "text_hot", "accent", "on_accent", "caret", "thumb"];
export const WORLD = "caprice:theme@1.0.0";
const TOOLCHAIN = /^[0-9A-Za-z][0-9A-Za-z .+-]{0,79}$/;
// The app's `bundle::PREVIEW_WIDTH`: the picture the gallery shows, as the shelf keeps it.
const PREVIEW_WIDTH = 320;
// The most of a source (`platform::compiler::MOST_SOURCE`) and of a picture.
const MOST_SOURCE = 512 << 10;
const MOST_PREVIEW = 2 << 20;

// What the theme file holds, checked: its manifest's world, name, words, author, palette and
// toolchain; the module's sections; a source; a picture of the gallery's width. The reasons for a
// refusal are said in a sentence.
export async function check(bytes) {
  const parts = await unzip(bytes, { "manifest.json": 64 << 10, "theme.wasm": 1 << 20, "theme.rs": MOST_SOURCE, "preview.png": MOST_PREVIEW });
  if (parts.why) return parts;
  const { "manifest.json": m, "theme.wasm": module, "theme.rs": source, "preview.png": preview } = parts.found;
  if (!m) return { why: "It has no manifest.json" };
  if (!module) return { why: "It has no program (theme.wasm)" };
  if (!source) return { why: "It has no source (theme.rs): the gallery builds every theme again from its source" };
  if (!preview) return { why: "It has no picture (preview.png)" };
  let manifest;
  try {
    manifest = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(m));
  } catch {
    return { why: "Its manifest.json could not be read" };
  }
  if (manifest?.world !== WORLD) return { why: `It is not made for ${WORLD}` };
  const short = (s, n) => (typeof s === "string" ? s.replace(/[\u0000-\u001f\u007f]/g, " ").trim().slice(0, n) : "");
  const name = short(manifest.name, 60);
  if (!name) return { why: "It has no name" };
  const toolchain = typeof manifest.toolchain === "string" ? manifest.toolchain : "";
  if (!TOOLCHAIN.test(toolchain)) return { why: "It does not say which toolchain built it" };
  const colours = {};
  for (const c of COLOURS) {
    const v = manifest.colours?.[c];
    if (typeof v === "string" && /^#[0-9a-fA-F]{6}$/.test(v)) colours[c] = v.toLowerCase();
  }
  if (manifest.colours?.light === "yes" || manifest.colours?.light === "no") colours.light = manifest.colours.light;
  const checked = checkModule(module);
  if (!checked.ok) return { why: `Its program cannot be played: ${checked.why.slice(0, 5).join("; ")}` };
  let code;
  try {
    code = new TextDecoder("utf-8", { fatal: true }).decode(source);
  } catch {
    return { why: "Its source is not text" };
  }
  if (!code.includes("theme!(")) return { why: "Its source is not a theme's" };
  const header = preview.length >= 24 ? new DataView(preview.buffer, preview.byteOffset, 24) : null;
  const png = header && [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => preview[i] === b);
  if (!png || header.getUint32(16) !== PREVIEW_WIDTH || header.getUint32(20) < 64 || header.getUint32(20) > 1000) {
    return { why: `Its picture is not a PNG ${PREVIEW_WIDTH} wide` };
  }
  const theme = {
    name, about: short(manifest.about, 140), author: short(manifest.author, 40) || "A writer", colours, toolchain,
    module: await sha256(module), source: await sha256(source),
  };
  return { theme, preview };
}

// The files of a zip that are wanted, each to at most its size however much the zip says it
// holds; a file named twice, packed another way than stored or deflated, or that unpacks to more
// than it says, is a refusal. Others are passed over unread.
async function unzip(bytes, wanted) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const not = { why: "It is not a Caprice theme file" };
  let end = -1;
  for (let at = bytes.length - 22; at >= Math.max(0, bytes.length - 22 - 65_535); at--) {
    if (view.getUint32(at, true) === 0x06054b50) {
      end = at;
      break;
    }
  }
  if (end < 0) return not;
  const count = view.getUint16(end + 10, true);
  let at = view.getUint32(end + 16, true);
  const found = {};
  const names = new TextDecoder();
  for (let k = 0; k < count; k++) {
    if (at + 46 > end || view.getUint32(at, true) !== 0x02014b50) return not;
    const method = view.getUint16(at + 10, true);
    const packed = view.getUint32(at + 20, true);
    const size = view.getUint32(at + 24, true);
    const [n, e, c] = [view.getUint16(at + 28, true), view.getUint16(at + 30, true), view.getUint16(at + 32, true)];
    const local = view.getUint32(at + 42, true);
    const name = names.decode(bytes.subarray(at + 46, at + 46 + n));
    at += 46 + n + e + c;
    if (!(name in wanted)) continue;
    if (found[name]) return { why: `It holds ${name} twice` };
    if (size > wanted[name]) return { why: `Its ${name} is more than it may be` };
    if (local + 30 > bytes.length || view.getUint32(local, true) !== 0x04034b50) return not;
    const from = local + 30 + view.getUint16(local + 26, true) + view.getUint16(local + 28, true);
    if (from + packed > bytes.length) return not;
    const data = bytes.subarray(from, from + packed);
    const out = method === 0 ? data : method === 8 ? await inflate(data, size) : null;
    if (!out || out.length !== size) return { why: `Its ${name} could not be read` };
    found[name] = out;
  }
  return { found };
}

// Deflated bytes unpacked, to at most `most`: past it, nothing.
async function inflate(data, most) {
  const reader = new Blob([data]).stream().pipeThrough(new DecompressionStream("deflate-raw")).getReader();
  const chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.length;
      if (length > most) {
        await reader.cancel();
        return null;
      }
      chunks.push(value);
    }
  } catch {
    return null;
  }
  const out = new Uint8Array(length);
  let k = 0;
  for (const c of chunks) {
    out.set(c, k);
    k += c.length;
  }
  return out;
}

export async function sha256(bytes) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(b => b.toString(16).padStart(2, "0")).join("");
}

