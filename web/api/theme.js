// Making a theme program with Claude (ARCHITECTURE.md R36, THEME_PROGRAMS_PLAN.md step 5): the
// writer says what they want, Claude writes one Rust file against Caprice's theme SDK, the writer's
// browser compiles it and plays it off screen, and Claude is shown what came of it: the compiler's
// messages, or pictures of it at a few moments of a story with its points. A few rounds, then the
// writer keeps it, or asks for a change.
//
// It must never become a way to ask Claude anything on Caprice's key. So the Worker writes the whole
// prompt (the guide written for this, the SDK, the built-in themes ported as examples), the writer's
// words are one capped field in it, the conversation is kept here (in the Ledger) so the browser
// cannot write Claude's side of it, a round is one of three kinds (`errors`, `frames`, `change`),
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

const WRITER = "claude-opus-5-5";
// A theme's id, given by the Ledger when one is begun.
export const THEME_ID = /^[0-9a-f]{16}$/;
// The palette's names (src/own.rs, NAMES), and "light".
const COLOURS = ["desk_top", "desk", "dock", "dock_edge", "button", "button_hover", "field", "text", "text_dim", "text_hot", "accent", "on_accent", "caret", "thumb"];

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
async function body(request, most) {
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
  change: (said) => `The writer asks for a change:\n\n<change>\n${said}\n</change>\n\nMake it, and give the palette and the whole file again.`,
  frames: (moments, points) => `It built. Here it is as Caprice drew it, at these moments of a story: ${moments.join(", ")}. `
    + `At most ${points.phone} points a frame on a phone's desk, ${points.computer} on a computer's (zoomed out included). `
    + "Look at it as the writer will, beside their page for hours, and make it better: what is wrong, unclear, ugly or too heavy, "
    + "and what would make it lovelier. Give the palette and the whole file again.",
};

// Making a theme: `id` none to begin one (the writer's request), else a round of `kind`.
export async function themeProgram(request, env, user, id) {
  if (env.THEMES_OFF === "1") return text(503, "Making themes is switched off for now", { "Retry-After": "3600" });
  if (!env.ANTHROPIC_API_KEY) return text(503, "Making themes is not set up", { "Retry-After": "3600" });
  const l = limits(env);
  const kind = id ? request.headers.get("X-Caprice-Kind") : "begin";
  const most = { begin: l.THEME_REQUEST_BYTES, change: l.THEME_REQUEST_BYTES, errors: l.THEME_ERRORS_BYTES, frames: l.THEME_FRAMES_BYTES }[kind];
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
  const round = await ledger.themeRound(user, id ?? null, kind);
  if (!round.ok) return refused(round);
  let answered = null;
  try {
    const messages = [...round.turns.map(t => ({ role: t.role, content: t.text })), { role: "user", content }];
    const written = await write(messages, env, l.THEME_TOKENS);
    if (written.error) return text(written.error, written.message, written.retry ? { "Retry-After": String(written.retry) } : {});
    answered = { asked: kept, answer: written.reply };
    return Response.json({ theme: round.id, ...written.theme, left: round.left });
  } finally {
    await ledger.themeAnswered(user, round.id, answered);
  }
}

// Claude's answer in one piece, not streamed, as a scene is (worker.js, `paint`): waiting costs the
// Worker none of its 10 ms. Thinking is adaptive; the prompt is cached, and the conversation too.
async function write(messages, env, most) {
  const sent = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: {
      "x-api-key": env.ANTHROPIC_API_KEY, "anthropic-version": "2023-06-01", "content-type": "application/json",
      // Should the model decline, the request goes on with another the platform chooses.
      "anthropic-beta": "server-side-fallback-2026-07-01",
    },
    body: JSON.stringify({
      model: WRITER, max_tokens: most,
      system: [{ type: "text", text: SYSTEM, cache_control: { type: "ephemeral" } }],
      output_config: { effort: env.THEME_EFFORT || "high" },
      cache_control: { type: "ephemeral" },
      fallbacks: "default",
      messages,
    }),
  });
  if (!sent.ok) {
    const said = await sent.text();
    console.error("theme: Claude answered", sent.status, said.slice(0, 200));
    if (sent.status === 402 || /credit balance/i.test(said)) return { error: 503, message: "Making themes is paused for now", retry: 3600 };
    return [429, 529].includes(sent.status)
      ? { error: 503, message: "Claude is busy: try again in a minute" }
      : { error: 502, message: "Claude could not be asked" };
  }
  const message = await sent.json();
  if (message.stop_reason === "refusal") return { error: 422, message: "Claude declined to write this theme" };
  const reply = (message.content ?? []).filter(b => b.type === "text").map(b => b.text).join("");
  const theme = read(reply);
  if (!theme) return { error: 502, message: message.stop_reason === "max_tokens" ? "The theme grew too long to finish" : "Claude's reply held no theme" };
  return { theme, reply };
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
