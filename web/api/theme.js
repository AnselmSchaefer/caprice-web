// Making a theme program with Claude (ARCHITECTURE.md R36, THEME_PROGRAMS_PLAN.md step 5): the
// writer says what they want, Claude writes one Rust file against Caprice's theme SDK, the writer's
// browser compiles it and plays it off screen, and Claude is shown what came of it: the compiler's
// messages, or pictures of it at a few moments of a story with its points. A few rounds, then the
// writer keeps it, or asks for a change.
//
// It must never become a way to ask Claude anything on Caprice's key. So the Worker writes the whole
// prompt (the guide written for this, the SDK, the built-in themes ported as examples), the writer's
// words are one capped field in it, the conversation is kept here (in the Ledger) so the browser
// cannot write Claude's side of it, a round is one of four kinds (`errors`, `checks`, `frames`, `change`),
// each capped in size and in number, and what goes back is only the program's source and its
// palette, never Claude's free text. The Ledger counts every round before Claude is asked, and the
// request fixes the most it can cost.
import guide from "../../theme-sdk/GUIDE.md";
import sdk from "../../theme-sdk/src/lib.rs";
import world from "../../theme-sdk/wit/theme.wit";
import fireflies from "../../themes/fireflies/theme.rs";
import snow from "../../themes/snow/theme.rs";
import vineyard from "../../themes/vineyard/theme.rs";
import { limits } from "./ledger.js";
import { COLOURS } from "./theme-file.js";
import { isOwner } from "./gallery.js";

const WRITER = "claude-opus-5-5";
// What a token costs, in dollars a million (Anthropic's prices, 2026-11): input, the cache written
// for an hour (twice the input's), read from the cache, and output (thinking included). A round the
// platform's refusal fallback answered with another model is counted at the writer's prices.
const PRICES = { "claude-opus-5-5": { input: 4, written: 8, read: 0.2, output: 20 } };
// A theme's id, given by the Ledger when one is begun.
export const THEME_ID = /^[0-9a-f]{16}$/;
// The palette's names (src/own.rs, NAMES), and "light".

// The prompt, the same for every theme and every round, so it is cached after the first.
const SYSTEM = [
  "You write magic themes for Caprice, a word processor for writing stories, as Rust programs against its theme SDK.",
  guide,
  "## The SDK (theme-sdk/src/lib.rs)\n\n```rust\n" + sdk + "\n```",
  "## The interface it is built on (theme-sdk/wit/theme.wit)\n\n```wit\n" + world + "\n```",
  "## A small example: fireflies in jars, a jar a chapter, a firefly a page, carrying titles\n\n```rust\n" + fireflies + "\n```",
  "## The Magic Snow: igloos by a frozen lake, a block a page, snow falling in the window\n\n```rust\n" + snow + "\n```",
  "## The Magic Vineyard: its own sky and hills, a stretch of vines a chapter, grapes a page, starlings\n\n```rust\n" + vineyard + "\n```",
].join("\n\n");

const text = (status, message, headers = {}) =>
  new Response(message, { status, headers: { "Content-Type": "text/plain; charset=utf-8", ...headers } });

function refused(answer) {
  return text(answer.error, answer.message, answer.retry ? { "Retry-After": String(answer.retry) } : {});
}

// The body, read only so far as it may be: its length said first, and held to it.
export async function body(request, most) {
  const length = Number(request.headers.get("Content-Length"));
  if (!request.headers.has("Content-Length") || !Number.isInteger(length)) return { error: 411, message: "Say how long the request is" };
  if (length > most) return { error: 413, message: "There is too much in it" };
  const bytes = new Uint8Array(await request.arrayBuffer());
  if (bytes.length > most) return { error: 413, message: "There is too much in it" };
  return { bytes };
}

const words = (bytes) => new TextDecoder().decode(bytes).trim();

// The pictures of a round, as the browser sends them: how many (a byte), each one's length (four
// bytes, little-endian), then the PNGs. Each is checked to be a PNG of a sensible size, never
// decoded; they go to Claude as they came, made base64 here (as JSON from the browser, parsing and
// writing them again cost the Worker more than its 10 ms: the plan's step 0d).
function pictures(bytes) {
  const count = bytes[0];
  if (!count || count > 6 || bytes.length < 1 + 4 * count) return null;
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out = [];
  let at = 1 + 4 * count;
  for (let k = 0; k < count; k++) {
    const length = view.getUint32(1 + 4 * k, true);
    if (length < 33 || at + length > bytes.length) return null;
    const png = bytes.subarray(at, at + length);
    const signed = [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => png[i] === b);
    const header = new DataView(png.buffer, png.byteOffset, 24);
    const [w, h] = [header.getUint32(16), header.getUint32(20)];
    if (!signed || w < 64 || h < 64 || w > 1600 || h > 1600) return null;
    out.push(png);
    at += length;
  }
  return at === bytes.length ? out : null;
}

// What the writer may have changed (a round's words) as Claude is told it.
const asked = {
  begin: (said) => `<request>\n${said}\n</request>\n\nWrite this theme. Answer as the guide says: the palette's json block, then the whole rust file.`,
  errors: (said) => `It did not build. The compiler said:\n\n\`\`\`\n${said}\n\`\`\`\n\nFix it, and give the palette and the whole file again.`,
  // It built, but Caprice found it does not do what every theme must (the load check, playing a
  // story through, the budget of points): what it found, in its own words.
  checks: (said) => `It built, but Caprice's checks found:\n\n<checks>\n${said}\n</checks>\n\nFix it, and give the palette and the whole file again.`,
  change: (said) => `The writer asks for a change:\n\n<change>\n${said}\n</change>\n\nMake it, and give the palette and the whole file again.`,
  frames: (moments, points) => `It built. Here it is as Caprice drew it, at these moments of a story: ${moments.join(", ")}. `
    + `At most ${points.phone} points a frame on a phone's desk, ${points.computer} on a computer's (zoomed out included). `
    + "Look at it as the writer will, beside their page for hours, and make it better. Is it as finished as the guide asks: "
    + "depth from far to near, light from one side, small parts on the things that grow with the story? Does it reach the "
    + "window's edges zoomed out, with no frame of the plain desk around it? Mend what is wrong, unclear, ugly or too heavy "
    + "first, then make it lovelier, within the points. Give the palette and the whole file again.",
};

// Making a theme: `id` none to begin one (the writer's request), else a round of `kind`.
export async function themeProgram(request, env, user, id) {
  if (env.THEMES_OFF === "1") return text(503, "Making themes is switched off for now", { "Retry-After": "3600" });
  if (!env.ANTHROPIC_API_KEY) return text(503, "Making themes is not set up", { "Retry-After": "3600" });
  const l = limits(env);
  const kind = id ? request.headers.get("X-Caprice-Kind") : "begin";
  const most = { begin: l.THEME_REQUEST_BYTES, change: l.THEME_REQUEST_BYTES, errors: l.THEME_ERRORS_BYTES, checks: l.THEME_ERRORS_BYTES, frames: l.THEME_FRAMES_BYTES }[kind];
  if (!most) return text(400, "Not a kind of round");
  const got = await body(request, most);
  if (got.error) return refused(got);

  // What is said this round: its words for Claude, and what of it is kept in the conversation.
  let content, kept;
  if (kind === "frames") {
    const pngs = pictures(got.bytes);
    const moments = (request.headers.get("X-Caprice-Moments") ?? "").split(",").filter(m => /^[a-z ]{1,24}$/.test(m));
    const [phone, computer] = (request.headers.get("X-Caprice-Points") ?? "").split(",").map(Number);
    if (!pngs || moments.length !== pngs.length || !Number.isInteger(phone) || !Number.isInteger(computer)) return text(400, "Not pictures of a theme");
    const said = asked.frames(moments, { phone, computer });
    content = [...pngs.map(png => ({ type: "image", source: { type: "base64", media_type: "image/png", data: png.toBase64() } })), { type: "text", text: said }];
    kept = `[${pngs.length} pictures were shown here.] ${said}`;
  } else {
    const said = words(got.bytes);
    if (!said) return text(400, "Nothing was said");
    kept = asked[kind](said);
    content = kept;
  }

  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  // Begun simple (`light`) or `rich`, as the writer chose; later rounds are of the theme's tier.
  const tier = request.headers.get("X-Caprice-Tier") === "rich" ? "rich" : "light";
  // The site's owner tries themes out without a day's limits (the Ledger's `themeRound`).
  const round = await ledger.themeRound(user, id ?? null, kind, undefined, tier, isOwner(env, user));
  if (!round.ok) return refused(round);
  const answer = (async () => {
    let answered = null, spent = null, done;
    try {
      const messages = [...round.turns.map(t => ({ role: t.role, content: t.text })), { role: "user", content }];
      const rich = round.tier === "rich";
      const effort = rich ? env.THEME_EFFORT_RICH || "medium" : env.THEME_EFFORT || "low";
      const written = await write(messages, env, rich ? l.THEME_TOKENS_RICH : l.THEME_TOKENS, rich ? l.THEME_BUDGET_RICH : l.THEME_BUDGET, effort);
      spent = written.spent ?? null;
      if (written.error) {
        done = written;
      } else {
        answered = { asked: kept, answer: written.reply };
        done = { theme: { theme: round.id, ...written.theme, left: round.left } };
      }
    } catch (e) {
      console.error("theme: the round failed", String(e).slice(0, 200));
      done = { error: 502, message: "Something went wrong while the theme was written: try again" };
    }
    const after = await ledger.themeAnswered(user, round.id, answered, spent);
    // A theme dear enough already is offered no look at its pictures (THEME_LOOK_MICRO): the app
    // shows it to the writer as it is.
    if (done.theme && after?.look === false) done.theme.left = { ...done.theme.left, frames: 0 };
    return done;
  })();
  return keptAlive(answer, Number(env.THEME_BEAT_MS) || BEAT_MS);
}

// A request weighed before it is made, so the writer can be told whether it looks simple or rich
// (and choose): a small model counts its parts, as the measurements were counted, and the Worker
// sums them. It answers only `{ tier, parts }`, never words: like a round, it is no way to ask
// Claude anything. Anything that goes wrong is `{ tier: null }`, and the writer chooses unhelped.
const WEIGHER = "claude-haiku-5-5";
const WEIGHING = "You judge how much work a desk theme for a writing app will be to program, before it is written. "
  + "A theme draws a scene beside the page: something grows a part for each page written and a new thing for each chapter. "
  + "Count in the request: things (distinct kinds of drawn objects, e.g. lighthouse, boat, sea, sky), growth (what is added "
  + "per page or per chapter), events (one-off animations, e.g. something leaves when a chapter begins), motion (things "
  + "that travel between places or recur, e.g. lights flying to a boat, a whale passing, lanterns at dusk).";
const PARTS = ["things", "growth", "events", "motion"];

export async function themeEstimate(request, env, user) {
  if (env.THEMES_OFF === "1") return text(503, "Making themes is switched off for now", { "Retry-After": "3600" });
  if (!env.ANTHROPIC_API_KEY) return text(503, "Making themes is not set up", { "Retry-After": "3600" });
  const l = limits(env);
  const got = await body(request, l.THEME_REQUEST_BYTES);
  if (got.error) return refused(got);
  const said = words(got.bytes);
  if (!said) return text(400, "Nothing was said");
  const counted = await env.LEDGER.get(env.LEDGER.idFromName("ledger")).themeWeighed(user, undefined, isOwner(env, user));
  if (!counted.ok) return refused(counted);
  const unknown = Response.json({ tier: null });
  try {
    const sent = await fetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: { "x-api-key": env.ANTHROPIC_API_KEY, "anthropic-version": "2023-06-01", "content-type": "application/json" },
      body: JSON.stringify({
        model: WEIGHER, max_tokens: 2000,
        system: WEIGHING,
        output_config: {
          effort: "low",
          format: { type: "json_schema", schema: { type: "object", properties: Object.fromEntries(PARTS.map(p => [p, { type: "integer" }])), required: PARTS, additionalProperties: false } },
        },
        messages: [{ role: "user", content: `<request>\n${said}\n</request>` }],
      }),
    });
    if (!sent.ok) return unknown;
    const message = await sent.json();
    if (message.stop_reason !== "end_turn") return unknown;
    const n = JSON.parse((message.content ?? []).filter(b => b.type === "text").map(b => b.text).join(""));
    const parts = PARTS.reduce((sum, p) => sum + (Number.isInteger(n[p]) && n[p] >= 0 && n[p] < 100 ? n[p] : NaN), 0);
    if (!Number.isFinite(parts)) return unknown;
    return Response.json({ tier: parts >= l.THEME_RICH_FROM ? "rich" : "light", parts });
  } catch (e) {
    console.error("theme: weighing failed", String(e).slice(0, 200));
    return unknown;
  }
}

// A beat, in milliseconds: how often a round still being written says so.
const BEAT_MS = 15000;

// A round takes minutes, and for all of them the writer's browser would hear nothing: it could not
// tell a theme still being written from a connection lost, and a quiet connection may be dropped
// on the way. So a round not answered within a beat is answered at once, and a space is sent every
// beat until the theme comes (the window counts a minute without one as the connection lost). JSON
// may begin with spaces, so the body is still one object: the theme, or what went wrong as
// `{ error, message, retry }`, since its status was sent before it was known. A round answered
// within the beat is answered as before, its status saying what went wrong. Sending spaces costs
// the Worker next to nothing of its 10 ms; `no-transform` and the type keep Cloudflare from holding
// them back to compress them.
async function keptAlive(answer, beat) {
  const first = await Promise.race([answer, new Promise(done => setTimeout(() => done(null), beat))]);
  if (first) return first.theme ? Response.json(first.theme) : refused(first);
  const { readable, writable } = new TransformStream();
  const out = writable.getWriter();
  const space = new TextEncoder().encode(" ");
  const sent = () => out.write(space).catch(() => {});
  sent();
  const beating = setInterval(sent, beat);
  answer.then(async a => {
    clearInterval(beating);
    const last = a.theme ?? { error: a.error, message: a.message, ...(a.retry ? { retry: a.retry } : {}) };
    await out.write(new TextEncoder().encode(JSON.stringify(last)));
    await out.close();
  }).catch(() => clearInterval(beating));
  return new Response(readable, { status: 200, headers: { "Content-Type": "text/event-stream", "Cache-Control": "no-store, no-transform" } });
}

// Claude's answer in one piece, not streamed, as a scene is (worker.js, `paint`): waiting costs the
// Worker none of its 10 ms. Thinking is adaptive; the prompt is cached, and the conversation too.
// `max_tokens` is a wall Claude cannot see: thinking that runs into it leaves no program, and the
// round is paid for all the same. So a rich theme is also told its room (`task_budget`, `budget`),
// which it sees counting down and finishes within, and `most` stays the most a round can cost. A
// simple one (`budget` 0) is told none: it fits without, and a tight budget can cut its answer short.
// For an hour, not the usual five minutes: a round takes seven or more, and the five are counted
// from when it was asked, so the next round would always find the cache gone and write it again.
// Writing it for an hour costs more once (twice the input's price instead of 1.25 times), and is
// paid back by the next round. The prompt is the same for every writer, so any theme begun within
// the hour reads it too; the conversation only that theme's next rounds.
const CACHED = { type: "ephemeral", ttl: "1h" };

async function write(messages, env, most, budget, effort) {
  const sent = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "x-api-key": env.ANTHROPIC_API_KEY, "anthropic-version": "2023-06-01", "content-type": "application/json",
      // Should the model decline, the request goes on with another the platform chooses.
      // And, given a budget, it is told the room it has.
      "anthropic-beta": budget > 0 ? "server-side-fallback-2026-07-01,task-budgets-2026-03-13" : "server-side-fallback-2026-07-01",
    },
    body: JSON.stringify({
      model: WRITER, max_tokens: most,
      system: [{ type: "text", text: SYSTEM, cache_control: CACHED }],
      output_config: { effort, ...(budget > 0 ? { task_budget: { type: "tokens", total: budget } } : {}) },
      cache_control: CACHED,
      fallbacks: "default",
      messages,
    }),
  });
  if (!sent.ok) {
    const said = await sent.text();
    console.error("theme: Claude answered", sent.status, said.slice(0, 200));
    if (sent.status === 402 || /credit balance/i.test(said)) return { error: 503, message: "Making themes is paused for now", retry: 3600 };
    return [429, 529].includes(sent.status)
      ? { error: 503, message: "Many themes are being made just now: try again in a few minutes" }
      : { error: 502, message: "The theme could not be begun just now: try again later" };
  }
  const message = await sent.json();
  // What the round cost, kept in the Ledger with the theme (and for `wrangler tail`): whether the
  // cache was read or written again.
  const spent = cost(message.usage ?? {});
  console.log("theme: a round", JSON.stringify(spent));
  if (message.stop_reason === "refusal") return { error: 422, message: "This theme cannot be made: ask for something else", spent };
  const reply = (message.content ?? []).filter(b => b.type === "text").map(b => b.text).join("");
  const theme = read(reply);
  // Why a reply had no program, for Workers Logs: how it stopped, what it spent, which model answered
  // (the fallback's?), whether a rust block or `theme!(` was in it at all, and how it ended. Never the
  // writer's words, which come first in no reply.
  if (!theme) {
    console.error("theme: no program", JSON.stringify({ stop: message.stop_reason, model: message.model, output: spent.output,
      chars: reply.length, rust: (reply.match(/```(rust|rs)\b/g) ?? []).length, macro: reply.includes("theme!("), end: reply.slice(-300) }));
  }
  if (!theme) return { error: 502, message: message.stop_reason === "max_tokens" ? "The theme ran out of room before it was finished. Starting again may well work" : "Something went wrong while the theme was written: try again", spent };
  return { theme, reply, spent };
}

// A round's tokens, and what they cost in millionths of a dollar.
export function cost(u) {
  const n = v => (Number.isInteger(v) && v > 0 ? v : 0);
  const spent = { input: n(u.input_tokens), written: n(u.cache_creation_input_tokens), read: n(u.cache_read_input_tokens), output: n(u.output_tokens) };
  const p = PRICES[WRITER];
  spent.micro = Math.round(spent.input * p.input + spent.written * p.written + spent.read * p.read + spent.output * p.output);
  return spent;
}

// The program and its palette out of Claude's reply, and nothing else of it: the last rust block,
// and the json block's name, few words and colours, each checked.
export function read(reply) {
  const blocks = [...reply.matchAll(/```(\w*)[ \t]*\r?\n([\s\S]*?)\r?\n```/g)].map(m => ({ lang: m[1].toLowerCase(), body: m[2] }));
  const source = blocks.filter(b => b.lang === "rust" || b.lang === "rs").at(-1)?.body;
  if (!source || !source.includes("theme!(")) return null;
  let palette = {};
  try {
    palette = JSON.parse(blocks.find(b => b.lang === "json")?.body ?? "{}");
  } catch {
    palette = {};
  }
  const colours = {};
  for (const name of COLOURS) {
    const c = palette?.colours?.[name];
    if (typeof c === "string" && /^#[0-9a-fA-F]{6}$/.test(c)) colours[name] = c.toLowerCase();
  }
  if (palette?.colours?.light === "yes" || palette?.colours?.light === "no") colours.light = palette.colours.light;
  const short = (s, n) => (typeof s === "string" ? s.replace(/[\u0000-\u001f]/g, " ").trim().slice(0, n) : "");
  return { source, name: short(palette?.name, 60), about: short(palette?.about, 140), colours };
}
