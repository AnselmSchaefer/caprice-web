// The Ledger: one Durable Object that knows who has signed up, which version of each story is
// current, the history kept online, and every count the limits need. It is one object for the
// whole site on purpose: its calls run one at a time, so two browsers saving at once cannot both
// win, and no limit can be passed by asking twice at the same moment. R2 holds only the files;
// the Worker (worker.js) asks here before it touches R2, so R2 never does what the limits forbid.
//
// Every answer is { ok: true, ... } or { error: <HTTP status>, message, retry? }.
import { DurableObject } from "cloudflare:workers";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

// What the limits are when wrangler.jsonc does not say. They keep R2 inside its free amounts
// (10 GB stored, a million writes and ten million reads a month): see CLOUD_PLAN.md.
const DEFAULTS = {
  SITE_WRITES_PER_DAY: 20_000,
  SITE_READS_PER_DAY: 250_000,
  SITE_BYTES: 8e9,
  SITE_ACCOUNTS_PER_DAY: 200,
  USER_WRITES_PER_MINUTE: 10,
  USER_WRITES_PER_DAY: 500,
  USER_BYTES: 100e6,
  FILE_BYTES: 25e6,
  HISTORY: 3,
  HISTORY_APART_HOURS: 6,
  // Claude bills every scene it paints: this is the most it paints in a day, for everyone together.
  SITE_SCENES_PER_DAY: 50,
  // And the most one scene can cost: what is sent (about 90 KB of it the cast's model sheets),
  // and what Claude writes, its thinking counted.
  SCENE_PROMPT_BYTES: 160_000,
  SCENE_TOKENS: 32_000,
  // Making a theme program (theme.js): Claude bills every round, each far dearer than a scene. The
  // rounds a day for everyone together, the themes a writer may begin in a day and the rounds they
  // may ask for; in one theme, the rounds of each kind; what each may hold, and what Claude may
  // write in one, its thinking counted. A conversation is kept this many days, then forgotten.
  SITE_THEME_ROUNDS_PER_DAY: 12,
  USER_THEMES_PER_DAY: 2,
  USER_THEME_ROUNDS_PER_DAY: 12,
  THEME_ERRORS_ROUNDS: 3,
  THEME_FRAMES_ROUNDS: 2,
  THEME_CHANGE_ROUNDS: 3,
  THEME_REQUEST_BYTES: 2_000,
  THEME_ERRORS_BYTES: 20_000,
  THEME_FRAMES_BYTES: 4_000_000,
  THEME_TOKENS: 64_000,
  THEME_DAYS: 7,
  // The gallery of theme programs (gallery.js): what one theme file may hold, the themes one writer
  // may send in a day, how many may wait for the owner at once, the room all of them may take in
  // R2, and how many the gallery lists at a time.
  GALLERY_FILE_BYTES: 3_000_000,
  USER_PUBLISHED_PER_DAY: 3,
  GALLERY_WAITING: 30,
  GALLERY_BYTES: 1e9,
  GALLERY_LIST: 60,
  // How many days the scenes painted, by hashed id, are kept for /api/admin/scenes.
  SCENE_DAYS: 30,
  // A picture painted is kept until the app has it (a reload, another story opened meanwhile):
  // this many hours at most, and this many for each writer.
  PAINTED_HOURS: 48,
  PAINTED_PER_USER: 20,
  // A story deleted is listed as gone for this many days, so the writer's other devices delete
  // their copy too rather than put it back.
  GONE_DAYS: 180,
  // A story live on several devices (room.js): connections a day for the site, messages a day
  // for one story, the most one message holds, how long a device holding the pen may be gone
  // before another has it, and how long the holder has to hand it over.
  SITE_LIVE_PER_DAY: 5_000,
  ROOM_MESSAGES_PER_DAY: 20_000,
  // And the messages a day for every room together, which rooms take from here so many at a time:
  // live editing's share of the 100,000, so that it cannot use up what stories online need.
  SITE_LIVE_MESSAGES_PER_DAY: 40_000,
  LIVE_MESSAGES_AT_ONCE: 200,
  LIVE_MESSAGE_BYTES: 256_000,
  LIVE_FREE_AFTER_MS: 30_000,
  LIVE_HAND_OVER_MS: 3_000,
};

export function limits(env) {
  const found = {};
  for (const [name, value] of Object.entries(DEFAULTS)) {
    const set = Number(env[name]);
    found[name] = env[name] !== undefined && env[name] !== "" && Number.isFinite(set) ? set : value;
  }
  return found;
}

// Where a version of a story is in R2. Only ever built from a signed-in user and a checked id.
export function key(user, id, version) {
  return `users/${user}/${id}/${version}.caprice`;
}

const day = now => new Date(now).toISOString().slice(0, 10);

// Seconds until the next day begins (UTC), when the daily counts start again.
function untilTomorrow(now) {
  const next = new Date(now);
  next.setUTCHours(24, 0, 0, 0);
  return Math.ceil((next - now) / 1000);
}

export class Ledger extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.sql = ctx.storage.sql;
    this.limits = limits(env);
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, made INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS stories (user TEXT, id TEXT, name TEXT, version TEXT, modified INTEGER, size INTEGER,
        PRIMARY KEY (user, id));
      CREATE TABLE IF NOT EXISTS versions (user TEXT, id TEXT, version TEXT, size INTEGER, at INTEGER,
        PRIMARY KEY (user, id, version));
      CREATE TABLE IF NOT EXISTS pending (user TEXT, id TEXT, version TEXT, size INTEGER, until INTEGER,
        PRIMARY KEY (user, id));
      CREATE TABLE IF NOT EXISTS counts (key TEXT PRIMARY KEY, n INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS recent (user TEXT, at INTEGER);
      CREATE TABLE IF NOT EXISTS painting (user TEXT PRIMARY KEY, until INTEGER);
      CREATE TABLE IF NOT EXISTS scene_days (day TEXT, user TEXT, n INTEGER NOT NULL, PRIMARY KEY (day, user));
      CREATE TABLE IF NOT EXISTS gone (user TEXT, id TEXT, version TEXT, at INTEGER, PRIMARY KEY (user, id));
      CREATE TABLE IF NOT EXISTS painted (user TEXT, id TEXT, story TEXT, at INTEGER, svg TEXT, made INTEGER,
        PRIMARY KEY (user, id));
      CREATE TABLE IF NOT EXISTS theme_talks (id TEXT PRIMARY KEY, user TEXT, made INTEGER, errors INTEGER, frames INTEGER,
        changes INTEGER, busy INTEGER);
      CREATE TABLE IF NOT EXISTS theme_turns (id TEXT, n INTEGER, role TEXT, text TEXT, PRIMARY KEY (id, n));
      CREATE TABLE IF NOT EXISTS theme_costs (id TEXT PRIMARY KEY, user TEXT, begun INTEGER, rounds INTEGER, input INTEGER,
        written INTEGER, read INTEGER, output INTEGER, micro INTEGER, kept INTEGER);
      CREATE TABLE IF NOT EXISTS gallery (id TEXT PRIMARY KEY, user TEXT, name TEXT, about TEXT, author TEXT, colours TEXT,
        module TEXT, source TEXT, toolchain TEXT, size INTEGER, at INTEGER, state TEXT, why TEXT, signature TEXT, decided INTEGER);
    `);
  }

  rows(query, ...args) {
    return this.sql.exec(query, ...args).toArray();
  }

  // The day's counts are named after the day, so yesterday's can go as soon as today's begin.
  count(now, what) {
    return this.rows("SELECT n FROM counts WHERE key = ?", `${day(now)}|${what}`)[0]?.n ?? 0;
  }

  bump(now, what) {
    this.sql.exec("DELETE FROM counts WHERE key < ?", day(now));
    this.sql.exec("INSERT INTO counts VALUES (?, 1) ON CONFLICT (key) DO UPDATE SET n = n + 1", `${day(now)}|${what}`);
  }

  known(user) {
    return this.rows("SELECT 1 FROM users WHERE id = ?", user).length > 0;
  }

  // A sign-in: someone already here is let in; someone new only while today's new accounts last.
  signIn(user, now = Date.now()) {
    if (this.known(user)) return { ok: true };
    if (this.count(now, "accounts") >= this.limits.SITE_ACCOUNTS_PER_DAY) {
      return { error: 503, message: "Caprice is taking no new sign-ups today. Your stories are safe in this browser.", retry: untilTomorrow(now) };
    }
    this.bump(now, "accounts");
    this.sql.exec("INSERT INTO users VALUES (?, ?)", user, now);
    return { ok: true };
  }

  // The writer's stories, and the ones deleted lately with the version each was when it went.
  list(user, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const stories = this.rows("SELECT id, name, version, modified, size FROM stories WHERE user = ? ORDER BY modified DESC", user);
    this.sql.exec("DELETE FROM gone WHERE at <= ?", now - this.limits.GONE_DAYS * DAY);
    const gone = this.rows("SELECT id, version FROM gone WHERE user = ?", user);
    return { ok: true, stories, gone };
  }

  // Which version of a story to read from R2, counted against the day's reads.
  current(user, id, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const story = this.rows("SELECT name, version FROM stories WHERE user = ? AND id = ?", user, id)[0];
    if (!story) return { error: 404, message: "No such story online" };
    if (this.count(now, "reads") >= this.limits.SITE_READS_PER_DAY) {
      return { error: 503, message: "Stories online are paused until tomorrow.", retry: untilTomorrow(now) };
    }
    this.bump(now, "reads");
    return { ok: true, ...story };
  }

  // Before an upload: every limit, and the version it replaces. When it may go ahead, it is given
  // a new version to write, and the history copies to delete from R2 to make room for it.
  begin(user, id, size, { ifMatch, ifNoneMatch }, now = Date.now()) {
    const l = this.limits;
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    this.sql.exec("DELETE FROM recent WHERE at <= ?", now - MINUTE);
    this.sql.exec("DELETE FROM pending WHERE until <= ?", now);
    if (this.rows("SELECT 1 FROM recent WHERE user = ?", user).length >= l.USER_WRITES_PER_MINUTE) {
      return { error: 429, message: "Too many saves in a minute", retry: 60 };
    }
    if (this.count(now, `writes|${user}`) >= l.USER_WRITES_PER_DAY) {
      return { error: 429, message: "Too many saves today", retry: untilTomorrow(now) };
    }
    if (this.count(now, "writes") >= l.SITE_WRITES_PER_DAY) {
      return { error: 503, message: "Stories online are paused until tomorrow.", retry: untilTomorrow(now) };
    }
    if (this.rows("SELECT 1 FROM pending WHERE user = ? AND id = ?", user, id).length) {
      return { error: 409, message: "This story is being saved already", retry: 5 };
    }
    const story = this.rows("SELECT version FROM stories WHERE user = ? AND id = ?", user, id)[0];
    if (ifNoneMatch === "*") {
      if (story) return { error: 412, message: "There is a copy of this story online already" };
    } else if (ifMatch !== undefined) {
      if (!story || story.version !== ifMatch) return { error: 412, message: "The copy online has changed" };
    } else {
      return { error: 428, message: "Say which copy this replaces" };
    }

    // Room, the history counted: this user's oldest history goes first, but only if that is enough.
    const used = who => this.rows(`SELECT (SELECT COALESCE(SUM(size), 0) FROM versions ${who})
      + (SELECT COALESCE(SUM(size), 0) FROM pending ${who}) AS n`, ...(who ? [user, user] : []))[0].n;
    const history = this.rows(`SELECT v.id, v.version, v.size FROM versions v JOIN stories s ON s.user = v.user AND s.id = v.id
      WHERE v.user = ? AND v.version != s.version ORDER BY v.at`, user);
    const mine = used("WHERE user = ?");
    const all = used("");
    const drop = [];
    let freed = 0;
    for (const old of history) {
      if (mine - freed + size <= l.USER_BYTES) break;
      drop.push(old);
      freed += old.size;
    }
    if (mine - freed + size > l.USER_BYTES) return { error: 507, message: "No more room for your stories online" };
    if (all - freed + size > l.SITE_BYTES) return { error: 507, message: "Caprice has no more room for stories online" };

    for (const old of drop) this.sql.exec("DELETE FROM versions WHERE user = ? AND id = ? AND version = ?", user, old.id, old.version);
    this.bump(now, "writes");
    this.bump(now, `writes|${user}`);
    this.sql.exec("INSERT INTO recent VALUES (?, ?)", user, now);
    const version = now.toString(36) + crypto.getRandomValues(new Uint32Array(1))[0].toString(36);
    this.sql.exec("INSERT INTO pending VALUES (?, ?, ?, ?, ?)", user, id, version, size, now + 2 * MINUTE);
    return { ok: true, version, drop: drop.map(old => key(user, old.id, old.version)) };
  }

  // The upload is in R2: it becomes the current version, and the history is thinned out. What is
  // kept: the version before, and older ones at least HISTORY_APART_HOURS apart, HISTORY in all.
  commit(user, id, version, name, now = Date.now()) {
    const l = this.limits;
    const pending = this.rows("SELECT size FROM pending WHERE user = ? AND id = ? AND version = ?", user, id, version)[0];
    if (!pending) return { error: 409, message: "The save took too long; try again" };
    this.sql.exec("DELETE FROM pending WHERE user = ? AND id = ?", user, id);
    this.sql.exec("INSERT INTO versions VALUES (?, ?, ?, ?, ?)", user, id, version, pending.size, now);
    this.sql.exec(`INSERT INTO stories VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT (user, id)
      DO UPDATE SET name = excluded.name, version = excluded.version, modified = excluded.modified, size = excluded.size`,
      user, id, name, version, now, pending.size);
    // Saved again after it was deleted (written in somewhere meanwhile): it is no longer gone.
    this.sql.exec("DELETE FROM gone WHERE user = ? AND id = ?", user, id);
    const [, before, ...older] = this.rows("SELECT version, at FROM versions WHERE user = ? AND id = ? ORDER BY at DESC, version DESC", user, id);
    const kept = before ? [before] : [];
    const drop = [];
    for (const old of older) {
      if (kept.length < l.HISTORY && kept.at(-1).at - old.at >= l.HISTORY_APART_HOURS * HOUR) kept.push(old);
      else drop.push(old);
    }
    for (const old of drop) this.sql.exec("DELETE FROM versions WHERE user = ? AND id = ? AND version = ?", user, id, old.version);
    return { ok: true, drop: drop.map(old => key(user, id, old.version)) };
  }

  // The upload failed: its room is given back (the write still counts: R2 may have been reached).
  abort(user, id, version) {
    this.sql.exec("DELETE FROM pending WHERE user = ? AND id = ? AND version = ?", user, id, version);
  }

  // Before Claude is asked for a scene: one of the day's, while they last, and one at a time for
  // each writer (a second click would cost another). It counts whether or not a picture comes of
  // it, since Claude bills the asking: so the day's limit is a limit on what is billed.
  paint(user, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    this.sql.exec("DELETE FROM painting WHERE until <= ?", now);
    if (this.rows("SELECT 1 FROM painting WHERE user = ?", user).length) {
      return { error: 409, message: "A scene is being painted for you already", retry: 30 };
    }
    const most = this.limits.SITE_SCENES_PER_DAY;
    const done = this.count(now, "scenes");
    if (done >= most) {
      return { error: 503, message: `No more scenes today: Caprice paints ${most} a day for everyone. More tomorrow.`, retry: untilTomorrow(now) };
    }
    this.bump(now, "scenes");
    // Who painted how many, by the hashed id only (no name is kept anywhere), for SCENE_DAYS: so
    // one account using up the day's scenes can be seen, without knowing who it is.
    this.sql.exec("DELETE FROM scene_days WHERE day < ?", day(now - this.limits.SCENE_DAYS * DAY));
    this.sql.exec("INSERT INTO scene_days VALUES (?, ?, 1) ON CONFLICT (day, user) DO UPDATE SET n = n + 1", day(now), user);
    this.sql.exec("INSERT INTO painting VALUES (?, ?)", user, now + 10 * MINUTE);
    return { ok: true, left: most - done - 1 };
  }

  // A round of making a theme (theme.js), counted before Claude is asked: whether it may be, the
  // conversation so far, and what is left of each kind. `id` none begins a theme. One round of a
  // theme at a time. It counts whether or not a theme comes of it, as Claude bills the asking.
  themeRound(user, id, kind, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const l = this.limits;
    const old = this.rows("SELECT id FROM theme_talks WHERE made <= ?", now - l.THEME_DAYS * DAY);
    for (const o of old) this.sql.exec("DELETE FROM theme_turns WHERE id = ?", o.id);
    this.sql.exec("DELETE FROM theme_talks WHERE made <= ?", now - l.THEME_DAYS * DAY);
    const most = { errors: l.THEME_ERRORS_ROUNDS, frames: l.THEME_FRAMES_ROUNDS, change: l.THEME_CHANGE_ROUNDS };
    const column = { errors: "errors", frames: "frames", change: "changes" };
    let talk = null;
    if (id !== null) {
      talk = this.rows("SELECT * FROM theme_talks WHERE id = ? AND user = ?", id, user)[0];
      if (!talk) return { error: 404, message: "That theme is no longer being made: begin it again" };
      if (talk.busy > now) return { error: 409, message: "This theme is being worked on already", retry: 30 };
      if (talk[column[kind]] >= most[kind]) return { error: 429, message: `No more rounds of this kind for this theme (${most[kind]})` };
    } else if (this.count(now, `themes:${user}`) >= l.USER_THEMES_PER_DAY) {
      return { error: 429, message: `You can begin ${l.USER_THEMES_PER_DAY} themes a day. More tomorrow.`, retry: untilTomorrow(now) };
    }
    if (this.count(now, "theme_rounds") >= l.SITE_THEME_ROUNDS_PER_DAY) {
      return { error: 503, message: "Caprice makes no more themes today, for anyone. More tomorrow.", retry: untilTomorrow(now) };
    }
    if (this.count(now, `theme_rounds:${user}`) >= l.USER_THEME_ROUNDS_PER_DAY) {
      return { error: 429, message: "You have made as much as a day allows. More tomorrow.", retry: untilTomorrow(now) };
    }
    this.bump(now, "theme_rounds");
    this.bump(now, `theme_rounds:${user}`);
    if (talk === null) {
      this.bump(now, `themes:${user}`);
      id = [...crypto.getRandomValues(new Uint8Array(8))].map(b => b.toString(16).padStart(2, "0")).join("");
      this.sql.exec("INSERT INTO theme_talks VALUES (?, ?, ?, 0, 0, 0, 0)", id, user, now);
      this.sql.exec("INSERT INTO theme_costs VALUES (?, ?, ?, 0, 0, 0, 0, 0, 0, 0)", id, user, now);
    } else {
      this.sql.exec(`UPDATE theme_talks SET ${column[kind]} = ${column[kind]} + 1 WHERE id = ?`, id);
    }
    this.sql.exec("UPDATE theme_talks SET busy = ? WHERE id = ?", now + 15 * MINUTE, id);
    const talked = this.rows("SELECT * FROM theme_talks WHERE id = ?", id)[0];
    const left = { errors: most.errors - talked.errors, frames: most.frames - talked.frames, change: most.change - talked.changes };
    return { ok: true, id, turns: this.rows("SELECT role, text FROM theme_turns WHERE id = ? ORDER BY n", id), left };
  }

  // A round done: what was asked and what Claude answered kept, whole, if it answered; the theme
  // free for its next round either way.
  // What the round cost (`spent`, its tokens and millionths of a dollar) is added to the theme's
  // costs whether or not a theme came of it: Claude was paid either way.
  themeAnswered(user, id, answered, spent = null) {
    const talk = this.rows("SELECT 1 FROM theme_talks WHERE id = ? AND user = ?", id, user)[0];
    if (!talk) return { ok: true };
    if (spent) {
      this.sql.exec(`UPDATE theme_costs SET rounds = rounds + 1, input = input + ?, written = written + ?, read = read + ?,
        output = output + ?, micro = micro + ? WHERE id = ?`, spent.input, spent.written, spent.read, spent.output, spent.micro, id);
    }
    if (answered) {
      const n = this.rows("SELECT COUNT(*) AS n FROM theme_turns WHERE id = ?", id)[0].n;
      this.sql.exec("INSERT INTO theme_turns VALUES (?, ?, 'user', ?)", id, n, answered.asked);
      this.sql.exec("INSERT INTO theme_turns VALUES (?, ?, 'assistant', ?)", id, n + 1, answered.answer);
    }
    this.sql.exec("UPDATE theme_talks SET busy = 0 WHERE id = ?", id);
    return { ok: true };
  }

  // The writer kept the theme (put it on their shelf): what billing will hang on, and what the
  // free time is measured by. Once; only by the writer who began it.
  themeKept(user, id, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const theme = this.rows("SELECT kept FROM theme_costs WHERE id = ? AND user = ?", id, user)[0];
    if (!theme) return { error: 404, message: "No such theme" };
    if (!theme.kept) this.sql.exec("UPDATE theme_costs SET kept = ? WHERE id = ?", now, id);
    return { ok: true };
  }

  // What themes cost, a day at a time (by the day they were begun), for the owner: begun, kept,
  // Claude's rounds, tokens and dollars, all of them and those of the themes kept.
  themeCosts(now = Date.now(), days = 30) {
    const since = now - days * DAY;
    const rows = this.rows(`SELECT date(begun / 1000, 'unixepoch') AS day, COUNT(*) AS begun, SUM(kept > 0) AS kept,
      SUM(rounds) AS rounds, SUM(input) AS input, SUM(written) AS written, SUM(read) AS read, SUM(output) AS output,
      SUM(micro) AS micro, SUM(CASE WHEN kept > 0 THEN micro ELSE 0 END) AS kept_micro
      FROM theme_costs WHERE begun > ? GROUP BY day ORDER BY day`, since);
    const dollars = m => Math.round(m / 10_000) / 100;
    const out = r => ({ day: r.day, begun: r.begun, kept: r.kept, rounds: r.rounds,
      tokens: { input: r.input, written: r.written, read: r.read, output: r.output },
      dollars: dollars(r.micro), keptDollars: dollars(r.kept_micro) });
    const sum = rows.reduce((t, r) => {
      for (const k of ["begun", "kept", "rounds", "input", "written", "read", "output", "micro", "kept_micro"]) t[k] += r[k];
      return t;
    }, { day: `last ${days} days`, begun: 0, kept: 0, rounds: 0, input: 0, written: 0, read: 0, output: 0, micro: 0, kept_micro: 0 });
    return { days: rows.map(out), total: { ...out(sum), costPerKept: sum.kept ? dollars(sum.micro / sum.kept) : null } };
  }

  // A theme sent to the gallery (gallery.js), checked by the Worker: allowed while the writer's day,
  // the queue, the site's writes and the gallery's room allow, and only once for one module. It is
  // `sending` until both its files are in R2 (`galleryCommit`), then waits for the owner.
  galleryPublish(user, t, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const l = this.limits;
    // One that never reached R2 gives its place back.
    this.sql.exec("DELETE FROM gallery WHERE state = 'sending' AND at <= ?", now - 10 * MINUTE);
    const same = this.rows("SELECT state FROM gallery WHERE module = ? AND state != 'refused'", t.module)[0];
    if (same) return { error: 409, message: same.state === "public" ? "This theme is in the gallery already" : "This theme is waiting for the gallery already" };
    if (this.count(now, `published|${user}`) >= l.USER_PUBLISHED_PER_DAY) {
      return { error: 429, message: `You can send ${l.USER_PUBLISHED_PER_DAY} themes to the gallery a day. More tomorrow.`, retry: untilTomorrow(now) };
    }
    if (this.rows("SELECT COUNT(*) AS n FROM gallery WHERE state IN ('sending', 'waiting')")[0].n >= l.GALLERY_WAITING) {
      return { error: 503, message: "The gallery has as many themes waiting as it can look at: try again in a few days.", retry: 86400 };
    }
    if (this.count(now, "writes") + 2 > l.SITE_WRITES_PER_DAY) {
      return { error: 503, message: "The gallery takes no more themes today. More tomorrow.", retry: untilTomorrow(now) };
    }
    const used = this.rows("SELECT COALESCE(SUM(size), 0) AS n FROM gallery WHERE state != 'refused'")[0].n;
    if (used + t.size > l.GALLERY_BYTES) return { error: 507, message: "The gallery has no more room for now" };
    this.bump(now, `published|${user}`);
    this.bump(now, "writes");
    this.bump(now, "writes");
    const id = [...crypto.getRandomValues(new Uint8Array(8))].map(b => b.toString(16).padStart(2, "0")).join("");
    this.sql.exec("INSERT INTO gallery VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'sending', '', '', 0)",
      id, user, t.name, t.about, t.author, JSON.stringify(t.colours), t.module, t.source, t.toolchain, t.size, now);
    return { ok: true, id };
  }

  // Its files are in R2: it waits for the owner. Or they could not be put there: it is forgotten.
  galleryCommit(id, ok) {
    if (ok) this.sql.exec("UPDATE gallery SET state = 'waiting' WHERE id = ? AND state = 'sending'", id);
    else this.sql.exec("DELETE FROM gallery WHERE id = ? AND state = 'sending'", id);
    return { ok: true };
  }

  // The themes in the gallery, newest first, as everyone may see them: never who sent them.
  galleryList() {
    const rows = this.rows(`SELECT id, name, about, author, colours, source, toolchain, signature, decided FROM gallery
      WHERE state = 'public' ORDER BY decided DESC, id LIMIT ?`, this.limits.GALLERY_LIST);
    return { ok: true, themes: rows.map(r => ({ ...r, colours: JSON.parse(r.colours) })) };
  }

  // What one writer has sent, and what became of it.
  galleryMine(user) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const rows = this.rows("SELECT id, name, module, state, why, at FROM gallery WHERE user = ? AND state != 'sending' ORDER BY at DESC", user);
    return { ok: true, themes: rows };
  }

  // The themes waiting for the owner, oldest first, with what the Worker found of each.
  galleryWaiting() {
    const rows = this.rows("SELECT id, name, about, author, colours, module, source, toolchain, size, at FROM gallery WHERE state = 'waiting' ORDER BY at, id");
    return { ok: true, themes: rows.map(r => ({ ...r, colours: JSON.parse(r.colours) })) };
  }

  // One of a theme's files to read from R2, counted against the day's reads: a theme in the gallery
  // for anyone signed in, one waiting for the owner only.
  galleryRead(user, id, owner, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const t = this.rows("SELECT state FROM gallery WHERE id = ?", id)[0];
    if (!t || !(t.state === "public" || (owner && t.state === "waiting"))) return { error: 404, message: "No such theme in the gallery" };
    if (this.count(now, "reads") >= this.limits.SITE_READS_PER_DAY) {
      return { error: 503, message: "The gallery is resting until tomorrow.", retry: untilTomorrow(now) };
    }
    this.bump(now, "reads");
    return { ok: true, state: t.state };
  }

  // A theme waiting, as the Worker signs it.
  galleryEntry(id) {
    const t = this.rows("SELECT * FROM gallery WHERE id = ? AND state = 'waiting'", id)[0];
    return t ? { ok: true, theme: t } : { error: 404, message: "No such theme waiting" };
  }

  // The owner's decision: in the gallery with the Worker's signature, or refused with a word why
  // (its files to delete). Only a theme still waiting is decided.
  galleryDecide(id, signature, why, now = Date.now()) {
    if (!this.rows("SELECT 1 FROM gallery WHERE id = ? AND state = 'waiting'", id).length) return { error: 409, message: "That theme is not waiting any more" };
    if (signature) this.sql.exec("UPDATE gallery SET state = 'public', signature = ?, decided = ? WHERE id = ?", signature, now, id);
    else this.sql.exec("UPDATE gallery SET state = 'refused', why = ?, decided = ? WHERE id = ?", why, now, id);
    return { ok: true };
  }

  // The scenes of the days kept, newest first: each day's total, how many writers, and each
  // writer's count by hashed id, most first.
  sceneDays(now = Date.now()) {
    this.sql.exec("DELETE FROM scene_days WHERE day < ?", day(now - this.limits.SCENE_DAYS * DAY));
    const rows = this.rows("SELECT day, user, n FROM scene_days ORDER BY day DESC, n DESC, user");
    const days = [];
    for (const r of rows) {
      if (days.at(-1)?.day !== r.day) days.push({ day: r.day, scenes: 0, writers: 0, by: [] });
      const d = days.at(-1);
      d.scenes += r.n;
      d.writers += 1;
      d.by.push({ user: r.user, scenes: r.n });
    }
    return { ok: true, limit: this.limits.SITE_SCENES_PER_DAY, days };
  }

  // A picture Claude finished, kept until the app that asked for it has it: paid for, it must
  // not be lost to a reload or another story opened meanwhile. Only where it goes is kept (the
  // story's id, the char its scene is pinned to), nothing of the story's text. Oldest go first.
  keepPainted(user, id, story, at, svg, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    this.sql.exec("DELETE FROM painted WHERE made <= ?", now - this.limits.PAINTED_HOURS * HOUR);
    this.sql.exec("INSERT OR REPLACE INTO painted VALUES (?, ?, ?, ?, ?, ?)", user, id, story, at, svg, now);
    const over = this.rows("SELECT id FROM painted WHERE user = ? ORDER BY made DESC LIMIT -1 OFFSET ?", user, this.limits.PAINTED_PER_USER);
    for (const o of over) this.sql.exec("DELETE FROM painted WHERE user = ? AND id = ?", user, o.id);
    return { ok: true };
  }

  // The pictures waiting for a story of this writer's.
  paintedFor(user, story, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    this.sql.exec("DELETE FROM painted WHERE made <= ?", now - this.limits.PAINTED_HOURS * HOUR);
    return { ok: true, painted: this.rows("SELECT id, at, svg FROM painted WHERE user = ? AND story = ? ORDER BY made", user, story) };
  }

  // The app has the picture: it is in the story, and saved.
  forgetPainted(user, id) {
    this.sql.exec("DELETE FROM painted WHERE user = ? AND id = ?", user, id);
    return { ok: true };
  }

  // A device opening a story live (room.js): counted against the site's connections for the day.
  goLive(user, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    if (this.count(now, "live") >= this.limits.SITE_LIVE_PER_DAY) {
      return { error: 503, message: "Live editing is paused until tomorrow", retry: untilTomorrow(now) };
    }
    this.bump(now, "live");
    return { ok: true };
  }

  // A room's next messages for today, out of live editing's share for the whole site. Taken so
  // many at a time, so that a room asks here once for every LIVE_MESSAGES_AT_ONCE it passes on.
  liveMessages(now = Date.now()) {
    const l = this.limits;
    const left = l.SITE_LIVE_MESSAGES_PER_DAY - this.count(now, "live messages");
    if (left <= 0) return { error: 503, message: "Live editing is paused until tomorrow", retry: untilTomorrow(now) };
    const n = Math.min(left, l.LIVE_MESSAGES_AT_ONCE);
    this.sql.exec("DELETE FROM counts WHERE key < ?", day(now));
    this.sql.exec("INSERT INTO counts VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET n = n + excluded.n", `${day(now)}|live messages`, n);
    return { ok: true, n };
  }

  // The painting is over, however it went: the writer may ask for the next.
  painted(user) {
    this.sql.exec("DELETE FROM painting WHERE user = ?", user);
  }

  // One story deleted: its copies go from R2, and it is listed as gone (`list`), so that a device
  // whose copy is the version deleted deletes it too.
  remove(user, id, ifMatch, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const story = this.rows("SELECT version FROM stories WHERE user = ? AND id = ?", user, id)[0];
    if (!story) return { error: 404, message: "No such story online" };
    if (story.version !== ifMatch) return { error: 412, message: "The copy online has changed" };
    const gone = this.rows("SELECT version FROM versions WHERE user = ? AND id = ?", user, id);
    this.sql.exec("DELETE FROM versions WHERE user = ? AND id = ?", user, id);
    this.sql.exec("DELETE FROM stories WHERE user = ? AND id = ?", user, id);
    this.sql.exec("INSERT OR REPLACE INTO gone VALUES (?, ?, ?, ?)", user, id, story.version, now);
    return { ok: true, drop: gone.map(v => key(user, id, v.version)) };
  }

  removeAll(user) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const gone = this.rows("SELECT id, version FROM versions WHERE user = ?", user);
    // Who painted how many goes too; the day's total for the site stays, so it is no way round the limit.
    for (const table of ["versions", "stories", "pending", "gone", "scene_days", "painted"]) this.sql.exec(`DELETE FROM ${table} WHERE user = ?`, user);
    // What their themes cost stays in the site's numbers, but no longer says whose they were.
    this.sql.exec("UPDATE theme_costs SET user = '' WHERE user = ?", user);
    // Themes sent to the gallery and not in it go too; those in it stay, as copies on shelves do.
    const unpublished = this.rows("SELECT id FROM gallery WHERE user = ? AND state != 'public'", user);
    this.sql.exec("DELETE FROM gallery WHERE user = ? AND state != 'public'", user);
    const files = unpublished.flatMap(t => [`gallery/${t.id}/theme.caprice-theme`, `gallery/${t.id}/preview.png`]);
    return { ok: true, drop: [...gone.map(v => key(user, v.id, v.version)), ...files] };
  }
}
