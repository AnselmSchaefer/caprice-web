// For the tests only (never deployed: wrangler bundles from worker.js): the Worker as it is, with
// a Ledger that can be made to fail every request as Cloudflare does once the day's Durable Object
// requests are used up, to show what the writer meets then.
import { Ledger } from "./ledger.js";

export { default, Ledger, StoryRoom } from "./worker.js";

export class DownLedger extends Ledger {
  down(on) {
    this.isDown = on;
  }
}

for (const name of Object.getOwnPropertyNames(Ledger.prototype)) {
  const real = Ledger.prototype[name];
  if (name === "constructor" || typeof real !== "function") continue;
  DownLedger.prototype[name] = function (...args) {
    if (this.isDown) throw new Error("Exceeded allowed volume of requests in Durable Objects free tier.");
    return real.apply(this, args);
  };
}
