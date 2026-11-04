// The stories API, run in the Workers runtime itself (Miniflare: R2 and the Durable Object in
// memory), signing in against stand-in Google and GitHub. Each limit is shown to refuse, and the
// ones that guard the bill to leave R2 untouched when they do.
// node --test api/   (here in web/)
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Miniflare, convertV4MiniflareOptions } from "miniflare";

const SITE = "https://caprice.test";
// The date the Worker is deployed with, so the tests run what readers get.
const DATE = readFileSync(new URL("../wrangler.jsonc", import.meta.url), "utf8").match(/"compatibility_date":\s*"([^"]+)"/)[1];

// The challenge of the last sign-in begun, which the stand-in providers check the verifier against.
let challenge;

function providers(request) {
  const url = new URL(request.url);
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

async function start(t, vars = {}) {
  const mf = new Miniflare(convertV4MiniflareOptions({
    modulesRoot: fileURLToPath(new URL(".", import.meta.url)),
    modules: ["worker.js", "ledger.js"].map(m => ({ type: "ESModule", path: fileURLToPath(new URL(m, import.meta.url)) })),
    compatibilityDate: DATE,
    r2Buckets: ["STORIES"],
    durableObjects: { LEDGER: { className: "Ledger", useSQLite: true } },
    bindings: {
      SESSION_SECRET: "a test's own secret, long enough to sign with",
      GOOGLE_CLIENT_ID: "google-app", GOOGLE_CLIENT_SECRET: "google-secret",
      GITHUB_CLIENT_ID: "github-app", GITHUB_CLIENT_SECRET: "github-secret",
      ...vars,
    },
    outboundService: providers,
  }));
  t.after(() => mf.dispose());
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
