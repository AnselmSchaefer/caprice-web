// The Worker's own requests: /api/* only (`run_worker_first`); every other path is a static file
// from web/dist, served without it. It signs writers in with Google or GitHub, keeps their
// stories in R2, one private bucket, and has Claude paint scenes for them. See CLOUD_PLAN.md for
// why, and ARCHITECTURE.md R33.
//
// Who is asking comes only from the session cookie, signed here; where a story is kept comes only
// from that and the story's checked id. Before anything reaches R2 the Ledger (ledger.js) says
// whether the limits allow it, so R2 stays inside its free amounts.
import { Ledger, key, limits } from "./ledger.js";

export { Ledger };

const SESSION = "__Host-caprice";
const SIGNING_IN = "__Host-caprice-signin";
const DAY = 86_400_000;
const STORY_ID = /^[0-9a-z-]{1,64}$/;
// The start of every .caprice file: an object whose first field is its format's version.
const CAPRICE = /^\s*\{\s*"version"\s*:\s*\d/;

export default {
  async fetch(request, env) {
    let response;
    try {
      response = await route(request, env);
    } catch (e) {
      // The reason only: never anything of a story.
      console.error("api:", new URL(request.url).pathname, e?.message);
      response = text(500, "Something went wrong");
    }
    return guarded(response);
  },
};

// Headers for every answer: nothing cached, nothing to sniff, frame or read from another site.
function guarded(response) {
  const r = new Response(response.body, response);
  r.headers.set("Cache-Control", "no-store");
  r.headers.set("X-Content-Type-Options", "nosniff");
  r.headers.set("Cross-Origin-Resource-Policy", "same-origin");
  r.headers.set("Content-Security-Policy", "default-src 'none'; frame-ancestors 'none'");
  r.headers.set("Referrer-Policy", "no-referrer");
  return r;
}

const text = (status, message, headers = {}) =>
  new Response(message, { status, headers: { "Content-Type": "text/plain; charset=utf-8", ...headers } });
const json = (value, headers = {}) => Response.json(value, { headers });

function refused(answer) {
  return text(answer.error, answer.message, answer.retry ? { "Retry-After": String(answer.retry) } : {});
}

async function route(request, env) {
  const url = new URL(request.url);
  const path = url.pathname;
  const method = request.method;

  const signIn = path.match(/^\/api\/(login|callback)\/(google|github)$/);
  if (signIn) {
    if (method !== "GET") return text(405, "GET only");
    return signIn[1] === "login" ? login(signIn[2], url, env) : callback(signIn[2], url, request, env);
  }

  // For the site's owner, with ADMIN_TOKEN, from a terminal: not the app, and no session.
  if (path === "/api/admin/scenes") return admin(request, env);

  // Everything else is asked by the app itself: a link or a form on another site sends no such
  // header, and the browser will not let another site's script add it without CORS, which we never allow.
  if (request.headers.get("X-Caprice") !== "1") return text(403, "Not from Caprice");
  if (method !== "GET" && request.headers.get("Origin") !== url.origin) return text(403, "Not from Caprice");

  if (path === "/api/logout") {
    if (method !== "POST") return text(405, "POST only");
    return new Response(null, { status: 204, headers: { "Set-Cookie": clear(SESSION) } });
  }
  // The ways to sign in that are set up, so the app offers only those.
  if (path === "/api/providers") return json(Object.keys(PROVIDERS).filter(p => PROVIDERS[p].id(env) && PROVIDERS[p].secret(env)));

  const session = await opened(request, SESSION, env);
  if (!session) return text(401, "Not signed in");
  // The user's id is no secret of theirs (a hash), and tells the app when someone else signs in.
  if (path === "/api/me") return json({ name: session.n, provider: session.p, user: session.u });
  if (path === "/api/scene") return method === "POST" ? scene(request, env, session.u) : text(405, "POST only");

  if (env.SYNC_OFF === "1") return text(503, "Stories online are switched off for now", { "Retry-After": "3600" });
  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  const user = session.u;

  if (path === "/api/stories") {
    if (method === "GET") {
      const answer = await ledger.list(user);
      return answer.ok ? json(answer.stories) : refused(answer);
    }
    if (method === "DELETE") {
      const answer = await ledger.removeAll(user);
      if (!answer.ok) return refused(answer);
      await forget(env, answer.drop);
      return new Response(null, { status: 204 });
    }
    return text(405, "GET or DELETE");
  }

  const story = path.match(/^\/api\/stories\/([^/]+)$/);
  if (!story) return text(404, "No such request");
  const id = story[1];
  if (!STORY_ID.test(id)) return text(400, "Not a story's id");

  if (method === "GET") {
    const answer = await ledger.current(user, id);
    if (!answer.ok) return refused(answer);
    const file = await env.STORIES.get(key(user, id, answer.version));
    if (!file) return text(404, "No such story online");
    return new Response(file.body, {
      headers: { "Content-Type": "application/json", "ETag": `"${answer.version}"`, "X-Caprice-Name": encodeURIComponent(answer.name) },
    });
  }
  if (method === "PUT") return put(request, env, ledger, user, id);
  if (method === "DELETE") {
    const answer = await ledger.remove(user, id, unquote(request.headers.get("If-Match")));
    if (!answer.ok) return refused(answer);
    await forget(env, answer.drop);
    return new Response(null, { status: 204 });
  }
  return text(405, "GET, PUT or DELETE");
}

async function put(request, env, ledger, user, id) {
  const most = limits(env).FILE_BYTES;
  const length = Number(request.headers.get("Content-Length"));
  if (!request.headers.has("Content-Length") || !Number.isInteger(length)) return text(411, "Say how long the story is");
  if (length > most) return text(413, "The story is too big to keep online");
  let name;
  try {
    name = decodeURIComponent(request.headers.get("X-Caprice-Name") ?? "Untitled").replace(/[\u0000-\u001f\u007f]/g, "").trim();
  } catch {
    return text(400, "Not a story's name");
  }
  if (!name || name.length > 200) return text(400, "Not a story's name");

  const body = await request.arrayBuffer();
  if (body.byteLength === 0 || body.byteLength > most) return text(413, "The story is too big to keep online");
  if (!CAPRICE.test(new TextDecoder().decode(body.slice(0, 64)))) return text(415, "Only Caprice stories can be kept online");

  const condition = { ifMatch: unquote(request.headers.get("If-Match")), ifNoneMatch: request.headers.get("If-None-Match")?.trim() };
  const begun = await ledger.begin(user, id, body.byteLength, condition);
  if (!begun.ok) return refused(begun);
  await forget(env, begun.drop);
  const where = key(user, id, begun.version);
  try {
    await env.STORIES.put(where, body, { httpMetadata: { contentType: "application/json" } });
  } catch (e) {
    await ledger.abort(user, id, begun.version);
    throw e;
  }
  const done = await ledger.commit(user, id, begun.version, name);
  if (!done.ok) {
    await forget(env, [where]);
    return refused(done);
  }
  await forget(env, done.drop);
  return json({ version: begun.version }, { "ETag": `"${begun.version}"` });
}

// Painting a scene. The app sends what to paint (backdrop.rs builds it, as on the desktop); Claude
// paints it with our key, which never leaves the Worker. The Ledger counts the scene before Claude
// is asked, and the request fixes the most it can cost, so the day's scenes bound the bill. Only
// the SVG goes back, so the key is of no use for anything but painting.
const PAINTER = "claude-opus-5-5";
const PAINTING = "You are a skilled illustrator who paints with SVG code. You make detailed, \
atmospheric illustrations for the pages of a story, in the manner of a classic book \
illustration: believable light, depth and texture rather than cartoon shapes.";
// Said in the picture itself, for anyone who finds it outside Caprice: it was made by an AI.
const MADE_BY = "<metadata>Made with AI: painted by Claude (Anthropic) for Caprice</metadata>";

async function scene(request, env, user) {
  if (env.SCENES_OFF === "1") return text(503, "Painting scenes is switched off for now", { "Retry-After": "3600" });
  if (!env.ANTHROPIC_API_KEY) return text(503, "Painting scenes is not set up", { "Retry-After": "3600" });
  const l = limits(env);
  const length = Number(request.headers.get("Content-Length"));
  if (!request.headers.has("Content-Length") || !Number.isInteger(length)) return text(411, "Say how long the request is");
  if (length > l.SCENE_PROMPT_BYTES) return text(413, "There is too much to paint at once");
  const prompt = await request.text();
  if (!prompt.trim() || new TextEncoder().encode(prompt).length > l.SCENE_PROMPT_BYTES) return text(413, "There is too much to paint at once");

  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  const allowed = await ledger.paint(user);
  if (!allowed.ok) return refused(allowed);
  const left = { "X-Caprice-Scenes-Left": String(allowed.left) };
  try {
    const painted = await paint(prompt, env, l.SCENE_TOKENS);
    if (painted.error) return text(painted.error, painted.message, painted.retry ? { ...left, "Retry-After": String(painted.retry) } : left);
    return new Response(painted.svg, { headers: { "Content-Type": "image/svg+xml; charset=utf-8", ...left } });
  } finally {
    await ledger.painted(user);
  }
}

// Claude's answer in one piece, not streamed: the Worker waits for it without using its CPU
// time (10 ms a request on the free plan), where reading thousands of streamed events would not.
// Thinking is adaptive (Opus cannot paint without it), at the effort the desktop paints with.
async function paint(prompt, env, most) {
  const asked = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: { "x-api-key": env.ANTHROPIC_API_KEY, "anthropic-version": "2023-06-01", "content-type": "application/json" },
    body: JSON.stringify({
      model: PAINTER, max_tokens: most, system: PAINTING,
      output_config: { effort: "medium" },
      messages: [{ role: "user", content: prompt }],
    }),
  });
  if (!asked.ok) {
    // The reason only: never anything of the story.
    const said = await asked.text();
    console.error("scene: Claude answered", asked.status, said.slice(0, 200));
    // The prepaid credit is used up (it is not reloaded by itself): painting waits until it is
    // topped up, and the writer is told so rather than that something broke.
    if (asked.status === 402 || /credit balance/i.test(said)) return { error: 503, message: "Painting is paused for now", retry: 3600 };
    return [429, 529].includes(asked.status)
      ? { error: 503, message: "Claude is busy: try again in a minute" }
      : { error: 502, message: "Claude could not be asked" };
  }
  const message = await asked.json();
  const stop = message.stop_reason;
  const reply = (message.content ?? []).filter(b => b.type === "text").map(b => b.text).join("");
  if (stop === "refusal") return { error: 422, message: "Claude declined to draw this scene" };
  const start = reply.indexOf("<svg");
  const end = reply.lastIndexOf("</svg>");
  if (start < 0 || end < start) return { error: 502, message: stop === "max_tokens" ? "The picture grew too big to finish" : "Claude's reply held no picture" };
  const svg = clean(reply.slice(start, end + "</svg>".length)).replace(/<svg\b[^>]*>/, open => open + MADE_BY);
  return { svg };
}

// What of an SVG could do more than draw, taken out. Caprice draws it with resvg, which runs no
// script, but a picture saved as .svg and opened in a browser would run them, and a story's text
// is in the prompt, so it could ask for one. Out go scripts, HTML inside the picture, `on...`
// handlers, `javascript:` and links to anything outside the picture (a browser would fetch them).
// Again until nothing changes, so that taking one out cannot join the pieces of another.
function clean(svg) {
  let before;
  do {
    before = svg;
    svg = svg
      .replace(/<(script|foreignObject)\b[\s\S]*?<\/\1\s*>/gi, "")
      .replace(/\s+on[a-z]+\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, "")
      .replace(/\s+(?:xlink:)?href\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi, (all, value) =>
        /^["']?\s*(#|data:image\/(png|jpeg|gif|webp))/i.test(value) ? all : "")
      .replace(/javascript\s*:/gi, "")
      .replace(/@import\b[^;]*;?/gi, "")
      .replace(/url\(\s*["']?\s*(?!#)[^)]*\)/gi, "none");
  } while (svg !== before);
  // A tag left open or closed alone: no script can follow it, but it goes too.
  return svg.replace(/<\/?(script|foreignObject)\b[^>]*>/gi, "");
}

// The scenes painted lately, by hashed id: `curl -H "Authorization: Bearer $TOKEN" <site>/api/admin/scenes`.
// Without a long enough ADMIN_TOKEN (a secret) there is no such request at all.
async function admin(request, env) {
  const token = env.ADMIN_TOKEN ?? "";
  if (token.length < 32) return text(404, "No such request");
  if (request.method !== "GET") return text(405, "GET only");
  const given = request.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1] ?? "";
  // Compared as hashes, in time that does not depend on how much of it was right.
  const [a, b] = await Promise.all([given, token].map(t => crypto.subtle.digest("SHA-256", new TextEncoder().encode(t))));
  if (!crypto.subtle.timingSafeEqual(a, b)) return text(401, "Not the site's owner");
  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  return json(await ledger.sceneDays());
}

// Deleting costs nothing in R2; it takes up to a thousand keys at once.
async function forget(env, keys) {
  for (let i = 0; i < keys.length; i += 1000) await env.STORIES.delete(keys.slice(i, i + 1000));
}

function unquote(tag) {
  if (tag === null) return undefined;
  return tag.trim().replace(/^W\//, "").replace(/^"(.*)"$/, "$1");
}

// Signing in, with the OAuth code flow and PKCE. Only the provider's own pages are shown; the app
// runs no script of theirs. We ask for the public profile only, never the email.

const PROVIDERS = {
  google: {
    authorize: "https://accounts.google.com/o/oauth2/v2/auth",
    scope: "openid profile",
    id: env => env.GOOGLE_CLIENT_ID,
    secret: env => env.GOOGLE_CLIENT_SECRET,
    // Google's answer comes straight from Google over TLS, for our secret: its id token need not
    // be checked for a signature, only for being meant for us.
    async who(code, verifier, redirect, env) {
      const answer = await (await fetch("https://oauth2.googleapis.com/token", {
        method: "POST",
        body: new URLSearchParams({
          code, code_verifier: verifier, redirect_uri: redirect, grant_type: "authorization_code",
          client_id: env.GOOGLE_CLIENT_ID, client_secret: env.GOOGLE_CLIENT_SECRET,
        }),
      })).json();
      const claims = JSON.parse(new TextDecoder().decode(unbase64(answer.id_token?.split(".")[1] ?? "")));
      if (claims.aud !== env.GOOGLE_CLIENT_ID || !["https://accounts.google.com", "accounts.google.com"].includes(claims.iss)) return null;
      return claims.sub ? { sub: String(claims.sub), name: claims.name || "Google user" } : null;
    },
  },
  github: {
    authorize: "https://github.com/login/oauth/authorize",
    scope: "",
    id: env => env.GITHUB_CLIENT_ID,
    secret: env => env.GITHUB_CLIENT_SECRET,
    async who(code, verifier, redirect, env) {
      const answer = await (await fetch("https://github.com/login/oauth/access_token", {
        method: "POST",
        headers: { Accept: "application/json" },
        body: new URLSearchParams({
          code, code_verifier: verifier, redirect_uri: redirect,
          client_id: env.GITHUB_CLIENT_ID, client_secret: env.GITHUB_CLIENT_SECRET,
        }),
      })).json();
      if (!answer.access_token) return null;
      const user = await (await fetch("https://api.github.com/user", {
        headers: { Authorization: `Bearer ${answer.access_token}`, Accept: "application/vnd.github+json", "User-Agent": "caprice-web" },
      })).json();
      return user.id ? { sub: String(user.id), name: user.name || user.login || "GitHub user" } : null;
    },
  },
};

async function login(provider, url, env) {
  const p = PROVIDERS[provider];
  if (!p.id(env) || !p.secret(env)) return text(404, `Signing in with ${provider} is not set up`);
  const state = base64(random(32));
  const verifier = base64(random(32));
  const challenge = base64(new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier))));
  const to = new URL(p.authorize);
  to.search = new URLSearchParams({
    client_id: p.id(env), redirect_uri: `${url.origin}/api/callback/${provider}`, response_type: "code",
    scope: p.scope, state, code_challenge: challenge, code_challenge_method: "S256",
  });
  const cookie = await sealed({ p: provider, s: state, v: verifier, x: Date.now() + 10 * 60_000 }, env);
  return new Response(null, { status: 302, headers: { Location: to.href, "Set-Cookie": set(SIGNING_IN, cookie, 600) } });
}

async function callback(provider, url, request, env) {
  const p = PROVIDERS[provider];
  if (!p.id(env) || !p.secret(env)) return text(404, `Signing in with ${provider} is not set up`);
  // Turned back at the provider's page: home again, signed out.
  if (url.searchParams.has("error")) return home([clear(SIGNING_IN)]);
  const asked = await opened(request, SIGNING_IN, env);
  const code = url.searchParams.get("code");
  if (!asked || asked.p !== provider || !code || url.searchParams.get("state") !== asked.s) {
    return text(400, "That sign-in was not started here, or took too long. Please try again.");
  }
  const who = await p.who(code, asked.v, `${url.origin}/api/callback/${provider}`, env);
  if (!who) return text(502, `${provider} did not say who you are. Please try again.`);
  const user = await hex(`${provider}:${who.sub}`);
  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  const answer = await ledger.signIn(user);
  if (!answer.ok) return refused(answer);
  const name = who.name.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 60);
  const session = await sealed({ u: user, n: name, p: provider, x: Date.now() + 30 * DAY }, env);
  return home([set(SESSION, session, 30 * DAY / 1000), clear(SIGNING_IN)]);
}

function home(cookies) {
  const headers = new Headers({ Location: "/" });
  for (const c of cookies) headers.append("Set-Cookie", c);
  return new Response(null, { status: 302, headers });
}

// Cookies the page's scripts cannot read, sent to this site only, and over HTTPS only.
const set = (name, value, seconds) => `${name}=${value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=${seconds}`;
const clear = name => set(name, "", 0);

// A cookie's value: what it holds, then its HMAC with SESSION_SECRET. Without a long enough
// secret nothing can be signed or opened, and the request fails rather than trusts.
async function signing(env) {
  if (!env.SESSION_SECRET || env.SESSION_SECRET.length < 32) throw new Error("SESSION_SECRET is not set");
  return crypto.subtle.importKey("raw", new TextEncoder().encode(env.SESSION_SECRET), { name: "HMAC", hash: "SHA-256" }, false, ["sign", "verify"]);
}

async function sealed(value, env) {
  const body = base64(new TextEncoder().encode(JSON.stringify(value)));
  const mac = await crypto.subtle.sign("HMAC", await signing(env), new TextEncoder().encode(body));
  return `${body}.${base64(new Uint8Array(mac))}`;
}

async function opened(request, name, env) {
  const cookie = (request.headers.get("Cookie") ?? "").split(";").map(c => c.trim()).find(c => c.startsWith(`${name}=`));
  const [body, mac] = cookie?.slice(name.length + 1).split(".") ?? [];
  if (!body || !mac) return null;
  let value;
  try {
    if (!await crypto.subtle.verify("HMAC", await signing(env), unbase64(mac), new TextEncoder().encode(body))) return null;
    value = JSON.parse(new TextDecoder().decode(unbase64(body)));
  } catch (e) {
    if (e.message === "SESSION_SECRET is not set") throw e;
    return null;
  }
  return value && value.x > Date.now() ? value : null;
}

const random = n => crypto.getRandomValues(new Uint8Array(n));

async function hex(text) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
  return [...digest].map(b => b.toString(16).padStart(2, "0")).join("").slice(0, 32);
}

function base64(bytes) {
  return btoa(String.fromCharCode(...bytes)).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function unbase64(text) {
  const plain = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
  return Uint8Array.from(plain, c => c.charCodeAt(0));
}
