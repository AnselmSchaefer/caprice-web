// A story's room: one Durable Object for each story online of each writer, where the devices it
// is open on meet (LIVE_PLAN.md, ARCHITECTURE.md R34). One device holds the pen and writes; its
// edits come here in numbered batches and go on to every other device, which follows and cannot
// edit. Transfer here moves the pen; a device gone for LIVE_FREE_AFTER_MS frees it.
//
// The room is named from the user and the story's key, both checked by the Worker (worker.js),
// so only one writer's own devices ever meet in it. It keeps no story: the edits pass through.
// The sockets hibernate, so a quiet room costs nothing. On the free plan every message in counts
// as a request and every write to storage as a row, out of 100,000 a day for the whole site, so a
// batch writes nothing here: its number goes with the holder's socket, and the day's count is
// written twenty at a time. The room passes on only messages it was given by the Ledger, taken
// from live editing's share for the whole site so many at a time (`allowed`), so that live
// editing leaves the rest of the 100,000 to stories online.
//
// What the devices say, as JSON:
//   to the room   edits { seq, edits, hash, at }   only the holder; seq one on from the last
//                 take                             Transfer here
//                 given                            the holder, once its last batch is sent
//                 saved { version }                the holder saved the story online
//                 whole                            a follower lost its way: the holder saves
//   from the room hello { you, holder, name, seq, version }   on coming in
//                 pen { holder, name, seq, version }          the pen moved
//                 edits { seq, edits, hash, at }              to every other device
//                 give, save                                  to the holder
//                 version { version, seq }                    the story online to fetch
//                 refused { why, seq }                        that message was not taken
import { DurableObject } from "cloudflare:workers";
import { limits } from "./ledger.js";

const day = now => new Date(now).toISOString().slice(0, 10);

export class StoryRoom extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    this.limits = limits(env);
    this.sql = ctx.storage.sql;
    this.sql.exec(`
      CREATE TABLE IF NOT EXISTS state (k TEXT PRIMARY KEY, v TEXT);
      CREATE TABLE IF NOT EXISTS counts (day TEXT PRIMARY KEY, n INTEGER NOT NULL);
    `);
    // A device's "ping" is answered without waking the room, so keeping a socket open is free.
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair("ping", "pong"));
  }

  get(k, otherwise = null) {
    const row = this.sql.exec("SELECT v FROM state WHERE k = ?", k).toArray()[0];
    return row ? JSON.parse(row.v) : otherwise;
  }

  set(k, v) {
    if (JSON.stringify(this.get(k)) === JSON.stringify(v)) return;
    this.sql.exec("INSERT INTO state VALUES (?, ?) ON CONFLICT (k) DO UPDATE SET v = excluded.v", k, JSON.stringify(v));
  }

  // The devices here now, each once, in the order they came.
  devices() {
    return this.ctx.getWebSockets()
      .filter(ws => ws.readyState === WebSocket.OPEN)
      .map(ws => ({ ws, ...ws.deserializeAttachment() }))
      .sort((a, b) => a.since - b.since);
  }

  async fetch(request) {
    const user = request.headers.get("X-Room-User");
    const device = request.headers.get("X-Device");
    const name = request.headers.get("X-Device-Name");
    const until = Number(request.headers.get("X-Until"));
    // The Worker names the room from the user, so this never happens; if it did, no one else's
    // device may come in.
    const owner = this.get("user");
    if (owner && owner !== user) return new Response("Not your story", { status: 403 });
    if (!owner) this.set("user", user);
    if (this.used() >= this.limits.ROOM_MESSAGES_PER_DAY) {
      return new Response("Live editing is paused for this story until tomorrow", { status: 503 });
    }
    // The same device again (a reload) takes the place of its old socket.
    for (const ws of this.ctx.getWebSockets(device)) ws.close(4000, "Opened again");
    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server, [device]);
    server.serializeAttachment({ device, name, until, since: Date.now() });
    await this.settle();
    const pen = this.get("pen");
    server.send(JSON.stringify({ t: "hello", you: device, holder: pen?.device ?? null, name: pen?.name ?? null, seq: this.seq(), version: this.get("version") }));
    return new Response(null, { status: 101, webSocket: client });
  }

  // The day's messages: those written down, and those since (lost if the room is put away
  // meanwhile, a few at most).
  used(now = Date.now()) {
    const written = this.sql.exec("SELECT n FROM counts WHERE day = ?", day(now)).toArray()[0]?.n ?? 0;
    return written + (this.unwritten?.day === day(now) ? this.unwritten.n : 0);
  }

  count(now = Date.now()) {
    if (this.unwritten?.day !== day(now)) this.unwritten = { day: day(now), n: 0 };
    if (++this.unwritten.n < 20) return;
    this.sql.exec("DELETE FROM counts WHERE day < ?", day(now));
    this.sql.exec("INSERT INTO counts VALUES (?, ?) ON CONFLICT (day) DO UPDATE SET n = n + excluded.n", day(now), this.unwritten.n);
    this.unwritten.n = 0;
  }

  // One more message out of those the Ledger gave this room today, asking it for more when they
  // are used up. Kept in memory only: a room put away loses the few it had, which stay counted.
  // The Ledger out of reach (Cloudflare's daily limit, a fault) passes on nothing more.
  async allowed(now = Date.now()) {
    if (this.given?.day !== day(now)) this.given = { day: day(now), n: 0 };
    if (this.given.n <= 0) {
      let answer;
      try {
        answer = await this.env.LEDGER.get(this.env.LEDGER.idFromName("ledger")).liveMessages();
      } catch (e) {
        console.error("room: no messages from the Ledger:", e?.message);
        return false;
      }
      if (!answer.ok) return false;
      this.given.n = answer.n;
    }
    this.given.n -= 1;
    return true;
  }

  // The number of the last batch: the holder's socket carries it, and storage has it once the
  // pen moves or the holder goes.
  holderSocket() {
    const pen = this.get("pen");
    return this.devices().find(d => d.device === pen?.device) ?? null;
  }

  seq() {
    return this.holderSocket()?.seq ?? this.get("seq", 0);
  }

  send(ws, message) {
    try {
      ws.send(JSON.stringify(message));
    } catch {
      // Gone meanwhile: webSocketClose follows.
    }
  }

  everyone(message, but = null) {
    for (const d of this.devices()) if (d.device !== but) this.send(d.ws, message);
  }

  // Who holds the pen, after anyone came, went or asked for it. The holder here keeps it. Gone,
  // it keeps it for LIVE_FREE_AFTER_MS, then the first device here has it. A device that asked
  // for it has it once the holder gave it, or after LIVE_HAND_OVER_MS without an answer, or at
  // once if the holder is not here.
  async settle(now = Date.now()) {
    const here = this.devices();
    const isHere = id => here.some(d => d.device === id);
    let pen = this.get("pen");
    let gone = this.get("gone");
    let asked = this.get("asked");
    if (asked && !isHere(asked.device)) asked = null;
    const before = pen?.device ?? null;
    this.set("seq", this.seq());

    if (pen && isHere(pen.device)) gone = null;
    else if (pen && gone === null) gone = now;
    if (asked && (!pen || !isHere(pen.device) || now >= asked.until)) {
      pen = { device: asked.device, name: asked.name };
      asked = null;
      gone = null;
    }
    if ((!pen || (gone !== null && now - gone >= this.limits.LIVE_FREE_AFTER_MS)) && here.length) {
      pen = { device: here[0].device, name: here[0].name };
      gone = null;
    }
    this.set("pen", pen);
    this.set("gone", gone);
    this.set("asked", asked);

    // Woken only to hand the pen to a device here: in an empty room the pen stays with the one gone,
    // and the next device to come in settles it (`fetch`). Every wake is a request, out of 100,000 a
    // day for the whole site; one due already (the pen gone long ago, no one here to take it) would
    // wake the room again at once, for ever.
    const wake = here.length ? [gone !== null ? gone + this.limits.LIVE_FREE_AFTER_MS : null, asked?.until ?? null].filter(t => t !== null) : [];
    const at = wake.length ? Math.max(Math.min(...wake), now + 1000) : null;
    if (at !== await this.ctx.storage.getAlarm()) {
      if (at !== null) await this.ctx.storage.setAlarm(at);
      else await this.ctx.storage.deleteAlarm();
    }
    // The first device into an empty room hears it in its hello.
    if (before !== null && pen && pen.device !== before) {
      this.everyone({ t: "pen", holder: pen.device, name: pen.name, seq: this.get("seq", 0), version: this.get("version") });
      const holder = this.holderSocket();
      if (holder) holder.ws.serializeAttachment({ ...holder.ws.deserializeAttachment(), seq: this.get("seq", 0) });
    }
  }

  async alarm() {
    this.woken = (this.woken ?? 0) + 1;
    await this.settle();
  }

  // For the tests: when the room's alarm wakes it next, and how often it has since it was loaded.
  async wakes() {
    return { at: await this.ctx.storage.getAlarm(), woken: this.woken ?? 0 };
  }

  async webSocketClose(ws) {
    // The holder going: the number of its last batch is written down first.
    const me = ws.deserializeAttachment();
    if (me?.device === this.get("pen")?.device && me.seq !== undefined) this.set("seq", me.seq);
    ws.close();
    await this.settle();
  }

  async webSocketError(ws) {
    await this.settle();
  }

  // Each message in its turn: asking the Ledger for more (`allowed`) lets the next one in
  // meanwhile, and a writer's batches must go on in the order they came.
  webSocketMessage(ws, data) {
    const turn = (this.turn ?? Promise.resolve()).then(() => this.message(ws, data));
    this.turn = turn.catch(() => {});
    return turn;
  }

  async message(ws, data) {
    const me = ws.deserializeAttachment();
    if (typeof data !== "string" || data.length > this.limits.LIVE_MESSAGE_BYTES) return ws.close(1009, "Too big");
    // The session it came in with has run out: signed in no longer, as far as this socket knows.
    if (Date.now() > me.until) return ws.close(4001, "Signed out");
    if (this.used() >= this.limits.ROOM_MESSAGES_PER_DAY) {
      for (const d of this.devices()) d.ws.close(4002, "Live editing is paused for this story until tomorrow");
      return;
    }
    if (!await this.allowed()) {
      for (const d of this.devices()) d.ws.close(4002, "Live editing is paused until tomorrow");
      return;
    }
    this.count();
    let m;
    try {
      m = JSON.parse(data);
    } catch {
      return this.send(ws, { t: "refused", why: "not json", seq: this.seq() });
    }
    const pen = this.get("pen");
    const holding = pen?.device === me.device;
    switch (m.t) {
      case "edits": {
        const seq = me.seq ?? this.get("seq", 0);
        if (!holding) return this.send(ws, { t: "refused", why: "not the pen", seq: this.seq() });
        if (m.seq !== seq + 1) return this.send(ws, { t: "refused", why: "out of order", seq });
        ws.serializeAttachment({ ...me, seq: m.seq });
        return this.everyone({ t: "edits", seq: m.seq, edits: m.edits, hash: m.hash, at: m.at }, me.device);
      }
      case "take": {
        if (holding) return;
        const here = this.devices().some(d => d.device === pen?.device);
        this.set("asked", { device: me.device, name: me.name, until: Date.now() + this.limits.LIVE_HAND_OVER_MS });
        if (here) this.toHolder({ t: "give" });
        return this.settle();
      }
      case "given": {
        const asked = this.get("asked");
        if (!holding || !asked) return;
        this.set("asked", { ...asked, until: 0 });
        return this.settle();
      }
      case "saved": {
        if (!holding || typeof m.version !== "string" || m.version.length > 100) return;
        this.set("version", m.version);
        return this.everyone({ t: "version", version: m.version, seq: this.seq() }, me.device);
      }
      case "whole":
        if (!holding) this.toHolder({ t: "save" });
        return;
      default:
        return this.send(ws, { t: "refused", why: "unknown", seq: this.seq() });
    }
  }

  toHolder(message) {
    const pen = this.get("pen");
    for (const d of this.devices()) if (d.device === pen?.device) this.send(d.ws, message);
  }
}
