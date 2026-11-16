// The gallery of theme programs (ARCHITECTURE.md R33 and R36, THEME_PROGRAMS_PLAN.md step 6). A
// writer sends a theme they made with Claude as its theme file (a zip of its manifest, module,
// source and picture, `src/bundle.rs`); the Worker does only what fits the free plan's 10 ms of
// CPU: it reads the zip, checks the module's sections (`wasm-check.js`, never running it), the
// manifest and the picture, hashes the module and the source, and queues it. Everything else is
// done in the owner's own browser, in Caprice, never trusting what the writer's browser sent but
// the source: the theme is built again from its source with the same toolchain and must be the
// same module byte for byte, played, and looked at. Then the Worker signs it (ed25519 over the
// module's hash and the source's that it worked out itself, the toolchain and the name, the
// message `own::Signed::message` checks), and it is public: listed for every writer signed in,
// its file and picture read through the Ledger, which counts each read before R2 is touched.
//
// The owner is whoever's hashed id is in GALLERY_OWNERS (a var set in the dashboard); the key is
// the secret GALLERY_SIGNING_KEY, the base64 of an ed25519 seed whose public half is the app's
// `own::GALLERY_KEY`. GALLERY_OFF = "1" switches sending and deciding off; the gallery still shows.
import { checkModule } from "./wasm-check.js";
import { body, COLOURS } from "./theme.js";
import { limits } from "./ledger.js";

// A theme's id in the gallery, given by the Ledger.
export const GALLERY_ID = /^[0-9a-f]{16}$/;
const WORLD = "caprice:theme@1.0.0";
const TOOLCHAIN = /^[0-9A-Za-z][0-9A-Za-z .+-]{0,79}$/;
// The app's `bundle::PREVIEW_WIDTH`: the picture the gallery shows, as the shelf keeps it.
const PREVIEW_WIDTH = 320;
// The most of a source (`platform::compiler::MOST_SOURCE`) and of a picture.
const MOST_SOURCE = 512 << 10;
const MOST_PREVIEW = 2 << 20;

const text = (status, message, headers = {}) =>
  new Response(message, { status, headers: { "Content-Type": "text/plain; charset=utf-8", ...headers } });

function refused(answer) {
  return text(answer.error, answer.message, answer.retry ? { "Retry-After": String(answer.retry) } : {});
}

const ledgerOf = env => env.LEDGER.get(env.LEDGER.idFromName("ledger"));
const files = id => ({ theme: `gallery/${id}/theme.caprice-theme`, preview: `gallery/${id}/preview.png` });

// Whether the writer signed in is the site's owner, who decides what the gallery shows.
export function isOwner(env, user) {
  return (env.GALLERY_OWNERS ?? "").split(",").map(u => u.trim()).filter(Boolean).includes(user);
}

// What a writer signed in may ask of the gallery: `rest` is the path after /api/gallery.
export async function gallery(request, env, user, rest) {
  const method = request.method;
  const ledger = ledgerOf(env);
  if (rest === "") {
    if (method === "GET") return answer(await ledger.galleryList());
    if (method === "POST") return publish(request, env, user);
    return text(405, "GET or POST only");
  }
  if (rest === "/mine") return method === "GET" ? answer(await ledger.galleryMine(user)) : text(405, "GET only");
  if (rest === "/waiting") {
    if (!isOwner(env, user)) return text(404, "No such request");
    return method === "GET" ? answer(await ledger.galleryWaiting()) : text(405, "GET only");
  }
  const decided = rest.match(/^\/waiting\/([^/]+)$/);
  if (decided) {
    if (!isOwner(env, user)) return text(404, "No such request");
    if (method !== "POST") return text(405, "POST only");
    if (!GALLERY_ID.test(decided[1])) return text(400, "Not a theme's id");
    return decide(request, env, decided[1]);
  }
  const part = rest.match(/^\/([^/]+)\/(theme|preview)$/);
  if (part) {
    if (method !== "GET") return text(405, "GET only");
    if (!GALLERY_ID.test(part[1])) return text(400, "Not a theme's id");
    return read(env, user, part[1], part[2]);
  }
  return text(404, "No such request");
}

async function answer(a) {
  if (!a.ok) return refused(a);
  const { ok: _, ...rest } = a;
  return Response.json(rest);
}

// A theme sent: read and checked here as far as 10 ms allow, then queued for the owner.
async function publish(request, env, user) {
  if (env.GALLERY_OFF === "1") return text(503, "The gallery takes no themes for now", { "Retry-After": "3600" });
  const l = limits(env);
  const got = await body(request, l.GALLERY_FILE_BYTES);
  if (got.error) return refused(got);
  const checked = await check(got.bytes);
  if (checked.why) return text(422, checked.why);
  const ledger = ledgerOf(env);
  const asked = await ledger.galleryPublish(user, { ...checked.theme, size: got.bytes.length });
  if (!asked.ok) return refused(asked);
  const at = files(asked.id);
  let put = false;
  try {
    await env.STORIES.put(at.theme, got.bytes, { httpMetadata: { contentType: "application/zip" } });
    await env.STORIES.put(at.preview, checked.preview, { httpMetadata: { contentType: "image/png" } });
    put = true;
  } finally {
    if (!put) await env.STORIES.delete([at.theme, at.preview]);
    await ledger.galleryCommit(asked.id, put);
  }
  return Response.json({ id: asked.id, state: "waiting" }, { status: 202 });
}

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

async function sha256(bytes) {
  return [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(b => b.toString(16).padStart(2, "0")).join("");
}

// A theme's file or picture: the Ledger says yes (and counts the read) before R2 is asked. A theme
// in the gallery never changes (a change is another theme), so the browser keeps what it read.
async function read(env, user, id, what) {
  const allowed = await ledgerOf(env).galleryRead(user, id, isOwner(env, user));
  if (!allowed.ok) return refused(allowed);
  const object = await env.STORIES.get(files(id)[what]);
  if (!object) return text(404, "No such theme in the gallery");
  const kept = allowed.state === "public" ? "private, max-age=31536000, immutable" : "no-store";
  return new Response(object.body, { headers: { "Content-Type": what === "theme" ? "application/zip" : "image/png", "Cache-Control": kept } });
}

// What the gallery signs for a theme, as the app checks it (`own::Signed::message`): the hashes are
// the ones the Worker worked out when it came in, never what a browser says.
export function message(t) {
  return `${WORLD}\nmodule ${t.module}\nsource ${t.source}\ntoolchain ${t.toolchain}\nname ${t.name}\n`;
}

// The owner's decision on a theme waiting: `{ publish: true, module }`, with the hash of the module
// their browser built again from its source, which must be the one sent; or `{ publish: false, why }`.
async function decide(request, env, id) {
  if (env.GALLERY_OFF === "1") return text(503, "The gallery takes no themes for now", { "Retry-After": "3600" });
  const got = await body(request, 4096);
  if (got.error) return refused(got);
  let said;
  try {
    said = JSON.parse(new TextDecoder().decode(got.bytes));
  } catch {
    return text(400, "Not a decision");
  }
  const ledger = ledgerOf(env);
  const waiting = await ledger.galleryEntry(id);
  if (!waiting.ok) return refused(waiting);
  const t = waiting.theme;
  if (said?.publish === true) {
    if (said.module !== t.module) return text(409, "The program built again from its source is not the one sent");
    const key = await signingKey(env);
    if (!key) return text(503, "The gallery has no key to sign with");
    const signature = new Uint8Array(await crypto.subtle.sign({ name: "Ed25519" }, key, new TextEncoder().encode(message(t))));
    const done = await ledger.galleryDecide(id, signature.toBase64(), "");
    if (!done.ok) return refused(done);
    return Response.json({ id, state: "public", signature: signature.toBase64() });
  }
  if (said?.publish === false) {
    const why = typeof said.why === "string" ? said.why.replace(/[\u0000-\u001f]/g, " ").trim().slice(0, 200) : "";
    const done = await ledger.galleryDecide(id, null, why || "It was not taken into the gallery");
    if (!done.ok) return refused(done);
    const at = files(id);
    await env.STORIES.delete([at.theme, at.preview]);
    return Response.json({ id, state: "refused" });
  }
  return text(400, "Not a decision");
}

// The gallery's private key, from the secret's 32-byte seed (PKCS #8 for ed25519 is a fixed
// 16-byte header and the seed); none without the secret.
async function signingKey(env) {
  let seed;
  try {
    seed = Uint8Array.fromBase64(env.GALLERY_SIGNING_KEY ?? "");
  } catch {
    return null;
  }
  if (seed.length !== 32) return null;
  const der = new Uint8Array([0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20, ...seed]);
  return crypto.subtle.importKey("pkcs8", der, { name: "Ed25519" }, false, ["sign"]);
}
