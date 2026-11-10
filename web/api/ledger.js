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
  // How many days the scenes painted, by hashed id, are kept for /api/admin/scenes.
  SCENE_DAYS: 30,
  // A story live on several devices (room.js): connections a day for the site, messages a day
  // for one story, the most one message holds, how long a device holding the pen may be gone
  // before another has it, and how long the holder has to hand it over.
  SITE_LIVE_PER_DAY: 5_000,
  ROOM_MESSAGES_PER_DAY: 20_000,
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

  list(user) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const stories = this.rows("SELECT id, name, version, modified, size FROM stories WHERE user = ? ORDER BY modified DESC", user);
    return { ok: true, stories };
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

  // A device opening a story live (room.js): counted against the site's connections for the day.
  goLive(user, now = Date.now()) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    if (this.count(now, "live") >= this.limits.SITE_LIVE_PER_DAY) {
      return { error: 503, message: "Live editing is paused until tomorrow", retry: untilTomorrow(now) };
    }
    this.bump(now, "live");
    return { ok: true };
  }

  // The painting is over, however it went: the writer may ask for the next.
  painted(user) {
    this.sql.exec("DELETE FROM painting WHERE user = ?", user);
  }

  remove(user, id, ifMatch) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const story = this.rows("SELECT version FROM stories WHERE user = ? AND id = ?", user, id)[0];
    if (!story) return { error: 404, message: "No such story online" };
    if (story.version !== ifMatch) return { error: 412, message: "The copy online has changed" };
    const gone = this.rows("SELECT version FROM versions WHERE user = ? AND id = ?", user, id);
    this.sql.exec("DELETE FROM versions WHERE user = ? AND id = ?", user, id);
    this.sql.exec("DELETE FROM stories WHERE user = ? AND id = ?", user, id);
    return { ok: true, drop: gone.map(v => key(user, id, v.version)) };
  }

  removeAll(user) {
    if (!this.known(user)) return { error: 401, message: "Not signed in" };
    const gone = this.rows("SELECT id, version FROM versions WHERE user = ?", user);
    // Who painted how many goes too; the day's total for the site stays, so it is no way round the limit.
    for (const table of ["versions", "stories", "pending", "scene_days"]) this.sql.exec(`DELETE FROM ${table} WHERE user = ?`, user);
    return { ok: true, drop: gone.map(v => key(user, v.id, v.version)) };
  }
}
