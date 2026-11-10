// The day's scenes, counted in R2 as well, for when the Ledger cannot be asked. Cloudflare's free
// plan allows the Durable Objects so many requests a day for the whole site, and once stories
// online and live editing have used them up the Ledger answers nothing until midnight UTC.
// Painting needs only Claude, so it goes on, and the day's scenes still bound the bill: the
// Ledger's count is copied here as it grows (`noted`), and while the Ledger is out of reach a
// scene is counted here alone (`tally`). R2's conditional writes stand in for the Ledger's calls
// one at a time: a write made on a count someone changed meanwhile fails, and is tried again, so
// two writers asking for the last scene at once cannot both have it.
//
// One small object, read and written once a scene: SITE_SCENES_PER_DAY bounds what it costs R2.

const KEY = "tally/scenes.json";

const day = now => new Date(now).toISOString().slice(0, 10);

function untilTomorrow(now) {
  const next = new Date(now);
  next.setUTCHours(24, 0, 0, 0);
  return Math.ceil((next - now) / 1000);
}

// The count made `next(n)`, from today's `n` (yesterday's is 0); `next` returning undefined
// writes nothing. What it was, and what it is now.
async function change(bucket, now, next) {
  for (let tries = 0; tries < 20; tries++) {
    const got = await bucket.get(KEY);
    const kept = got ? await got.json() : null;
    const n = kept?.day === day(now) ? kept.n : 0;
    const to = next(n);
    if (to === undefined) return { was: n, is: n };
    // Only over the copy just read, or only if there is none yet.
    const onlyIf = got ? { etagMatches: got.etag } : { etagDoesNotMatch: "*" };
    if (await bucket.put(KEY, JSON.stringify({ day: day(now), n: to }), { onlyIf })) return { was: n, is: to };
  }
  throw new Error("The day's scenes could not be counted");
}

// The Ledger has counted `n` scenes today: the count here is at least as many.
export function noted(bucket, n, now = Date.now()) {
  return change(bucket, now, have => (n > have ? n : undefined));
}

// A scene without the Ledger: one of the day's while they last, answered as `Ledger::paint` answers.
export async function tally(bucket, most, now = Date.now()) {
  const { was, is } = await change(bucket, now, have => (have < most ? have + 1 : undefined));
  if (is > was) return { ok: true, left: most - is };
  return { error: 503, message: `No more scenes today: Caprice paints ${most} a day for everyone. More tomorrow.`, retry: untilTomorrow(now) };
}
