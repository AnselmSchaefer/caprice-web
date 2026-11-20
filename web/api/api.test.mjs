// The stories API, run in the Workers runtime itself (Miniflare: R2 and the Durable Object in
// memory), signing in against stand-in Google and GitHub. Each limit is shown to refuse, and the
// ones that guard the bill to leave R2 untouched when they do.
// node --test api/api.test.mjs   (here in web/; Node 24 takes no folder)
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { deflateRawSync, inflateRawSync } from "node:zlib";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";
import { failure } from "./failure.js";

const SITE = "https://caprice.test";
// The date the Worker is deployed with, so the tests run what readers get.
const DATE = readFileSync(new URL("../wrangler.jsonc", import.meta.url), "utf8").match(/"compatibility_date":\s*"([^"]+)"/)[1];

// The challenge of the last sign-in begun, which the stand-in providers check the verifier against.
let challenge;

// The stand-in Claude: every request it was sent, what it answers (the text it writes and why it
// stops), and a promise it waits for first, when a test wants it still painting.
const claude = { asked: [], writes: "Here it is:\n<svg viewBox=\"0 0 4 3\"><rect/></svg>", stop: "end_turn", hold: null, broke: null };

async function painted(request) {
  const body = await request.json();
  claude.asked.push({ key: request.headers.get("x-api-key"), body });
  if (claude.hold) await claude.hold;
  if (claude.broke) return Response.json({ type: "error", error: { type: "invalid_request_error", message: claude.broke.said } }, { status: claude.broke.status });
  // Thinking first, as Opus answers, then the text in two blocks.
  const content = [{ type: "thinking", thinking: "" }, { type: "text", text: claude.writes.slice(0, 9) }, { type: "text", text: claude.writes.slice(9) }];
  return Response.json({ type: "message", role: "assistant", content, stop_reason: claude.stop, usage: claude.usage ?? { input_tokens: 10, output_tokens: 20 } });
}

function providers(request) {
  const url = new URL(request.url);
  if (url.href === "https://api.anthropic.com/v1/messages") return painted(request);
  const form = request.text().then(t => new URLSearchParams(t));
  const verified = f => createHash("sha256").update(f.get("code_verifier")).digest("base64url") === challenge;
  // A code is "who:<id>:<name>", so a test can sign in as anyone.
  const who = f => { const [, sub, name] = f.get("code").split(":"); return { sub, name }; };
  if (url.href === "https://oauth2.googleapis.com/token") {
    return form.then(f => {
      if (f.get("client_secret") !== "google-secret" || !verified(f)) return Response.json({ error: "invalid_grant" }, { status: 400 });
      const { sub, name } = who(f);
      const claims = Buffer.from(JSON.stringify({ iss: "https://accounts.google.com", aud: "google-app", sub, name })).toString("base64url");
      return Response.json({ id_token: `e30.${claims}.c2ln` });
    });
  }
  if (url.href === "https://github.com/login/oauth/access_token") {
    return form.then(f => {
      if (f.get("client_secret") !== "github-secret" || !verified(f)) return Response.json({ error: "bad_verification_code" });
      return Response.json({ access_token: `token:${f.get("code")}` });
    });
  }
  if (url.href === "https://api.github.com/user") {
    const [, , sub, name] = request.headers.get("Authorization").split(":");
    return Response.json({ id: Number(sub), login: name.toLowerCase(), name });
  }
  return new Response("Not reachable from the test", { status: 502 });
}

// The files the theme prompt is made of (api/theme.js imports them as text, as wrangler bundles them).
const PROMPT = ["theme-sdk/GUIDE.md", "theme-sdk/src/lib.rs", "theme-sdk/wit/theme.wit", "themes/fireflies/theme.rs", "themes/snow/theme.rs", "themes/vineyard/theme.rs"];

// The rooms each test opened, each with the sockets of the devices that came in (`device`).
const opened = new WeakMap();

// With `down`, the Ledger is one that fails every request while told to (`ledger-down.mjs`).
async function start(t, vars = {}, down = false) {
  const mf = new Miniflare(convertV4MiniflareOptions({
    // From the checkout's root: the theme prompt's texts are the SDK's and the themes' own files.
    modulesRoot: fileURLToPath(new URL("../..", import.meta.url)),
    modules: [
      ...[...(down ? ["ledger-down.mjs"] : []), "worker.js", "ledger.js", "room.js", "failure.js", "tally.js", "theme.js", "theme-file.js", "gallery.js", "wasm-check.js"].map(m => ({ type: "ESModule", path: fileURLToPath(new URL(m, import.meta.url)) })),
      ...PROMPT.map(f => ({ type: "Text", path: fileURLToPath(new URL(`../../${f}`, import.meta.url)) })),
    ],
    compatibilityDate: DATE,
    r2Buckets: ["STORIES"],
    durableObjects: { LEDGER: { className: down ? "DownLedger" : "Ledger", useSQLite: true }, ROOMS: { className: "StoryRoom", useSQLite: true } },
    bindings: {
      SESSION_SECRET: "a test's own secret, long enough to sign with",
      GOOGLE_CLIENT_ID: "google-app", GOOGLE_CLIENT_SECRET: "google-secret",
      GITHUB_CLIENT_ID: "github-app", GITHUB_CLIENT_SECRET: "github-secret",
      ANTHROPIC_API_KEY: "a test's key",
      ...vars,
    },
    outboundService: providers,
  }));
  opened.set(mf, new Map());
  t.after(async () => {
    try {
      await everyRoomSleeps(mf);
    } finally {
      await mf.dispose();
    }
  });
  Object.assign(claude, { asked: [], writes: "Here it is:\n<svg viewBox=\"0 0 4 3\"><rect/></svg>", stop: "end_turn", hold: null, broke: null });
  return mf;
}

const cookie = (response, name) => response.headers.getSetCookie().find(c => c.startsWith(`${name}=`))?.split(";")[0];

async function signIn(mf, sub = "1", name = "Ada", provider = "google") {
  const begun = await mf.dispatchFetch(`${SITE}/api/login/${provider}`, { redirect: "manual" });
  assert.equal(begun.status, 302);
  const to = new URL(begun.headers.get("Location"));
  challenge = to.searchParams.get("code_challenge");
  const back = await mf.dispatchFetch(
    `${SITE}/api/callback/${provider}?code=who:${sub}:${name}&state=${to.searchParams.get("state")}`,
    { headers: { Cookie: cookie(begun, "__Host-caprice-signin") }, redirect: "manual" },
  );
  return { back, session: cookie(back, "__Host-caprice") };
}

async function writer(mf, sub, name) {
  const { session } = await signIn(mf, sub, name);
  assert.ok(session, `${name} is signed in`);
  return (path, { method = "GET", body, headers = {} } = {}) => mf.dispatchFetch(`${SITE}${path}`, {
    method, body, redirect: "manual",
    headers: { "X-Caprice": "1", Origin: SITE, Cookie: session, ...headers },
  });
}

const story = (words, size = 0) => JSON.stringify({ version: 5, content: [{ text: words + " ".repeat(size) }] }, null, 2);
const saveNew = (api, id, body, name = "The Hatch") =>
  api(`/api/stories/${id}`, { method: "PUT", body, headers: { "If-None-Match": "*", "X-Caprice-Name": encodeURIComponent(name) } });
const saveOver = (api, id, body, version) =>
  api(`/api/stories/${id}`, { method: "PUT", body, headers: { "If-Match": `"${version}"` } });

async function inR2(mf) {
  const bucket = await mf.getR2Bucket("STORIES");
  return (await bucket.list()).objects.map(o => o.key);
}

test("cloudflare's daily limit is said plainly, with when it lifts, and other faults say only that", () => {
  // Cloudflare's own words, as the Worker's log showed them on 2026-11-10.
  const over = new Error("Exceeded allowed volume of requests in Durable Objects free tier.");
  const at = Date.UTC(2026, 10, 10, 16, 30);
  const said = failure(over, at);
  assert.equal(said.status, 503);
  assert.match(said.said, /used up what its host, Cloudflare, allows it in a day/);
  assert.match(said.said, /until midnight UTC, in about 8 hours/);
  assert.equal(said.retry, 7.5 * 3600, "the app waits until midnight UTC before asking again");
  assert.match(failure(over, Date.UTC(2026, 10, 10, 23, 50)).said, /within the hour/);
  assert.deepEqual(failure(new Error("Unexpected end of JSON input"), at), { status: 500, said: "Something went wrong", retry: null });
  assert.equal(failure(undefined, at).status, 500);
});

test("with the ledger out of reach the writer still signs in and is named, and is recorded once it is back", async t => {
  const mf = await start(t, {}, true);
  const namespace = await mf.getDurableObjectNamespace("LEDGER");
  const ledger = namespace.get(namespace.idFromName("ledger"));
  await ledger.down(true);
  const api = await writer(mf, "7", "Ada");
  const me = await api("/api/me");
  assert.equal(me.status, 200);
  assert.equal((await me.json()).name, "Ada", "the app knows who is signed in");
  const list = await api("/api/stories");
  assert.equal(list.status, 503);
  assert.match(await list.text(), /used up what its host, Cloudflare, allows it in a day/);
  assert.ok(Number(list.headers.get("Retry-After")) > 0);

  await ledger.down(false);
  const back = await api("/api/stories");
  assert.equal(back.status, 200, "once the ledger is back, the writer is signed up and their stories are there");
  const renewed = cookie(back, "__Host-caprice");
  assert.ok(renewed, "the session is renewed as recorded");
  const recorded = await mf.dispatchFetch(`${SITE}/api/stories`, { headers: { "X-Caprice": "1", Origin: SITE, Cookie: renewed } });
  assert.equal(recorded.status, 200);
  assert.equal(cookie(recorded, "__Host-caprice"), undefined, "and is not recorded again");
});

test("a session still to be recorded cannot pass the day's limit on new accounts", async t => {
  const mf = await start(t, { SITE_ACCOUNTS_PER_DAY: "0" }, true);
  const namespace = await mf.getDurableObjectNamespace("LEDGER");
  const ledger = namespace.get(namespace.idFromName("ledger"));
  await ledger.down(true);
  const api = await writer(mf, "8", "Bo");
  await ledger.down(false);
  const list = await api("/api/stories");
  assert.equal(list.status, 503);
  assert.match(await list.text(), /taking no new sign-ups today/);
});

test("signing in with google gives a session that names the writer", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const me = await api("/api/me");
  assert.equal(me.status, 200);
  const { name, provider, user } = await me.json();
  assert.deepEqual({ name, provider }, { name: "Ada", provider: "google" });
  assert.match(user, /^[0-9a-f]{32}$/, "the user's id is a hash, not Google's own id");
});

test("signing in with github works the same", async t => {
  const mf = await start(t);
  const { back, session } = await signIn(mf, "42", "Grace", "github");
  assert.equal(back.headers.get("Location"), "/");
  const me = await mf.dispatchFetch(`${SITE}/api/me`, { headers: { "X-Caprice": "1", Cookie: session } });
  const { name, provider } = await me.json();
  assert.deepEqual({ name, provider }, { name: "Grace", provider: "github" });
});

test("the session cookie is out of reach of scripts and other sites", async t => {
  const mf = await start(t);
  const { back } = await signIn(mf);
  const set = back.headers.getSetCookie().find(c => c.startsWith("__Host-caprice="));
  for (const part of ["Secure", "HttpOnly", "SameSite=Lax", "Path=/"]) assert.ok(set.includes(part), part);
});

test("a sign-in not started here is refused", async t => {
  const mf = await start(t);
  const begun = await mf.dispatchFetch(`${SITE}/api/login/google`, { redirect: "manual" });
  challenge = new URL(begun.headers.get("Location")).searchParams.get("code_challenge");
  const forged = await mf.dispatchFetch(`${SITE}/api/callback/google?code=who:1:Eve&state=guessed`, {
    headers: { Cookie: cookie(begun, "__Host-caprice-signin") }, redirect: "manual",
  });
  assert.equal(forged.status, 400);
  assert.equal(cookie(forged, "__Host-caprice"), undefined);
  const without = await mf.dispatchFetch(`${SITE}/api/callback/google?code=who:1:Eve&state=x`, { redirect: "manual" });
  assert.equal(without.status, 400);
});

test("a sign-in turned back at the provider goes home signed out", async t => {
  const mf = await start(t);
  const back = await mf.dispatchFetch(`${SITE}/api/callback/google?error=access_denied`, { redirect: "manual" });
  assert.equal(back.status, 302);
  assert.equal(back.headers.get("Location"), "/");
  assert.equal(cookie(back, "__Host-caprice"), undefined);
});

test("signing in with a provider that is not set up is not offered", async t => {
  const mf = await start(t, { GITHUB_CLIENT_ID: "" });
  assert.equal((await mf.dispatchFetch(`${SITE}/api/login/github`, { redirect: "manual" })).status, 404);
  const offered = await mf.dispatchFetch(`${SITE}/api/providers`, { headers: { "X-Caprice": "1" } });
  assert.deepEqual(await offered.json(), ["google"]);
});

test("without a session, or with a forged one, nothing is answered", async t => {
  const mf = await start(t);
  const headers = { "X-Caprice": "1" };
  assert.equal((await mf.dispatchFetch(`${SITE}/api/me`, { headers })).status, 401);
  assert.equal((await mf.dispatchFetch(`${SITE}/api/stories`, { headers })).status, 401);
  const { session } = await signIn(mf);
  const [body, mac] = session.slice("__Host-caprice=".length).split(".");
  const claims = JSON.parse(Buffer.from(body, "base64url"));
  const other = Buffer.from(JSON.stringify({ ...claims, u: "someone else" })).toString("base64url");
  const forged = await mf.dispatchFetch(`${SITE}/api/stories`, { headers: { ...headers, Cookie: `__Host-caprice=${other}.${mac}` } });
  assert.equal(forged.status, 401);
});

test("without a session secret nothing is signed or trusted", async t => {
  const mf = await start(t, { SESSION_SECRET: "short" });
  assert.equal((await mf.dispatchFetch(`${SITE}/api/login/google`, { redirect: "manual" })).status, 500);
  const me = await mf.dispatchFetch(`${SITE}/api/me`, { headers: { "X-Caprice": "1", Cookie: "__Host-caprice=e30.e30" } });
  assert.equal(me.status, 500);
});

test("a story saved comes back as it was, and is listed", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const body = story("Once upon a time");
  const saved = await saveNew(api, "story-1", body, "The Lion Boy, second draft");
  assert.equal(saved.status, 200);
  const { version } = await saved.json();
  const back = await api("/api/stories/story-1");
  assert.equal(await back.text(), body);
  assert.equal(back.headers.get("ETag"), `"${version}"`);
  assert.equal(decodeURIComponent(back.headers.get("X-Caprice-Name")), "The Lion Boy, second draft");
  const [listed, ...more] = await (await api("/api/stories")).json();
  assert.equal(more.length, 0);
  assert.equal(listed.id, "story-1");
  assert.equal(listed.name, "The Lion Boy, second draft");
  assert.equal(listed.version, version);
  assert.equal(listed.size, Buffer.byteLength(body));
});

test("one writer cannot reach another's stories", async t => {
  const mf = await start(t);
  const ada = await writer(mf, "1", "Ada");
  const bob = await writer(mf, "2", "Bob");
  const body = story("Ada's words");
  assert.equal((await saveNew(ada, "story-1", body)).status, 200);
  assert.equal((await bob("/api/stories/story-1")).status, 404);
  assert.deepEqual(await (await bob("/api/stories")).json(), []);
  assert.equal((await bob("/api/stories/story-1", { method: "DELETE", headers: { "If-Match": "*" } })).status, 404);
  assert.equal((await saveNew(bob, "story-1", story("Bob's own"))).status, 200);
  assert.equal(await (await ada("/api/stories/story-1")).text(), body);
  assert.equal((await bob("/api/stories", { method: "DELETE" })).status, 204);
  assert.equal(await (await ada("/api/stories/story-1")).text(), body);
});

test("ids that could reach elsewhere are refused", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  for (const id of ["..%2F..%2Fx", "a%2Fb", "Story", "a.b", "x".repeat(65)]) {
    assert.equal((await saveNew(api, id, story("x"))).status, 400, id);
    assert.equal((await api(`/api/stories/${id}`)).status, 400, id);
  }
  // These climb out of /api/stories before the Worker sees them, to paths that are no request.
  for (const path of ["/api/stories/a/b", "/api/stories/%2e%2e"]) {
    assert.equal((await api(path, { method: "PUT", body: story("x"), headers: { "If-None-Match": "*" } })).status, 404, path);
  }
  assert.deepEqual(await inR2(mf), []);
});

test("requests from another site are refused", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  assert.equal((await saveNew(api, "s", story("x"))).status, 200);
  const put = headers => api("/api/stories/t", { method: "PUT", body: story("x"), headers: { "If-None-Match": "*", ...headers } });
  assert.equal((await put({ "X-Caprice": "" })).status, 403);
  assert.equal((await put({ Origin: "https://elsewhere.example" })).status, 403);
  assert.equal((await put({ Origin: "" })).status, 403);
  assert.equal((await api("/api/stories/s", { headers: { "X-Caprice": "" } })).status, 403);
  assert.equal((await api("/api/stories", { method: "DELETE", headers: { Origin: "https://elsewhere.example" } })).status, 403);
  assert.equal((await api("/api/stories/s")).status, 200, "nothing was deleted");
});

test("every answer forbids caching, framing and other sites, and allows no CORS", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  for (const answer of [await api("/api/me"), await api("/api/stories/nothing"), await mf.dispatchFetch(`${SITE}/api/me`)]) {
    assert.equal(answer.headers.get("Cache-Control"), "no-store");
    assert.equal(answer.headers.get("Cross-Origin-Resource-Policy"), "same-origin");
    assert.equal(answer.headers.get("X-Content-Type-Options"), "nosniff");
    assert.equal(answer.headers.get("Access-Control-Allow-Origin"), null);
  }
});

test("a save must say which copy it replaces, and a stale one is refused", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const { version: first } = await (await saveNew(api, "s", story("one"))).json();
  assert.equal((await api("/api/stories/s", { method: "PUT", body: story("two") })).status, 428);
  assert.equal((await saveNew(api, "s", story("two"))).status, 412);
  const { version: second } = await (await saveOver(api, "s", story("two"), first)).json();
  // Another browser still holding the first version.
  assert.equal((await saveOver(api, "s", story("three"), first)).status, 412);
  assert.equal((await saveOver(api, "new", story("x"), first)).status, 412);
  assert.equal(await (await api("/api/stories/s")).text(), story("two"));
  assert.notEqual(first, second);
});

test("only caprice stories, within the size limit, are kept", async t => {
  const mf = await start(t, { FILE_BYTES: "200" });
  const api = await writer(mf, "1", "Ada");
  assert.equal((await saveNew(api, "a", "<html><body>Anything at all</body></html>")).status, 415);
  assert.equal((await saveNew(api, "b", JSON.stringify({ content: [] }))).status, 415);
  assert.equal((await saveNew(api, "c", "")).status, 413);
  assert.equal((await saveNew(api, "d", story("x", 300))).status, 413);
  assert.deepEqual(await inR2(mf), []);
  assert.equal((await saveNew(api, "e", story("fits"))).status, 200);
});

test("saving too often in a minute is refused, and leaves r2 alone", async t => {
  const mf = await start(t, { USER_WRITES_PER_MINUTE: "2" });
  const api = await writer(mf, "1", "Ada");
  const { version } = await (await saveNew(api, "s", story("one"))).json();
  assert.equal((await saveOver(api, "s", story("two"), version)).status, 200);
  const before = await inR2(mf);
  const third = await saveNew(api, "t", story("three"));
  assert.equal(third.status, 429);
  assert.equal(third.headers.get("Retry-After"), "60");
  assert.deepEqual(await inR2(mf), before);
  // Another writer has a minute of their own.
  assert.equal((await saveNew(await writer(mf, "2", "Bob"), "s", story("Bob"))).status, 200);
});

test("a writer's saves for the day run out", async t => {
  const mf = await start(t, { USER_WRITES_PER_DAY: "1" });
  const api = await writer(mf, "1", "Ada");
  assert.equal((await saveNew(api, "s", story("one"))).status, 200);
  const refused = await saveNew(api, "t", story("two"));
  assert.equal(refused.status, 429);
  assert.ok(Number(refused.headers.get("Retry-After")) <= 86_400);
});

test("the site's writes for the day pause everyone, and leave r2 alone", async t => {
  const mf = await start(t, { SITE_WRITES_PER_DAY: "2" });
  const ada = await writer(mf, "1", "Ada");
  const bob = await writer(mf, "2", "Bob");
  assert.equal((await saveNew(ada, "a", story("one"))).status, 200);
  assert.equal((await saveNew(bob, "b", story("two"))).status, 200);
  const before = await inR2(mf);
  const paused = await saveNew(bob, "c", story("three"));
  assert.equal(paused.status, 503);
  assert.ok(Number(paused.headers.get("Retry-After")) > 0);
  assert.deepEqual(await inR2(mf), before);
  assert.equal((await ada("/api/stories/a")).status, 200, "reading goes on");
});

test("the site's reads for the day run out", async t => {
  const mf = await start(t, { SITE_READS_PER_DAY: "1" });
  const api = await writer(mf, "1", "Ada");
  await saveNew(api, "s", story("one"));
  assert.equal((await api("/api/stories/s")).status, 200);
  assert.equal((await api("/api/stories/s")).status, 503);
  assert.equal((await api("/api/stories")).status, 200, "the list is not a read of R2");
});

test("history online keeps the version before, and drops the ones close behind it", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  let { version } = await (await saveNew(api, "s", story("v1"))).json();
  const versions = [version];
  for (const words of ["v2", "v3", "v4"]) {
    ({ version } = await (await saveOver(api, "s", story(words), version)).json());
    versions.push(version);
  }
  const kept = (await inR2(mf)).map(k => k.split("/").at(-1).replace(".caprice", "")).sort();
  assert.deepEqual(kept, versions.slice(2).sort());
});

test("a writer's room drops their oldest history first, then refuses", async t => {
  const size = Buffer.byteLength(story("v1", 1000));
  const mf = await start(t, { USER_BYTES: String(Math.floor(size * 2.5)) });
  const api = await writer(mf, "1", "Ada");
  let { version } = await (await saveNew(api, "a", story("v1", 1000))).json();
  ({ version } = await (await saveOver(api, "a", story("v2", 1000), version)).json());
  assert.equal((await inR2(mf)).length, 2, "the current version and the one before");
  assert.equal((await saveNew(api, "b", story("b1", 1000))).status, 200, "room made by dropping a's history");
  assert.equal((await inR2(mf)).length, 2);
  const full = await saveNew(api, "c", story("c1", 1000));
  assert.equal(full.status, 507);
  assert.equal(await (await api("/api/stories/a")).text(), story("v2", 1000));
  assert.equal(await (await api("/api/stories/b")).text(), story("b1", 1000));
});

test("the site's room is shared by everyone", async t => {
  const size = Buffer.byteLength(story("x", 1000));
  const mf = await start(t, { SITE_BYTES: String(Math.floor(size * 1.5)) });
  assert.equal((await saveNew(await writer(mf, "1", "Ada"), "a", story("x", 1000))).status, 200);
  assert.equal((await saveNew(await writer(mf, "2", "Bob"), "b", story("x", 1000))).status, 507);
  assert.equal((await inR2(mf)).length, 1);
});

test("new accounts for the day run out, but those already here still sign in", async t => {
  const mf = await start(t, { SITE_ACCOUNTS_PER_DAY: "1" });
  assert.ok((await signIn(mf, "1", "Ada")).session);
  const bob = await signIn(mf, "2", "Bob");
  assert.equal(bob.back.status, 503);
  assert.equal(bob.session, undefined);
  assert.ok((await signIn(mf, "1", "Ada")).session);
});

test("a story deleted online is gone from r2, history and all", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  let { version } = await (await saveNew(api, "a", story("v1"))).json();
  ({ version } = await (await saveOver(api, "a", story("v2"), version)).json());
  await saveNew(api, "b", story("b"));
  assert.equal((await api("/api/stories/a", { method: "DELETE", headers: { "If-Match": `"stale"` } })).status, 412);
  assert.equal((await api("/api/stories/a", { method: "DELETE", headers: { "If-Match": `"${version}"` } })).status, 204);
  assert.equal((await api("/api/stories/a")).status, 404);
  assert.equal((await inR2(mf)).length, 1);
  assert.equal((await api("/api/stories", { method: "DELETE" })).status, 204);
  assert.deepEqual(await inR2(mf), []);
  assert.deepEqual(await (await api("/api/stories")).json(), []);
});

test("a story deleted is listed as gone for the writer's other devices, until it is saved again", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const other = await writer(mf, "2", "Bea");
  const { version } = await (await saveNew(api, "a", story("v1"))).json();
  await saveNew(api, "b", story("b"));
  assert.equal((await api("/api/stories/a", { method: "DELETE", headers: { "If-Match": `"${version}"` } })).status, 204);
  const listed = await (await api("/api/stories?gone=1")).json();
  assert.deepEqual(listed.stories.map(s => s.id), ["b"]);
  assert.deepEqual(listed.gone, [{ id: "a", version }], "with the version it was when it went");
  assert.deepEqual((await (await other("/api/stories?gone=1")).json()).gone, [], "only to its own writer");
  assert.deepEqual((await (await api("/api/stories")).json()).map(s => s.id), ["b"], "without asking, the list as before");
  // Written in on another device meanwhile, it comes back, and is no longer gone.
  assert.equal((await saveNew(api, "a", story("v2"))).status, 200);
  assert.deepEqual((await (await api("/api/stories?gone=1")).json()).gone, []);
  const { version: b } = (await (await api("/api/stories?gone=1")).json()).stories.find(s => s.id === "b");
  await api("/api/stories/b", { method: "DELETE", headers: { "If-Match": `"${b}"` } });
  assert.equal((await api("/api/stories", { method: "DELETE" })).status, 204);
  assert.deepEqual((await (await api("/api/stories?gone=1")).json()).gone, [], "deleting them all forgets what went");
});

test("the off switch stops stories online at once", async t => {
  const mf = await start(t, { SYNC_OFF: "1" });
  const api = await writer(mf, "1", "Ada");
  assert.equal((await api("/api/me")).status, 200);
  assert.equal((await saveNew(api, "s", story("x"))).status, 503);
  assert.equal((await api("/api/stories")).status, 503);
  assert.deepEqual(await inR2(mf), []);
});

test("signing out clears the session", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const out = await api("/api/logout", { method: "POST" });
  assert.equal(out.status, 204);
  assert.ok(out.headers.getSetCookie().some(c => c.startsWith("__Host-caprice=;") && c.includes("Max-Age=0")));
});

const paint = (api, prompt = "<scene>\nA lighthouse at dusk\n</scene>\n\nPaint one illustration of this scene") =>
  api("/api/scene", { method: "POST", body: prompt, headers: { "Content-Type": "text/plain; charset=utf-8" } });

test("a scene is painted for a writer signed in, and only the picture comes back", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  const got = await paint(api);
  assert.equal(got.status, 200);
  assert.match(got.headers.get("Content-Type"), /^image\/svg\+xml/);
  const svg = await got.text();
  assert.ok(svg.startsWith("<svg") && svg.endsWith("</svg>"), `nothing of Claude's but the picture: ${svg}`);
  assert.ok(svg.includes("<metadata>Made with AI"), "it says in itself that an AI made it");
  assert.equal(got.headers.get("X-Caprice-Scenes-Left"), "49");

  const [{ key, body }] = claude.asked;
  assert.equal(key, "a test's key");
  assert.equal(body.model, "claude-opus-5-5");
  assert.equal(body.max_tokens, 32000, "the most a scene can cost is the Worker's, not the app's");
  assert.equal(body.messages[0].content, "<scene>\nA lighthouse at dusk\n</scene>\n\nPaint one illustration of this scene");

  const headers = { "X-Caprice": "1", Origin: SITE };
  assert.equal((await mf.dispatchFetch(`${SITE}/api/scene`, { method: "POST", body: "x", headers })).status, 401, "not signed in");
  assert.equal((await mf.dispatchFetch(`${SITE}/api/scene`, { method: "POST", body: "x", headers: { Cookie: (await signIn(mf)).session } })).status, 403, "not from Caprice");
  assert.equal(claude.asked.length, 1, "neither reached Claude");
});

test("the site's scenes for the day run out for everyone, and claude is not asked again", async t => {
  const mf = await start(t, { SITE_SCENES_PER_DAY: "2" });
  const ada = await writer(mf, "1", "Ada");
  const bob = await writer(mf, "2", "Bob");
  assert.equal((await paint(ada)).headers.get("X-Caprice-Scenes-Left"), "1");
  assert.equal((await paint(ada)).headers.get("X-Caprice-Scenes-Left"), "0", "one writer may take them all");
  const refused = await paint(bob);
  assert.equal(refused.status, 503);
  assert.match(await refused.text(), /No more scenes today/);
  assert.ok(Number(refused.headers.get("Retry-After")) <= 86_400, "until tomorrow");
  assert.equal(claude.asked.length, 2);
  assert.equal((await saveNew(bob, "b", story("two"))).status, 200, "stories online go on");
});

test("a painting that fails or is declined still counts, since claude was asked", async t => {
  const mf = await start(t, { SITE_SCENES_PER_DAY: "3" });
  const api = await writer(mf, "1", "Ada");
  claude.stop = "refusal";
  claude.writes = "";
  const declined = await paint(api);
  assert.equal(declined.status, 422);
  assert.equal(declined.headers.get("X-Caprice-Scenes-Left"), "2");
  claude.stop = "max_tokens";
  claude.writes = "<svg viewBox=\"0 0 4 3\"><rect/>";
  const unfinished = await paint(api);
  assert.equal(unfinished.status, 502);
  assert.match(await unfinished.text(), /too big to finish/);
  claude.stop = "end_turn";
  claude.writes = "<svg viewBox=\"0 0 4 3\"><rect/></svg>";
  assert.equal((await paint(api)).headers.get("X-Caprice-Scenes-Left"), "0");
  assert.equal((await paint(api)).status, 503);
});

test("a writer paints one scene at a time", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  let release;
  claude.hold = new Promise(r => { release = r; });
  const first = paint(api);
  while (claude.asked.length === 0) await new Promise(r => setTimeout(r, 10));
  const second = await paint(api);
  assert.equal(second.status, 409, "a second click costs nothing");
  release();
  assert.equal((await first).status, 200);
  assert.equal((await paint(api)).status, 200, "and once it is done, the next may go");
  assert.equal(claude.asked.length, 2);
});

test("too much to paint is refused before anything is counted", async t => {
  const mf = await start(t, { SITE_SCENES_PER_DAY: "1" });
  const api = await writer(mf, "1", "Ada");
  assert.equal((await paint(api, "x".repeat(160_001))).status, 413);
  assert.equal((await paint(api, "  ")).status, 413);
  assert.equal(claude.asked.length, 0);
  assert.equal((await paint(api)).status, 200, "the day's one scene is still there");
});

test("without a key, or switched off, no scene is painted", async t => {
  for (const vars of [{ ANTHROPIC_API_KEY: "" }, { SCENES_OFF: "1" }]) {
    const mf = await start(t, vars);
    const api = await writer(mf, "1", "Ada");
    assert.equal((await paint(api)).status, 503, JSON.stringify(vars));
    assert.equal((await api("/api/me")).status, 200, "the rest goes on");
    assert.equal(claude.asked.length, 0);
  }
});

test("when the credit is used up, painting is paused, and claude being down is said so", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.broke = { status: 400, said: "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits." };
  const paused = await paint(api);
  assert.equal(paused.status, 503);
  assert.equal(await paused.text(), "Painting is paused for now");
  assert.ok(Number(paused.headers.get("Retry-After")) > 0, "the app may try again later");
  claude.broke = { status: 500, said: "Internal server error" };
  const down = await paint(api);
  assert.equal(down.status, 502);
  assert.equal(await down.text(), "Claude could not be asked");
});

test("with the ledger out of reach scenes are still painted, and the day's scenes still bound them", async t => {
  const mf = await start(t, { SITE_SCENES_PER_DAY: "5" }, true);
  const namespace = await mf.getDurableObjectNamespace("LEDGER");
  const ledger = namespace.get(namespace.idFromName("ledger"));
  const ada = await writer(mf, "1", "Ada");
  assert.equal((await paint(ada)).headers.get("X-Caprice-Scenes-Left"), "4", "counted by the ledger");

  await ledger.down(true);
  assert.equal((await ada("/api/stories")).status, 503, "stories online wait for it");
  const got = await paint(ada);
  assert.equal(got.status, 200, "a scene does not");
  assert.ok((await got.text()).startsWith("<svg"));
  assert.equal(got.headers.get("X-Caprice-Scenes-Left"), "3", "counted on from the ledger's count");
  const bob = await writer(mf, "2", "Bob");
  assert.equal((await paint(bob)).status, 200, "one signed in meanwhile, still to be recorded, paints too");

  // Two writers asking for the last scenes at once: no more than there are.
  const asked = await Promise.all([paint(ada), paint(bob), paint(ada)]);
  assert.deepEqual(asked.map(a => a.status).sort(), [200, 200, 503]);
  const refused = await paint(bob);
  assert.equal(refused.status, 503);
  assert.match(await refused.text(), /No more scenes today/);
  assert.ok(Number(refused.headers.get("Retry-After")) <= 86_400, "until tomorrow");
  assert.equal(claude.asked.length, 5, "claude was asked for the day's five, no more");

  await ledger.down(false);
  assert.equal((await ada("/api/stories")).status, 200, "and stories online come back");
});

test("the site's owner sees the scenes of each day by hashed id, and nobody else does", async t => {
  const token = "the owner's token, long enough to be one";
  const mf = await start(t, { ADMIN_TOKEN: token });
  const ada = await writer(mf, "1", "Ada");
  const bob = await writer(mf, "2", "Bob");
  await paint(ada);
  await paint(ada);
  await paint(bob);
  const ask = authorization => mf.dispatchFetch(`${SITE}/api/admin/scenes`, { headers: authorization ? { Authorization: authorization } : {} });
  assert.equal((await ask()).status, 401);
  assert.equal((await ask("Bearer not it")).status, 401);
  const seen = await ask(`Bearer ${token}`);
  assert.equal(seen.status, 200);
  const { limit, days } = await seen.json();
  assert.equal(limit, 50);
  assert.equal(days.length, 1);
  assert.deepEqual([days[0].scenes, days[0].writers], [3, 2]);
  assert.deepEqual(days[0].by.map(w => w.scenes), [2, 1], "most first");
  const me = await (await ada("/api/me")).json();
  assert.equal(days[0].by[0].user, me.user, "by the same hashed id the writer has");
  assert.ok(!JSON.stringify(days).includes("Ada"), "and no name");

  const without = await start(t);
  assert.equal((await without.dispatchFetch(`${SITE}/api/admin/scenes`, { headers: { Authorization: "Bearer " } })).status, 404, "no token, no such request");
});

test("a picture comes back without anything that could run or reach outside it", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = `<svg viewBox="0 0 4 3" onload="alert(1)"><defs><linearGradient id="g"/></defs>
    <script>alert(2)</script><scr<script></script>ipt>alert(3)</script>
    <foreignObject><iframe src="https://elsewhere.test"></iframe></foreignObject>
    <style>@import url(https://elsewhere.test/a.css); rect { fill: url(https://elsewhere.test/x) }</style>
    <a href="javascript:alert(4)"><rect fill="url(#g)" onclick='alert(5)'/></a>
    <image href="https://elsewhere.test/track.png"/><use xlink:href="#g"/>
    <image href="data:image/png;base64,iVBORw0KGgo="/></svg>`;
  const svg = await (await paint(api)).text();
  for (const gone of ["alert", "<script", "foreignObject", "iframe", "elsewhere.test", "javascript", "@import", "onload", "onclick"]) {
    assert.ok(!svg.toLowerCase().includes(gone.toLowerCase()), `${gone} is gone: ${svg}`);
  }
  for (const kept of ['fill="url(#g)"', 'xlink:href="#g"', 'href="data:image/png;base64,', "<linearGradient", "<rect"]) {
    assert.ok(svg.includes(kept), `${kept} is kept: ${svg}`);
  }
});

// After every test: each room it opened sleeps once its devices are gone. A room that wakes by
// itself costs a request each time, out of 100,000 a day for the whole site, and nothing else
// counts those: two rooms waking for ever used up a whole day's in a morning.
async function everyRoomSleeps(mf) {
  const rooms = opened.get(mf);
  if (!rooms.size) return;
  for (const sockets of rooms.values()) {
    for (const ws of sockets) {
      try {
        ws.close();
      } catch {
        // Closed by the room already.
      }
    }
  }
  await new Promise(r => setTimeout(r, 500));
  const namespace = await mf.getDurableObjectNamespace("ROOMS");
  for (const name of rooms.keys()) {
    const { at, woken } = await namespace.get(namespace.idFromName(name)).wakes();
    assert.equal(at, null, `room ${name}: no device is in it, so nothing is to wake it`);
    assert.ok(woken <= 5, `room ${name} woke ${woken} times by itself`);
  }
}

// The room a writer's story meets in, as the Worker names it: the session's user, and the key.
function roomName(cookie, key) {
  const body = cookie.slice(cookie.indexOf("=") + 1).split(".")[0];
  return `${JSON.parse(Buffer.from(body, "base64url").toString()).u}/${key}`;
}

// A device opening story `key` live, as the writer of `cookie`: its socket, every message it was
// sent, and a way to wait for the next of a kind.
async function device(mf, cookie, key, id, name = "Firefox on Linux", headers = {}) {
  const r = await mf.dispatchFetch(`${SITE}/api/live/${key}?device=${id}&name=${encodeURIComponent(name)}`, {
    headers: { Upgrade: "websocket", Origin: SITE, Cookie: cookie, ...headers },
  });
  if (r.status !== 101) return { status: r.status, said: await r.text() };
  const ws = r.webSocket;
  const room = roomName(cookie, key);
  opened.get(mf).set(room, [...(opened.get(mf).get(room) ?? []), ws]);
  const got = [];
  const waiting = [];
  ws.addEventListener("message", e => {
    const m = JSON.parse(e.data);
    got.push(m);
    for (const w of waiting.splice(0)) w();
  });
  let closed = null;
  ws.addEventListener("close", e => { closed = e.code; for (const w of waiting.splice(0)) w(); });
  ws.accept();
  const next = async (t, ms = 2000) => {
    const end = Date.now() + ms;
    for (;;) {
      const i = got.findIndex(m => m.t === t);
      if (i >= 0) return got.splice(i, 1)[0];
      if (closed !== null || Date.now() > end) return null;
      await Promise.race([new Promise(r => waiting.push(r)), new Promise(r => setTimeout(r, end - Date.now()))]);
    }
  };
  return { status: 101, ws, got, next, say: m => ws.send(JSON.stringify(m)), closed: () => closed };
}

async function liveWriter(mf, sub, name) {
  const { session } = await signIn(mf, sub, name);
  return session;
}

test("the first device holds the pen, the others follow, and every edit reaches them", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001", "Firefox on Linux");
  const hello = await laptop.next("hello");
  assert.deepEqual([hello.you, hello.holder, hello.seq], ["laptop0001", "laptop0001", 0], "the first one holds the pen");
  const phone = await device(mf, ada, "story-1", "phone00001", "Chrome on Android");
  const tablet = await device(mf, ada, "story-1", "tablet0001", "Safari on iOS");
  for (const d of [phone, tablet]) {
    const h = await d.next("hello");
    assert.deepEqual([h.holder, h.name], ["laptop0001", "Firefox on Linux"], "the others are told who writes");
  }
  laptop.say({ t: "edits", seq: 1, edits: [{ at: 0, text: "Once" }], hash: "h1", at: 4 });
  laptop.say({ t: "edits", seq: 2, edits: [{ at: 4, text: " upon" }], hash: "h2", at: 9 });
  for (const d of [phone, tablet]) {
    assert.deepEqual((await d.next("edits")).edits, [{ at: 0, text: "Once" }]);
    const second = await d.next("edits");
    assert.deepEqual([second.seq, second.hash, second.at], [2, "h2", 9], "in order, with the hash and where it is written");
  }
  assert.equal(await laptop.next("edits", 200), null, "the writer is not sent its own");
});

test("a follower cannot write, and a batch out of order is refused", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001");
  const phone = await device(mf, ada, "story-1", "phone00001");
  await laptop.next("hello");
  await phone.next("hello");
  phone.say({ t: "edits", seq: 1, edits: ["sneaked in"], hash: "x" });
  assert.equal((await phone.next("refused")).why, "not the pen");
  laptop.say({ t: "edits", seq: 5, edits: ["skipped"], hash: "x" });
  assert.deepEqual(await laptop.next("refused"), { t: "refused", why: "out of order", seq: 0 });
  assert.equal(await phone.next("edits", 300), null, "neither reached anyone");
  laptop.say({ t: "edits", seq: 1, edits: ["fine"], hash: "x" });
  assert.equal((await phone.next("edits")).seq, 1);
});

test("only the writer's own devices meet in a story's room", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  const eve = await liveWriter(mf, "2", "Eve");
  const mine = await device(mf, ada, "story-1", "laptop0001");
  await mine.next("hello");
  const theirs = await device(mf, eve, "story-1", "evedevice1");
  assert.equal((await theirs.next("hello")).holder, "evedevice1", "the same key is another room for another writer");
  mine.say({ t: "edits", seq: 1, edits: ["private"], hash: "x" });
  assert.equal(await theirs.next("edits", 300), null, "and hears nothing of it");

  assert.equal((await device(mf, "", "story-1", "nobody0001")).status, 401, "no session");
  assert.equal((await device(mf, ada, "story-1", "laptop0002", "x", { Origin: "https://elsewhere.test" })).status, 403, "another site");
  assert.equal((await device(mf, ada, "Not.An_Id", "laptop0003")).status, 400, "not a story's id");
  assert.equal((await device(mf, ada, "story-1", "x")).status, 400, "not a device's id");
  const plain = await mf.dispatchFetch(`${SITE}/api/live/story-1?device=laptop0001`, { headers: { Origin: SITE, Cookie: ada } });
  assert.equal(plain.status, 426, "only a WebSocket");
});

test("transfer here: the writer sends its last batch, gives the pen, and everyone is told", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001", "Firefox on Linux");
  const phone = await device(mf, ada, "story-1", "phone00001", "Chrome on Android");
  const tablet = await device(mf, ada, "story-1", "tablet0001", "Safari on iOS");
  for (const d of [laptop, phone, tablet]) await d.next("hello");
  phone.say({ t: "take" });
  assert.ok(await laptop.next("give"), "the writer is asked to give the pen");
  laptop.say({ t: "edits", seq: 1, edits: ["last words"], hash: "x" });
  laptop.say({ t: "given" });
  for (const d of [phone, tablet, laptop]) {
    const pen = await d.next("pen");
    assert.deepEqual([pen.holder, pen.name, pen.seq], ["phone00001", "Chrome on Android", 1], "the phone writes, on from batch 1");
  }
  phone.say({ t: "edits", seq: 2, edits: ["from the phone"], hash: "y" });
  assert.equal((await laptop.next("edits")).edits[0], "from the phone", "the laptop follows now");
});

test("a writer that does not answer loses the pen after the hand-over time", async t => {
  const mf = await start(t, { LIVE_HAND_OVER_MS: "300" });
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001");
  const phone = await device(mf, ada, "story-1", "phone00001");
  await laptop.next("hello");
  await phone.next("hello");
  phone.say({ t: "take" });
  await laptop.next("give");
  assert.equal((await phone.next("pen", 3000)).holder, "phone00001", "asleep, it has the pen taken all the same");
});

test("a writer gone for a while frees the pen; back sooner, it keeps it", async t => {
  const mf = await start(t, { LIVE_FREE_AFTER_MS: "500" });
  const ada = await liveWriter(mf, "1", "Ada");
  let laptop = await device(mf, ada, "story-1", "laptop0001");
  const phone = await device(mf, ada, "story-1", "phone00001");
  await laptop.next("hello");
  await phone.next("hello");
  laptop.ws.close();
  await new Promise(r => setTimeout(r, 100));
  laptop = await device(mf, ada, "story-1", "laptop0001");
  assert.equal((await laptop.next("hello")).holder, "laptop0001", "a reload keeps the pen");
  laptop.ws.close();
  assert.equal((await phone.next("pen", 3000)).holder, "phone00001", "gone long enough, the pen is free");
});

test("a writer whose page or story closed frees the pen at once; one loading again keeps it", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  // A page loading again to come straight back (again.rs): the pen waits for it.
  let laptop = await device(mf, ada, "story-1", "laptop0001");
  const phone = await device(mf, ada, "story-1", "phone00001");
  await laptop.next("hello");
  await phone.next("hello");
  laptop.ws.close(4003, "Left");
  assert.equal(await phone.next("pen", 500), null, "kept for the page coming back");
  laptop = await device(mf, ada, "story-1", "laptop0001");
  assert.equal((await laptop.next("hello")).holder, "laptop0001", "and back, it writes on");
  // Its tab closed (1001, as the browser closes the page's sockets): the phone writes at once,
  // not half a minute later.
  laptop.ws.close(1001, "Left");
  assert.equal((await phone.next("pen", 2000)).holder, "phone00001", "the tab closed, the pen is free");
  // The story closed there (1000): the same.
  const tablet = await device(mf, ada, "story-1", "tablet0001");
  await tablet.next("hello");
  phone.ws.close(1000, "Left");
  assert.equal((await tablet.next("pen", 2000)).holder, "tablet0001", "the story closed, the pen is free");
});

test("a room everyone left sleeps until a device comes back, however long the pen was gone", async t => {
  const mf = await start(t, { LIVE_FREE_AFTER_MS: "200" });
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001");
  await laptop.next("hello");
  laptop.ws.close();
  await new Promise(r => setTimeout(r, 1500));
  const rooms = await mf.getDurableObjectNamespace("ROOMS");
  const wakes = await rooms.get(rooms.idFromName(roomName(ada, "story-1"))).wakes();
  assert.ok(wakes.woken <= 1, `woken ${wakes.woken} times: every wake is a request, out of 100,000 a day`);
  assert.equal(wakes.at, null, "and nothing left to wake it");
  const phone = await device(mf, ada, "story-1", "phone00001");
  assert.equal((await phone.next("hello")).holder, "phone00001", "the next device in has the pen");
});

test("a follower that lost its way has the writer save, and is told what to fetch", async t => {
  const mf = await start(t);
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001");
  const phone = await device(mf, ada, "story-1", "phone00001");
  await laptop.next("hello");
  await phone.next("hello");
  phone.say({ t: "whole" });
  assert.ok(await laptop.next("save"), "the writer is asked to save");
  laptop.say({ t: "saved", version: "v7" });
  assert.equal((await phone.next("version")).version, "v7");
  const tablet = await device(mf, ada, "story-1", "tablet0001");
  assert.equal((await tablet.next("hello")).version, "v7", "a device coming later starts from it too");
});

test("a story's messages for the day run out, and too big a message is not taken", async t => {
  const mf = await start(t, { ROOM_MESSAGES_PER_DAY: "2", LIVE_MESSAGE_BYTES: "100" });
  const ada = await liveWriter(mf, "1", "Ada");
  const laptop = await device(mf, ada, "story-1", "laptop0001");
  await laptop.next("hello");
  laptop.ws.send("x".repeat(101));
  await laptop.next("never", 300);
  assert.equal(laptop.closed(), 1009, "too big: closed");
  const again = await device(mf, ada, "story-1", "laptop0001");
  await again.next("hello");
  again.say({ t: "edits", seq: 1, edits: [], hash: "a" });
  again.say({ t: "edits", seq: 2, edits: [], hash: "b" });
  again.say({ t: "edits", seq: 3, edits: [], hash: "c" });
  await again.next("never", 300);
  assert.equal(again.closed(), 4002, "the day's messages ran out");
  assert.equal((await device(mf, ada, "story-1", "phone00001")).status, 503, "and no one comes in until tomorrow");
});

test("live editing has its share of the site's requests, and stories online keep the rest", async t => {
  const mf = await start(t, { SITE_LIVE_MESSAGES_PER_DAY: "5", LIVE_MESSAGES_AT_ONCE: "2" });
  const ada = await liveWriter(mf, "1", "Ada");
  const one = await device(mf, ada, "story-1", "laptop0001");
  await one.next("hello");
  const other = await device(mf, ada, "story-2", "laptop0001");
  await other.next("hello");
  // Two rooms take from the same share: two at a time, five in all.
  one.say({ t: "edits", seq: 1, edits: [], hash: "a" });
  one.say({ t: "edits", seq: 2, edits: [], hash: "b" });
  one.say({ t: "edits", seq: 3, edits: [], hash: "c" });
  other.say({ t: "edits", seq: 1, edits: [], hash: "a" });
  other.say({ t: "edits", seq: 2, edits: [], hash: "b" });
  other.say({ t: "edits", seq: 3, edits: [], hash: "c" });
  await one.next("never", 300);
  await other.next("never", 300);
  assert.deepEqual([one.closed(), other.closed()].sort(), [4002, null], "the share ran out in one room");
  const api = await writer(mf, "1", "Ada");
  assert.equal((await saveNew(api, "s", story("still saved"))).status, 200, "stories online go on");
});

test("the site's live connections for the day run out, and the off switch stops them", async t => {
  const mf = await start(t, { SITE_LIVE_PER_DAY: "1" });
  const ada = await liveWriter(mf, "1", "Ada");
  assert.equal((await device(mf, ada, "story-1", "laptop0001")).status, 101);
  assert.equal((await device(mf, ada, "story-1", "phone00001")).status, 503);
  const off = await start(t, { LIVE_OFF: "1" });
  assert.equal((await device(off, await liveWriter(off, "1", "Ada"), "story-1", "laptop0001")).status, 503);
});

const paintFor = (api, id, story = "story-1", at = 12) =>
  api("/api/scene", { method: "POST", body: "<scene>\nA lighthouse\n</scene>", headers: { "X-Caprice-Painting": id, "X-Caprice-Story": story, "X-Caprice-At": String(at) } });

test("a picture painted waits for the app until it has it, and only for its writer", async t => {
  const mf = await start(t);
  const ada = await writer(mf, "1", "Ada");
  const eve = await writer(mf, "2", "Eve");
  assert.equal((await paintFor(ada, "paint0001")).status, 200);
  await paintFor(ada, "paint0002", "story-2", 40);
  // The page went before the answer came: opening the story again, it is there.
  const waiting = await (await ada("/api/scenes?story=story-1")).json();
  assert.deepEqual(waiting.map(p => [p.id, p.at]), [["paint0001", 12]]);
  assert.ok(waiting[0].svg.startsWith("<svg") && waiting[0].svg.includes("Made with AI"), "the picture as it was sent");
  assert.deepEqual(await (await eve("/api/scenes?story=story-1")).json(), [], "no one else's");
  assert.equal((await ada("/api/scenes?story=../x")).status, 400);
  // The app has it: it waits no longer.
  assert.equal((await ada("/api/scenes/paint0001", { method: "DELETE" })).status, 204);
  assert.deepEqual(await (await ada("/api/scenes?story=story-1")).json(), []);
  assert.equal((await (await ada("/api/scenes?story=story-2")).json()).length, 1, "another story's still waits");
  // Without saying where it goes, a picture is not kept.
  await ada("/api/scene", { method: "POST", body: "<scene>x</scene>" });
  assert.equal((await (await ada("/api/scenes?story=story-1")).json()).length, 0);
});

test("pictures waiting are few for each writer, and go with the stories online", async t => {
  const mf = await start(t, { PAINTED_PER_USER: "2" });
  const ada = await writer(mf, "1", "Ada");
  for (const id of ["paint0001", "paint0002", "paint0003"]) await paintFor(ada, id);
  const waiting = await (await ada("/api/scenes?story=story-1")).json();
  assert.deepEqual(waiting.map(p => p.id), ["paint0002", "paint0003"], "the oldest went first");
  assert.equal((await ada("/api/stories", { method: "DELETE" })).status, 204);
  assert.deepEqual(await (await ada("/api/scenes?story=story-1")).json(), [], "deleting the stories online deletes them too");
});

// ---- Making a theme program with Claude (theme.js) ---------------------------------------------

// What Claude writes for a theme: some words, which never reach the browser, the palette and the file.
const THEME_REPLY = "Here is a lighthouse for you, I hope you like it!\n\n```json\n"
  + JSON.stringify({ name: "Lighthouse", about: "A light on the cliff, a window a page", colours: { desk_top: "#102040", desk: "#305080", text: "not a colour", light: "no", sparkle: "#ffffff" } })
  + "\n```\n\n```rust\nuse theme_sdk::*;\nstruct Light;\nimpl Theme for Light { fn new() -> Self { Light } fn frame(&mut self, _: &Input, _: &mut Out) {} }\ntheme!(Light);\n```\n\nEnjoy!";

const begin = (api, said = "A lighthouse on a cliff, a window lit for each page") =>
  api("/api/theme-program", { method: "POST", body: said });
const round = (api, id, kind, body, headers = {}) =>
  api(`/api/theme-program/${id}`, { method: "POST", body, headers: { "X-Caprice-Kind": kind, ...headers } });

// A PNG's first bytes, enough for the Worker's check: its signature and a header of `w` by `h`.
function png(w, h, fill = 0) {
  const b = new Uint8Array(60).fill(fill);
  b.set([137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82]);
  new DataView(b.buffer).setUint32(16, w);
  new DataView(b.buffer).setUint32(20, h);
  return b;
}
// Pictures as the browser sends them: a count, the lengths, the PNGs.
function framed(pngs) {
  const head = new Uint8Array(1 + 4 * pngs.length);
  head[0] = pngs.length;
  pngs.forEach((p, k) => new DataView(head.buffer).setUint32(1 + 4 * k, p.length, true));
  return Buffer.concat([head, ...pngs]);
}

test("a theme is begun from the writer's words, and only its source and palette come back", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  const got = await begin(api);
  assert.equal(got.status, 200);
  const theme = await got.json();
  assert.match(theme.theme, /^[0-9a-f]{16}$/);
  assert.ok(theme.source.startsWith("use theme_sdk::*;") && theme.source.trim().endsWith("theme!(Light);"));
  assert.deepEqual(theme.colours, { desk_top: "#102040", desk: "#305080", light: "no" }, "only colours by the palette's names, as colours");
  assert.equal(theme.name, "Lighthouse");
  assert.deepEqual(theme.left, { errors: 3, frames: 2, change: 3 });
  assert.ok(!JSON.stringify(theme).includes("hope you like it") && !JSON.stringify(theme).includes("Enjoy"), "none of Claude's own words");

  const [{ key, body }] = claude.asked;
  assert.equal(key, "a test's key");
  assert.equal(body.model, "claude-opus-5-5");
  assert.equal(body.max_tokens, 64000, "the most a round can cost is the Worker's");
  const system = body.system.map(b => b.text).join("");
  for (const part of ["# Writing a magic theme for Caprice", "pub trait Theme", "package caprice:theme", "The Magic Snow, as a theme program", "theme!(Fireflies)"]) {
    assert.ok(system.includes(part), `the prompt holds ${part}`);
  }
  assert.deepEqual(body.system.at(-1).cache_control, { type: "ephemeral", ttl: "1h" }, "and is cached for longer than a round takes");
  assert.deepEqual(body.cache_control, { type: "ephemeral", ttl: "1h" }, "the conversation too");
  assert.deepEqual(body.messages, [{ role: "user", content: "<request>\nA lighthouse on a cliff, a window lit for each page\n</request>\n\nWrite this theme. Answer as the guide says: the palette's json block, then the whole rust file." }]);
});

test("the next round tells claude the compiler's words, with the whole conversation kept by the worker", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  const { theme } = await (await begin(api)).json();
  const fixed = await round(api, theme, "errors", "error[E0425]: cannot find value `x` in this scope");
  assert.equal(fixed.status, 200);
  assert.deepEqual((await fixed.json()).left, { errors: 2, frames: 2, change: 3 });
  const { messages } = claude.asked[1].body;
  assert.equal(messages.length, 3);
  assert.equal(messages[1].role, "assistant");
  assert.equal(messages[1].content, THEME_REPLY, "Claude's own answer, whole, as the Worker kept it: never the browser's");
  assert.match(messages[2].content, /^It did not build\. The compiler said:\n\n```\nerror\[E0425\]/);
  // A change asked for by the writer goes the same way.
  assert.equal((await round(api, theme, "change", "Make the sea stormier")).status, 200);
  assert.match(claude.asked[2].body.messages[4].content, /<change>\nMake the sea stormier\n<\/change>/);
});

test("what caprice's checks found of a program that built goes to claude as a fix, counted with the compiler's", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  const { theme } = await (await begin(api)).json();
  const found = "It plays, but draws too much: at most 180000 points a frame on a phone's desk";
  const checked = await round(api, theme, "checks", found);
  assert.equal(checked.status, 200);
  assert.deepEqual((await checked.json()).left, { errors: 2, frames: 2, change: 3 }, "a fix like any other");
  assert.equal(claude.asked[1].body.messages[2].content, `It built, but Caprice's checks found:\n\n<checks>\n${found}\n</checks>\n\nFix it, and give the palette and the whole file again.`);
  assert.equal((await round(api, theme, "errors", "error: one")).status, 200);
  assert.deepEqual((await (await round(api, theme, "checks", "Caprice stopped it while playing the story: it failed")).json()).left.errors, 0);
  const more = await round(api, theme, "checks", "Caprice will not play its program");
  assert.equal(more.status, 429, "three fixes in all, of either kind");
  assert.equal(claude.asked.length, 4);
});

test("pictures go to claude as images, and are kept in the conversation as words only", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  const { theme } = await (await begin(api)).json();
  const pngs = [png(1280, 800, 1), png(1280, 800, 2)];
  const looked = await round(api, theme, "frames", framed(pngs), { "X-Caprice-Moments": "begun,written", "X-Caprice-Points": "4949,9686" });
  assert.equal(looked.status, 200);
  const content = claude.asked[1].body.messages[2].content;
  assert.equal(content.length, 3);
  assert.deepEqual(content.slice(0, 2).map(c => c.source.data), pngs.map(p => Buffer.from(p).toString("base64")), "each picture as it came");
  assert.match(content[2].text, /begun, written\. At most 4949 points a frame on a phone's desk, 9686 on a computer's/);
  await round(api, theme, "change", "Brighter");
  const kept = claude.asked[2].body.messages[2].content;
  assert.ok(typeof kept === "string" && kept.startsWith("[2 pictures were shown here.]"), "later rounds see words in their place");
});

test("what is not one of its kinds, too big, or not pictures is refused before claude is asked", async t => {
  const mf = await start(t);
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  assert.equal((await begin(api, "x".repeat(2001))).status, 413);
  assert.equal((await begin(api, "   ")).status, 400);
  const { theme } = await (await begin(api)).json();
  assert.equal((await round(api, theme, "chat", "What is the capital of France?")).status, 400, "no round of any other kind");
  assert.equal((await round(api, theme, "errors", "e".repeat(20_001))).status, 413);
  assert.equal((await round(api, theme, "checks", "c".repeat(20_001))).status, 413);
  assert.equal((await round(api, theme, "frames", framed([png(20, 20)]), { "X-Caprice-Moments": "begun", "X-Caprice-Points": "1,2" })).status, 400, "too small to be a picture of a desk");
  assert.equal((await round(api, theme, "frames", Buffer.from("not pictures at all"), { "X-Caprice-Moments": "begun", "X-Caprice-Points": "1,2" })).status, 400);
  assert.equal((await round(api, theme, "frames", framed([png(800, 600)]), { "X-Caprice-Moments": "begun,written", "X-Caprice-Points": "1,2" })).status, 400, "a moment for each picture");
  assert.equal((await api("/api/theme-program/not-an-id", { method: "POST", body: "x" })).status, 400);
  assert.equal(claude.asked.length, 1, "only the theme begun reached Claude");
});

test("what each theme costs is kept with it, and the owner sees it a day at a time with how many were kept", async t => {
  const token = "the owner's token, long enough to be one";
  const mf = await start(t, { ADMIN_TOKEN: token });
  const ada = await writer(mf, "1", "Ada");
  const bob = await writer(mf, "2", "Bob");
  // A round as measured: the prompt written to the cache for an hour, and most of it thinking.
  claude.usage = { input_tokens: 1000, cache_creation_input_tokens: 50_000, cache_read_input_tokens: 0, output_tokens: 40_000 };
  claude.writes = THEME_REPLY;
  const { theme } = await (await begin(ada)).json();
  assert.equal((await round(ada, theme, "errors", "error: one")).status, 200);
  // Claude is paid for a reply without a program too.
  claude.writes = "I would rather not write any code today.";
  assert.equal((await begin(bob)).status, 502);
  claude.usage = undefined;

  const kept = id => ada(`/api/theme-program/${id}/kept`, { method: "POST" });
  assert.equal((await kept(theme)).status, 204);
  assert.equal((await kept(theme)).status, 204, "once is enough, and twice is no harm");
  assert.equal((await bob(`/api/theme-program/${theme}/kept`, { method: "POST" })).status, 404, "only its writer keeps it");
  assert.equal((await kept("0123456789abcdef")).status, 404);
  assert.equal(claude.asked.length, 3, "keeping asks nothing of Claude");

  const ask = authorization => mf.dispatchFetch(`${SITE}/api/admin/themes`, { headers: authorization ? { Authorization: authorization } : {} });
  assert.equal((await ask("Bearer not it")).status, 401);
  const { days, total } = await (await ask(`Bearer ${token}`)).json();
  assert.equal(days.length, 1);
  // A round: 1 000 × $4 + 50 000 × $8 + 40 000 × $20 a million tokens, $1.204.
  assert.deepEqual({ ...total, day: undefined }, {
    day: undefined, begun: 2, kept: 1, rounds: 3, tokens: { input: 3000, written: 150_000, read: 0, output: 120_000 },
    dollars: 3.61, keptDollars: 2.41, costPerKept: 3.61,
  });
  assert.ok(!JSON.stringify(days).includes("Ada") && !JSON.stringify(days).includes(theme), "nobody's, and no theme's");

  // Ada deletes what she has online: the numbers stay, but say no more that the theme was hers.
  assert.equal((await ada("/api/stories", { method: "DELETE" })).status, 204);
  assert.equal((await (await ask(`Bearer ${token}`)).json()).total.dollars, 3.61);
  assert.equal((await kept(theme)).status, 404);
});

test("rounds of each kind run out for a theme, and themes for a writer, and claude is not asked again", async t => {
  const mf = await start(t, { THEME_ERRORS_ROUNDS: "1", USER_THEMES_PER_DAY: "1" });
  const ada = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  const { theme } = await (await begin(ada)).json();
  assert.equal((await round(ada, theme, "errors", "error: one")).status, 200);
  const again = await round(ada, theme, "errors", "error: two");
  assert.equal(again.status, 429);
  assert.match(await again.text(), /No more rounds of this kind/);
  assert.equal((await round(ada, theme, "change", "Bluer")).status, 200, "the other kinds go on");
  const another = await begin(ada);
  assert.equal(another.status, 429);
  assert.match(await another.text(), /1 themes a day/);
  // Nobody else may go on with Ada's theme.
  const bob = await writer(mf, "2", "Bob");
  assert.equal((await round(bob, theme, "change", "Mine now")).status, 404);
  assert.equal(claude.asked.length, 3);
});

test("the site's rounds for the day run out for everyone, and a round that fails still counts", async t => {
  const mf = await start(t, { SITE_THEME_ROUNDS_PER_DAY: "2" });
  const ada = await writer(mf, "1", "Ada");
  claude.writes = "I would rather not write any code today.";
  const nothing = await begin(ada);
  assert.equal(nothing.status, 502, "a reply without a program gives nothing back");
  claude.writes = THEME_REPLY;
  assert.equal((await begin(ada)).status, 200);
  const bob = await writer(mf, "2", "Bob");
  const refused = await begin(bob);
  assert.equal(refused.status, 503);
  assert.match(await refused.text(), /no more themes today/);
  assert.equal(claude.asked.length, 2);
});

test("without a key, or switched off, no theme is made", async t => {
  const off = await start(t, { THEMES_OFF: "1" });
  assert.equal((await begin(await writer(off, "1", "Ada"))).status, 503);
  const keyless = await start(t, { ANTHROPIC_API_KEY: "" });
  assert.equal((await begin(await writer(keyless, "1", "Ada"))).status, 503);
  assert.equal(claude.asked.length, 0);
});

test("a round that takes longer than a beat is answered at once, a space every beat, then the theme or why not", async t => {
  const mf = await start(t, { THEME_BEAT_MS: "40" });
  const api = await writer(mf, "1", "Ada");
  claude.writes = THEME_REPLY;
  let release;
  claude.hold = new Promise(r => { release = r; });
  const got = await begin(api);
  assert.equal(got.status, 200, "answered before Claude has");
  assert.equal(got.headers.get("Cache-Control"), "no-store, no-transform", "nothing on the way holds the spaces back");
  const reading = got.body.getReader();
  let heard = "";
  while (heard.length < 3) heard += new TextDecoder().decode((await reading.read()).value);
  assert.match(heard, /^ +$/, "only spaces while it is being written");
  release();
  for (let r = await reading.read(); !r.done; r = await reading.read()) heard += new TextDecoder().decode(r.value);
  const theme = JSON.parse(heard);
  assert.equal(theme.name, "Lighthouse", "the theme, after the spaces");
  assert.match(theme.theme, /^[0-9a-f]{16}$/);
  assert.ok(!heard.includes("hope you like it"), "none of Claude's own words");

  // Claude failing once the spaces have begun: the status is sent already, the body says it.
  claude.hold = new Promise(r => { release = r; });
  claude.broke = { status: 529, said: "Overloaded" };
  const busy = await round(api, theme.theme, "change", "Bluer");
  assert.equal(busy.status, 200);
  release();
  const said = await busy.json();
  assert.equal(said.error, 503);
  assert.match(said.message, /^Many themes are being made just now/);
  assert.ok(!said.message.includes("Claude"), "the writer is not told of Claude");
  claude.broke = null;
  claude.hold = null;
  assert.equal((await round(api, theme.theme, "change", "Greener")).status, 200, "the theme is free for its next round");
});

// The gallery (gallery.js, R36): a theme file sent, checked and queued; the owner's decision; what
// everyone sees. The owner is the writer signed in as "9", their hashed id in GALLERY_OWNERS.
const OWNER = createHash("sha256").update("google:9").digest("hex").slice(0, 32);
const KEYS = generateKeyPairSync("ed25519");
const SEED = KEYS.privateKey.export({ format: "der", type: "pkcs8" }).subarray(16).toString("base64");
const GALLERY = { GALLERY_OWNERS: `someone-else,${OWNER}`, GALLERY_SIGNING_KEY: SEED };
const SNOW_FILE = readFileSync(new URL("../../tests/fixtures/snow.caprice-theme", import.meta.url));
const sha = b => createHash("sha256").update(b).digest("hex");

// The parts of a zip, as the app's `bundle::write` makes them (deflated), read back.
function unzipped(zip) {
  const parts = {};
  for (let at = 0; zip.readUInt32LE(at) === 0x04034b50;) {
    const [packed, n, e] = [zip.readUInt32LE(at + 18), zip.readUInt16LE(at + 26), zip.readUInt16LE(at + 28)];
    const name = zip.subarray(at + 30, at + 30 + n).toString();
    const data = zip.subarray(at + 30 + n + e, at + 30 + n + e + packed);
    parts[name] = zip.readUInt16LE(at + 8) === 8 ? inflateRawSync(data) : data;
    at += 30 + n + e + packed;
  }
  return parts;
}

// A zip of `parts` (name to bytes), deflated, as a theme file is.
function zipped(parts) {
  const local = [], central = [];
  let at = 0;
  for (const [name, bytes] of Object.entries(parts)) {
    const data = deflateRawSync(bytes);
    const n = Buffer.from(name);
    const head = Buffer.alloc(30);
    head.writeUInt32LE(0x04034b50, 0); head.writeUInt16LE(20, 4); head.writeUInt16LE(8, 8);
    head.writeUInt32LE(data.length, 18); head.writeUInt32LE(bytes.length, 22); head.writeUInt16LE(n.length, 26);
    const dir = Buffer.alloc(46);
    dir.writeUInt32LE(0x02014b50, 0); dir.writeUInt16LE(20, 4); dir.writeUInt16LE(20, 6); dir.writeUInt16LE(8, 10);
    dir.writeUInt32LE(data.length, 20); dir.writeUInt32LE(bytes.length, 24); dir.writeUInt16LE(n.length, 28); dir.writeUInt32LE(at, 42);
    local.push(head, n, data);
    central.push(dir, n);
    at += 30 + n.length + data.length;
  }
  const dir = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(central.length / 2, 8); end.writeUInt16LE(central.length / 2, 10);
  end.writeUInt32LE(dir.length, 12); end.writeUInt32LE(at, 16);
  return Buffer.concat([...local, dir, end]);
}

const SNOW = unzipped(SNOW_FILE);
const manifest = changes => Buffer.from(JSON.stringify({ ...JSON.parse(SNOW["manifest.json"]), ...changes }));
const send = (api, file) => api("/api/gallery", { method: "POST", body: file, headers: { "Content-Type": "application/zip" } });

test("a theme sent to the gallery is checked by the worker and waits for the owner, seen by no one else", async t => {
  const mf = await start(t, GALLERY);
  const ada = await writer(mf, "1", "Ada");
  const sent = await send(ada, SNOW_FILE);
  assert.equal(sent.status, 202);
  const { id, state } = await sent.json();
  assert.match(id, /^[0-9a-f]{16}$/);
  assert.equal(state, "waiting");
  assert.deepEqual((await inR2(mf)).sort(), [`gallery/${id}/preview.png`, `gallery/${id}/theme.caprice-theme`]);
  const mine = await (await ada("/api/gallery/mine")).json();
  assert.deepEqual(mine.themes.map(m => [m.id, m.name, m.state, m.module]), [[id, "Magic Snow", "waiting", sha(SNOW["theme.wasm"])]]);

  // Not in the gallery yet: listed for no one, read by no one but the owner.
  assert.deepEqual((await (await ada("/api/gallery")).json()).themes, []);
  assert.equal((await ada(`/api/gallery/${id}/theme`)).status, 404);
  assert.equal((await ada("/api/gallery/waiting")).status, 404, "the queue is the owner's");
  const owner = await writer(mf, "9", "Owner");
  assert.equal((await (await owner("/api/me")).json()).owner, true, "the app is told who the owner is");
  assert.equal((await (await ada("/api/me")).json()).owner, undefined);
  const waiting = (await (await owner("/api/gallery/waiting")).json()).themes;
  assert.equal(waiting.length, 1);
  assert.deepEqual([waiting[0].name, waiting[0].author, waiting[0].toolchain, waiting[0].module, waiting[0].source],
    ["Magic Snow", "Caprice", "rustc 1.99.1", sha(SNOW["theme.wasm"]), sha(SNOW["theme.rs"])], "worked out by the worker");
  const file = await owner(`/api/gallery/${id}/theme`);
  assert.equal(file.headers.get("Cache-Control"), "no-store", "a theme waiting may still change its fate");
  assert.ok(Buffer.from(await file.arrayBuffer()).equals(SNOW_FILE), "the owner reads it as it was sent");

  assert.equal((await send(ada, SNOW_FILE)).status, 409, "the same program is sent once");
});

test("what is not a theme the gallery can build and play is refused before anything is kept", async t => {
  // A limit of 200 kB, above the themes sent here: a body over 3 MB, refused by its length before
  // it was all sent, sometimes broke the connection under it (EPIPE) instead of being answered.
  const most = 200_000;
  const mf = await start(t, { ...GALLERY, GALLERY_FILE_BYTES: String(most) });
  const ada = await writer(mf, "1", "Ada");
  const refusedFor = async (parts, said) => {
    const got = await send(ada, Buffer.isBuffer(parts) ? parts : zipped(parts));
    assert.equal(got.status, 422, said);
    assert.match(await got.text(), said);
  };
  const { "theme.rs": _, ...noSource } = SNOW;
  await refusedFor(Buffer.from("not a zip at all"), /not a Caprice theme file/);
  await refusedFor(noSource, /no source/);
  await refusedFor({ ...SNOW, "manifest.json": manifest({ world: "caprice:theme@2.0.0" }) }, /not made for caprice:theme@1\.0\.0/);
  await refusedFor({ ...SNOW, "manifest.json": manifest({ name: " " }) }, /no name/);
  await refusedFor({ ...SNOW, "manifest.json": manifest({ toolchain: "" }) }, /which toolchain/);
  await refusedFor({ ...SNOW, "theme.rs": Buffer.from("fn main() {}") }, /not a theme's/);
  await refusedFor({ ...SNOW, "preview.png": SNOW["theme.wasm"] }, /picture/);
  // A module that asks for something besides Caprice's host: its import renamed, byte for byte.
  const module = Buffer.from(SNOW["theme.wasm"]);
  const at = module.indexOf("caprice:theme/host@1.0.0");
  module.write("caprice:theme/hack@1.0.0", at);
  await refusedFor({ ...SNOW, "theme.wasm": module }, /cannot be played: imports caprice:theme\/hack@1\.0\.0/);
  assert.deepEqual(await inR2(mf), [], "nothing kept");
  assert.deepEqual((await (await ada("/api/gallery/mine")).json()).themes, []);

  const big = await ada("/api/gallery", { method: "POST", body: Buffer.alloc(most + 1) });
  assert.equal(big.status, 413);
});

test("the owner's yes is signed by the worker over what it worked out, and the theme is everyone's", async t => {
  const mf = await start(t, GALLERY);
  const ada = await writer(mf, "1", "Ada");
  const { id } = await (await send(ada, SNOW_FILE)).json();
  const owner = await writer(mf, "9", "Owner");
  const decide = said => owner(`/api/gallery/waiting/${id}`, { method: "POST", body: JSON.stringify(said), headers: { "Content-Type": "application/json" } });
  assert.equal((await ada(`/api/gallery/waiting/${id}`, { method: "POST", body: '{"publish":true}' })).status, 404, "only the owner decides");

  // The module the owner's browser built from the source must be the one sent.
  const other = await decide({ publish: true, module: sha(Buffer.from("another module")) });
  assert.equal(other.status, 409);
  const yes = await decide({ publish: true, module: sha(SNOW["theme.wasm"]) });
  assert.equal(yes.status, 200);
  const { signature } = await yes.json();
  const signed = `caprice:theme@1.0.0\nmodule ${sha(SNOW["theme.wasm"])}\nsource ${sha(SNOW["theme.rs"])}\ntoolchain rustc 1.99.1\nname Magic Snow\n`;
  assert.ok(verify(null, Buffer.from(signed), KEYS.publicKey, Buffer.from(signature, "base64")), "the message the app checks");
  assert.equal((await decide({ publish: false, why: "changed my mind" })).status, 404, "decided once: it waits no more");

  // Everyone signed in sees it, with what the app needs to check the signature, but not who sent it.
  const bo = await writer(mf, "2", "Bo");
  const { themes } = await (await bo("/api/gallery")).json();
  assert.equal(themes.length, 1);
  const [listed] = themes;
  assert.deepEqual([listed.id, listed.name, listed.author, listed.source, listed.toolchain, listed.signature],
    [id, "Magic Snow", "Caprice", sha(SNOW["theme.rs"]), "rustc 1.99.1", signature]);
  assert.equal(listed.colours.desk, "#7a80b2");
  assert.ok(!JSON.stringify(themes).includes(createHash("sha256").update("google:1").digest("hex").slice(0, 32)), "never the writer's id");
  const file = await bo(`/api/gallery/${id}/theme`);
  assert.equal(file.headers.get("Cache-Control"), "private, max-age=31536000, immutable", "it never changes");
  assert.ok(Buffer.from(await file.arrayBuffer()).equals(SNOW_FILE));
  const picture = await bo(`/api/gallery/${id}/preview`);
  assert.equal(picture.headers.get("Content-Type"), "image/png");
  assert.ok(Buffer.from(await picture.arrayBuffer()).equals(SNOW["preview.png"]));
  assert.equal((await (await ada("/api/gallery/mine")).json()).themes[0].state, "public");

  // Signed out, nothing of it.
  assert.equal((await mf.dispatchFetch(`${SITE}/api/gallery`, { headers: { "X-Caprice": "1" } })).status, 401);
});

test("a theme the owner refuses says why to its writer, and its files go", async t => {
  const mf = await start(t, GALLERY);
  const ada = await writer(mf, "1", "Ada");
  const { id } = await (await send(ada, SNOW_FILE)).json();
  const owner = await writer(mf, "9", "Owner");
  const no = await owner(`/api/gallery/waiting/${id}`, { method: "POST", body: JSON.stringify({ publish: false, why: "It is the Magic Snow itself" }) });
  assert.equal(no.status, 200);
  assert.deepEqual(await inR2(mf), []);
  const [mine] = (await (await ada("/api/gallery/mine")).json()).themes;
  assert.deepEqual([mine.state, mine.why], ["refused", "It is the Magic Snow itself"]);
  assert.equal((await owner(`/api/gallery/${id}/theme`)).status, 404);
  assert.equal((await send(ada, SNOW_FILE)).status, 202, "and it may be sent again, changed or not");
});

test("the owner takes a published theme out of the gallery: listed and read no more, its files gone, its writer told", async t => {
  const mf = await start(t, GALLERY);
  const ada = await writer(mf, "1", "Ada");
  const { id } = await (await send(ada, SNOW_FILE)).json();
  const owner = await writer(mf, "9", "Owner");
  const out = () => owner(`/api/gallery/${id}`, { method: "DELETE" });
  assert.equal((await out()).status, 404, "only a theme in the gallery is taken out");
  await owner(`/api/gallery/waiting/${id}`, { method: "POST", body: JSON.stringify({ publish: true, module: sha(SNOW["theme.wasm"]) }) });
  assert.equal((await ada(`/api/gallery/${id}`, { method: "DELETE" })).status, 404, "only the owner takes one out");
  assert.equal((await owner("/api/gallery/not-an-id", { method: "DELETE" })).status, 400);
  assert.equal((await owner(`/api/gallery/${id}`)).status, 405);

  const taken = await out();
  assert.equal(taken.status, 200);
  assert.deepEqual(await taken.json(), { id, state: "removed" });
  assert.deepEqual((await (await ada("/api/gallery")).json()).themes, [], "listed no more");
  assert.equal((await owner(`/api/gallery/${id}/theme`)).status, 404, "read no more");
  assert.deepEqual(await inR2(mf), [], "its files gone");
  const [mine] = (await (await ada("/api/gallery/mine")).json()).themes;
  assert.deepEqual([mine.state, mine.why], ["removed", "Taken out of the gallery"]);
  assert.equal((await out()).status, 404, "taken out once");
  assert.equal((await send(ada, SNOW_FILE)).status, 202, "and it may be sent again");
});

test("a theme refused or taken out is listed to its writer for a while, then forgotten", async t => {
  // GALLERY_TOLD_DAYS of 0: told and forgotten at once. The two tests above show it told with the 30 days.
  const mf = await start(t, { ...GALLERY, GALLERY_TOLD_DAYS: "0" });
  const ada = await writer(mf, "1", "Ada");
  const owner = await writer(mf, "9", "Owner");
  const mine = async () => (await (await ada("/api/gallery/mine")).json()).themes.map(m => m.state);
  const first = (await (await send(ada, SNOW_FILE)).json()).id;
  await owner(`/api/gallery/waiting/${first}`, { method: "POST", body: JSON.stringify({ publish: false, why: "Not yet" }) });
  const second = (await (await send(ada, SNOW_FILE)).json()).id;
  assert.deepEqual(await mine(), ["waiting"], "the refused one forgotten, the one waiting kept");
  await owner(`/api/gallery/waiting/${second}`, { method: "POST", body: JSON.stringify({ publish: true, module: sha(SNOW["theme.wasm"]) }) });
  assert.deepEqual(await mine(), ["public"], "one in the gallery is never forgotten");
  await owner(`/api/gallery/${second}`, { method: "DELETE" });
  assert.deepEqual(await mine(), [], "and one taken out goes like a refused one");
});

test("a writer sends a few themes a day, the queue is bounded, and without a key nothing is signed", async t => {
  const mf = await start(t, { ...GALLERY, USER_PUBLISHED_PER_DAY: "2", GALLERY_WAITING: "3", GALLERY_SIGNING_KEY: "" });
  const [ada, bo] = [await writer(mf, "1", "Ada"), await writer(mf, "2", "Bo")];
  const named = name => zipped({ ...SNOW, "manifest.json": manifest({ name }), "theme.rs": Buffer.concat([SNOW["theme.rs"], Buffer.from(`// ${name}\n`)]),
    "theme.wasm": Buffer.concat([SNOW["theme.wasm"], Buffer.from([0, 4, 3, 0x61, 0x62, name.charCodeAt(0)])]) });
  assert.equal((await send(ada, named("a"))).status, 202);
  assert.equal((await send(ada, named("b"))).status, 202);
  const third = await send(ada, named("c"));
  assert.equal(third.status, 429);
  assert.ok(Number(third.headers.get("Retry-After")) > 0);
  assert.equal((await send(bo, named("d"))).status, 202);
  assert.equal((await send(bo, named("e"))).status, 503, "the owner has as many as they can look at");

  const owner = await writer(mf, "9", "Owner");
  const [first] = (await (await owner("/api/gallery/waiting")).json()).themes;
  const yes = await owner(`/api/gallery/waiting/${first.id}`, { method: "POST", body: JSON.stringify({ publish: true, module: first.module }) });
  assert.equal(yes.status, 503, "no key, no signature");
  assert.equal((await (await owner("/api/gallery/waiting")).json()).themes.length, 3, "still waiting");
});

test("deleting a writer's stories online takes their themes still waiting, and leaves the gallery's", async t => {
  const mf = await start(t, GALLERY);
  const ada = await writer(mf, "1", "Ada");
  const other = zipped({ ...SNOW, "theme.wasm": Buffer.concat([SNOW["theme.wasm"], Buffer.from([0, 3, 2, 0x61, 0x62])]) });
  const { id: kept } = await (await send(ada, SNOW_FILE)).json();
  const { id: waiting } = await (await send(ada, other)).json();
  const owner = await writer(mf, "9", "Owner");
  await owner(`/api/gallery/waiting/${kept}`, { method: "POST", body: JSON.stringify({ publish: true, module: sha(SNOW["theme.wasm"]) }) });
  assert.equal((await ada("/api/stories", { method: "DELETE" })).status, 204);
  assert.deepEqual((await inR2(mf)).sort(), [`gallery/${kept}/preview.png`, `gallery/${kept}/theme.caprice-theme`]);
  assert.deepEqual((await (await owner("/api/gallery/waiting")).json()).themes, []);
  assert.equal((await (await owner("/api/gallery")).json()).themes[0].id, kept);
  assert.notEqual(waiting, kept);
});
