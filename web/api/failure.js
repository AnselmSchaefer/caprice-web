// What a request that failed answers. Most failures are a fault, and say only that. One is not:
// Cloudflare's free plan allows the site so many Durable Object requests a day, and once they are
// used up every story online is refused until midnight UTC (signing in and painting scenes go on
// without them: worker.js, tally.js). That is said plainly, with when it lifts, so the writer knows
// it is not their doing and the app waits so long.
// The reason only, never anything of a story.

const DAY = 86_400_000;

export const OVER_LIMIT = /exceeded allowed .* free tier/i;

export function failure(e, now = Date.now()) {
  if (!OVER_LIMIT.test(e?.message ?? "")) return { status: 500, said: "Something went wrong", retry: null };
  const seconds = Math.max(60, Math.ceil((DAY - now % DAY) / 1000));
  const hours = Math.round(seconds / 3600);
  const when = hours < 1 ? "within the hour" : hours === 1 ? "in about an hour" : `in about ${hours} hours`;
  return {
    status: 503,
    said: "Caprice's server has used up what its host, Cloudflare, allows it in a day. Stories online are off " +
      `until midnight UTC, ${when}. What you write stays safe in this browser.`,
    retry: seconds,
  };
}
