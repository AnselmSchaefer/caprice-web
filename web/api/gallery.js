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
import { check, WORLD } from "./theme-file.js";
import { body } from "./theme.js";
import { limits } from "./ledger.js";

// A theme's id in the gallery, given by the Ledger.
export const GALLERY_ID = /^[0-9a-f]{16}$/;

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
