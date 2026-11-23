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
// bytes, little-endian), then the PNGs, then what Caprice's checks found of how it looks, in words
// (at most `most` bytes; none for a version that passed them all). Each picture is checked to be a
// PNG of a sensible size, never decoded; they go to Claude as they came, made base64 here (as JSON
// from the browser, parsing and writing them again cost the Worker more than its 10 ms: the plan's
// step 0d). The words go to Claude as a `checks` round's do, inside a sentence the Worker wrote.
function pictures(bytes, most) {
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
  if (bytes.length - at > most) return null;
  return { pngs: out, found: words(bytes.subarray(at)) };
}

// The desk a hand-drawn sketch is laid out on (matching src/eggs/preview.rs `layout(PICTURE)`).
const DESK_W = 1280, DESK_H = 800;
const DESK_ZOOM = (DESK_H - 2 * 72 - 16) / 842;
const DESK_PW = 595 * DESK_ZOOM, DESK_PH = 842 * DESK_ZOOM;
const DESK_PAGE = { x: (DESK_W - DESK_PW) / 2, y: (DESK_H - DESK_PH) / 2, w: DESK_PW, h: DESK_PH };
const DESK_BAR_Y = DESK_H - 14 - 48;
const DESK_UNIT = Math.min(Math.max(DESK_H / 900, 0.6), 1.6);
const SKETCH_INKS = ["black", "blue", "red", "green"];
const SKETCH_PNG_BYTES = 1_400_000;
const SKETCH_JSON_BYTES = 200_000;
const MOST_SKETCH_STROKES = 500;
const MOST_SKETCH_POINTS = 30_000;
const MOST_STROKES_TEXT = 14_000;

function simplified(pts, eps) {
  if (pts.length < 3) return pts;
  const keep = new Uint8Array(pts.length);
  keep[0] = keep[pts.length - 1] = 1;
  const spans = [[0, pts.length - 1]];
  while (spans.length) {
    const [a, b] = spans.pop();
    const [ax, ay] = pts[a], dx = pts[b][0] - ax, dy = pts[b][1] - ay, len = Math.hypot(dx, dy);
    let far = 0, at = -1;
    for (let i = a + 1; i < b; i++) {
      const [x, y] = pts[i];
      const d = len > 0.5 ? Math.abs((x - ax) * dy - (y - ay) * dx) / len : Math.hypot(x - ax, y - ay);
      if (d > far) [far, at] = [d, i];
    }
    if (far > eps) {
      keep[at] = 1;
      spans.push([a, at], [at, b]);
    }
  }
  return pts.filter((_, i) => keep[i]);
}

function strokeList(strokes) {
  const n = Math.round;
  const ux = x => n((x - DESK_W / 2) / DESK_UNIT), uy = y => n((DESK_BAR_Y - y) / DESK_UNIT);
  for (let eps = 2; ; eps *= 1.6) {
    const said = strokes.map(s => `${s.ink} ${n(s.width / DESK_UNIT)}: ${simplified(s.pts, eps).map(p => `${ux(p[0])},${uy(p[1])}`).join(" ")}`).join("\n");
    if (said.length <= MOST_STROKES_TEXT) return said;
    if (eps > 40) return `${said.slice(0, MOST_STROKES_TEXT)}\n(and more: see the pictures)`;
  }
}

// Validate a hand-drawn sketch sent to begin a theme: 1..5 PNGs (whole, then one per ink if >1 ink)
// followed by JSON `{ words, strokes }`. Only validated ink names and numbers enter the prompt.
function readSketch(bytes, l) {
  const got = pictures(bytes, SKETCH_JSON_BYTES);
  if (!got || got.pngs.length > 5 || got.pngs.some(p => p.length > SKETCH_PNG_BYTES)) return null;
  let data;
  try { data = JSON.parse(got.found); } catch { return null; }
  if (!data || typeof data !== "object") return null;
  const said = data.words === undefined ? "" : typeof data.words === "string" ? data.words.trim() : null;
  if (said === null || new TextEncoder().encode(said).length > l.THEME_REQUEST_BYTES) return null;
  if (!Array.isArray(data.strokes) || !data.strokes.length || data.strokes.length > MOST_SKETCH_STROKES) return null;
  let total = 0;
  const strokes = [];
  for (const s of data.strokes) {
    if (!s || !SKETCH_INKS.includes(s.ink) || typeof s.width !== "number" || !Number.isFinite(s.width) || s.width < 1 || s.width > 40) return null;
    if (!Array.isArray(s.pts) || !s.pts.length || s.pts.length > 2000) return null;
    total += s.pts.length;
    if (total > MOST_SKETCH_POINTS) return null;
    const pts = [];
    for (const p of s.pts) {
      if (!Array.isArray(p) || p.length < 2 || typeof p[0] !== "number" || typeof p[1] !== "number" || !Number.isFinite(p[0]) || !Number.isFinite(p[1])) return null;
      pts.push([Math.min(DESK_W, Math.max(0, p[0])), Math.min(DESK_H, Math.max(0, p[1]))]);
    }
    strokes.push({ ink: s.ink, width: s.width, pts });
  }
  const used = SKETCH_INKS.filter(k => strokes.some(s => s.ink === k));
  const expected = used.length > 1 ? 1 + used.length : 1;
  if (got.pngs.length !== expected) return null;
  return { pngs: got.pngs, words: said, strokes, used };
}

function sketchPrompt({ words: said, strokes, used }) {
  const n = Math.round;
  const ux = x => n((x - DESK_W / 2) / DESK_UNIT), uy = y => n((DESK_BAR_Y - y) / DESK_UNIT);
  return [
    `The writer drew the theme they want by hand. The first picture is the whole sketch, on a desk ${DESK_W} by ${DESK_H}; the faint border in it is the page and is no part of the drawing.`
      + (used.length > 1 ? ` After it, the same sketch one ink at a time: ${used.join(", ")}.` : ""),
    said ? `They say of it:\n\n<request>\n${said}\n</request>` : "They say nothing more of it: see for yourself what it shows.",
    [
      "**The inks say when a part moves, never what colour it is.** Give everything the colours the world asks for, and none the ink's because it was drawn in it: a wave in green is the sea's colour.",
      "- **Black** is what is always there and stays still: the land, the sky's things, buildings, trees.",
      "- **Blue** is what happens when a new page is written (`Event::Grown`, the garden's pages): it comes, grows, lights or moves then, and stays as it has become. Several alike are a page each, in the order they were drawn; go on in the same way for more pages than were drawn, and let a chapter's first page be the first of them.",
      "- **Red** is what happens when a new chapter begins (`Event::Begun`, the garden's chapters): it comes, or does what it does, then. A thing in red is the thing a chapter; give the chapters after the ones drawn theirs in the same way, where there is room.",
      "- **Green** is what moves all the time, whatever the story does: slowly and calmly, never resting the eye's whole attention.",
      "A part of one thing in another ink moves as its ink says (a black house with a blue window is a house whose window lights with a page). A line in blue, red or green that shows a way rather than a thing (an arrow, a dotted line, a swirl beside something) is how the thing near it in that ink moves: follow it, and do not draw it.",
    ].join("\n"),
    [
      "**Keep their composition.** What they drew on the left of the page stands on the left of it, as large beside the page as they drew it, as high above the ground, in front of what they drew it in front of. The picture is one desk: place things from the page's edges, the ground and the desk's, so it holds on others too, as the guide asks. "
        + "Their strokes below are in units already, so nothing needs converting: each point is x from the page's middle (left of it is less than 0) and y up from the top of the page bar, and goes on any desk at\n\n"
        + "```rust\nlet foot = input.bar.map_or(input.area.bottom(), |b| b.top());\nlet at = |x: f32, y: f32| pt(input.page.center().x + x * input.unit, foot - y * input.unit);\n```\n\n"
        + `Place and size the things they drew by their numbers through that, and they land where they were drawn. In these numbers the page is x ${ux(DESK_PAGE.x)} to ${ux(DESK_PAGE.x + DESK_PAGE.w)} and y ${uy(DESK_PAGE.y + DESK_PAGE.h)} to ${uy(DESK_PAGE.y)}, and the sketch's desk x ${ux(0)} to ${ux(DESK_W)} and y ${uy(DESK_H)} to ${uy(0)}; below 0 is behind the page bar. Where a desk is narrower (a phone), bring things in as the guide asks. `,
      "**What they drew inside the page's border** lies behind the page, seen when it is dragged aside and behind the welcome: draw it there, and never leave it out. Red drawn inside the border that reads as something coming to the page or carrying something over it is the carrier (`host::ask_lift`, `Out::over`).",
      "**It is a sketch, not the drawing.** See what each thing is meant to be and draw that, finished as the guide asks; never their shaky lines. What they left out (a sky, a ground, the far distance, light) is yours to add, in keeping, and still. What the guide asks of every theme holds here too: if a chapter or a page has nothing drawn for it, find the smallest thing in the sketch that can show it.",
    ].join("\n\n"),
    `Their strokes, as drawn in order: ink, width, then points as x,y, all in units as said above.\n\n<strokes>\n${strokeList(strokes)}\n</strokes>`,
    "Write this theme. Answer as the guide says: the palette's json block, then the whole rust file.",
  ].join("\n\n");
}

// What the writer may have changed (a round's words) as Claude is told it.
const asked = {
  begin: (said) => `<request>\n${said}\n</request>\n\nWrite this theme. Answer as the guide says: the palette's json block, then the whole rust file.`,
  errors: (said) => `It did not build. The compiler said:\n\n\`\`\`\n${said}\n\`\`\`\n\nFix it, and give the palette and the whole file again.`,
  // It built, but Caprice found it does not do what every theme must (the load check, playing a
  // story through, the budget of points): what it found, in its own words.
  checks: (said) => `It built, but Caprice's checks found:\n\n<checks>\n${said}\n</checks>\n\nFix it, and give the palette and the whole file again.`,
  change: (said) => `The writer asks for a change:\n\n<change>\n${said}\n</change>\n\nMake it, and give the palette and the whole file again.`,
  frames: (moments, points, found, hasSketch = false) => `It built. Here it is as Caprice drew it, at these moments of a story: ${moments.join(", ")}. `
    + (found ? `Caprice's checks found, which you mend first:\n\n<checks>\n${found}\n</checks>\n\n` : "")
    + `At most ${points.phone} points a frame on a phone's desk, ${points.computer} on a computer's (zoomed out included). `
    + (hasSketch ? "Hold it against the sketch: is every thing they drew there, where they drew it, moving when its ink says? Mend that first. " : "")
    + "Look at it as the writer will, beside their page for hours, and make it better. Is it as finished as the guide asks: "
    + "depth from far to near, light from one side, small parts on the things that grow with the story? Does it reach the "
    + "window's edges zoomed out, with no frame of the plain desk around it? Is the world whole without the page, nothing in it "
    + "ending where the page's edge lies? Mend what is wrong, unclear, ugly or too heavy "
    + "first, then make it lovelier, within the points. Give the palette and the whole file again.",
};

// The gist of what Caprice found, as a round's log line gives it: the compiler's errors, or the first
// words of each thing the checks found, a few of each. None for the writer's own words.
function gist(kind, said) {
  if (!said || (kind !== "errors" && kind !== "checks" && kind !== "frames")) return [];
  const parts = kind === "errors" ? said.split("\n").filter(line => /^error(\[|: )/.test(line) && !line.startsWith("error: aborting"))
    : said.split(/\n\s*\n/);
  return parts.map(p => p.trim().replace(/\s+/g, " ").slice(0, 160)).filter(Boolean).slice(0, 4);
}

// Making a theme: `id` none to begin one (the writer's request, or a hand-drawn sketch), else a
// round of `kind`.
export async function themeProgram(request, env, user, id) {
  if (env.THEMES_OFF === "1") return text(503, "Making themes is switched off for now", { "Retry-After": "3600" });
  if (!env.ANTHROPIC_API_KEY) return text(503, "Making themes is not set up", { "Retry-After": "3600" });
  const l = limits(env);
  const isSketch = !id && request.headers.get("X-Caprice-Kind") === "sketch";
  const kind = id ? request.headers.get("X-Caprice-Kind") : "begin";
  const most = isSketch
    ? l.THEME_FRAMES_BYTES
    : { begin: l.THEME_REQUEST_BYTES, change: l.THEME_REQUEST_BYTES, errors: l.THEME_ERRORS_BYTES, checks: l.THEME_ERRORS_BYTES, frames: l.THEME_FRAMES_BYTES }[kind];
  if (!most) return text(400, "Not a kind of round");
  const got = await body(request, most);
  if (got.error) return refused(got);

  // Validate before asking the Ledger so bad requests never count as a round.
  let content, kept, found, sketchPngs = null, frameData = null;
  if (isSketch) {
    const parsed = readSketch(got.bytes, l);
    if (!parsed) return text(400, "Not a sketch");
    sketchPngs = parsed.pngs;
    kept = sketchPrompt(parsed);
    content = [...parsed.pngs.map(png => ({ type: "image", source: { type: "base64", media_type: "image/png", data: png.toBase64() } })), { type: "text", text: kept }];
  } else if (kind === "frames") {
    const parsed = pictures(got.bytes, l.THEME_ERRORS_BYTES);
    const moments = (request.headers.get("X-Caprice-Moments") ?? "").split(",").filter(m => /^[a-z ]{1,24}$/.test(m));
    const [phone, computer] = (request.headers.get("X-Caprice-Points") ?? "").split(",").map(Number);
    if (!parsed || moments.length !== parsed.pngs.length || !Number.isInteger(phone) || !Number.isInteger(computer)) return text(400, "Not pictures of a theme");
    found = parsed.found;
    frameData = { pngs: parsed.pngs, moments, points: { phone, computer } };
  } else {
    const said = words(got.bytes);
    if (!said) return text(400, "Nothing was said");
    found = said;
    kept = asked[kind](said);
    content = kept;
  }

  const ledger = env.LEDGER.get(env.LEDGER.idFromName("ledger"));
  // Begun simple (`light`) or `rich`, as the writer chose; later rounds are of the theme's tier.
  const tier = request.headers.get("X-Caprice-Tier") === "rich" ? "rich" : "light";
  // The site's owner tries themes out without a day's limits (the Ledger's `themeRound`).
  const round = await ledger.themeRound(user, id ?? null, kind, undefined, tier, isOwner(env, user));
  if (!round.ok) return refused(round);
  const hasSketch = Boolean(round.sketch?.length);
  if (frameData) {
    const said = asked.frames(frameData.moments, frameData.points, found, hasSketch);
    content = [...frameData.pngs.map(png => ({ type: "image", source: { type: "base64", media_type: "image/png", data: png.toBase64() } })), { type: "text", text: said }];
    kept = `[${frameData.pngs.length} pictures were shown here.] ${said}`;
  }
  // Each round for Workers Logs and `wrangler tail`: the theme, its kind, and the gist of what
  // Caprice found (the compiler's errors, the checks' first words), so that a theme's way there
  // can be followed. Never the writer's words.
  const gisted = gist(kind, found);
  const loggedKind = isSketch ? "sketch" : kind;
  console.log("theme: asked", JSON.stringify({ theme: round.id, kind: loggedKind, ...(gisted.length ? { found: gisted } : {}) }));
  const answer = (async () => {
    let answered = null, spent = null, done;
    try {
      const sketchImages = hasSketch
        ? round.sketch.map(png => ({ type: "image", source: { type: "base64", media_type: "image/png", data: png.toBase64() } }))
        : [];
      const messages = [
        ...round.turns.map((t, i) => ({
          role: t.role,
          content: i === 0 && sketchImages.length ? [...sketchImages, { type: "text", text: t.text }] : t.text,
        })),
        { role: "user", content },
      ];
      const rich = round.tier === "rich";
      const effort = rich ? env.THEME_EFFORT_RICH || "medium" : env.THEME_EFFORT || "medium";
      const written = await write(messages, env, rich ? l.THEME_TOKENS_RICH : l.THEME_TOKENS, rich ? l.THEME_BUDGET_RICH : l.THEME_BUDGET, effort);
      spent = written.spent ?? null;
      if (written.error) {
        done = written;
      } else {
        answered = { asked: kept, answer: written.reply, ...(sketchPngs ? { sketch: sketchPngs } : {}) };
        done = { theme: { theme: round.id, ...written.theme, left: round.left } };
      }
    } catch (e) {
      console.error("theme: the round failed", String(e).slice(0, 200));
      done = { error: 502, message: "Something went wrong while the theme was written: try again" };
    }
    console.log("theme: answered", JSON.stringify({ theme: round.id, kind: loggedKind, ...(done.error ? { error: done.error, message: done.message } : {}), spent }));
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
// round is paid for all the same. So it is also told its room (`task_budget`, `budget`), which it
// sees counting down and finishes within, and `most` stays the most a round can cost. A `budget` of
// 0 tells it none (a tight budget can cut an answer short).
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
  // What the round cost, kept in the Ledger with the theme (and logged with the round): whether the
  // cache was read or written again.
  const spent = cost(message.usage ?? {});
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
