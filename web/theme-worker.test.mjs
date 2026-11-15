// The browser's host for theme programs (theme-worker.js) makes the words Caprice's tests make
// (src/platform/program.rs, `played`): there no WebAssembly is run, each theme's source is played
// natively in place of its module, its input read out of Caprice's bytes and its output made into
// words. The echo theme, which draws back everything it is told, is asked here as a module what
// the Rust test asked it natively, and must make the same words (tests/fixtures/echo.words.json,
// written by that test): so Caprice's bytes mean to the real module what the tests read in them,
// and the module's output comes back as the tests make it. node --test web/theme-worker.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { load } from "./theme-worker.js";

const fixtures = new URL("../tests/fixtures/", import.meta.url);

test("the browser's host makes the same words of a frame as Caprice's tests make, playing the theme natively", async () => {
  const kept = JSON.parse(fs.readFileSync(new URL("echo.words.json", fixtures), "utf8"));
  const frame = await load(fs.readFileSync(new URL("echo.wasm", fixtures)));
  const ask = {
    input: new Uint8Array(kept.input).buffer, events: new Uint8Array(kept.events).buffer,
    seed: BigInt(kept.seed), garden: Uint32Array.from(kept.garden), pages: kept.pages, lift: kept.lift,
  };
  const { words, n } = frame(ask, new Uint32Array(0));
  assert.deepEqual([...words.subarray(0, n)], kept.words);
});

test("a theme's frame with every kind of shape comes back whole, frame after frame", async () => {
  // The fireflies, which carry titles; the Snow, which draws paths and snow in the window; and the
  // Vineyard, its own sky and thousands of leaves, more than a hundred thousand points.
  for (const [name, asks, window] of [["fireflies", 1, 0], ["snow", 0, 140], ["vineyard", 0, 0]]) {
    const frame = await load(fs.readFileSync(new URL(`${name}.wasm`, fixtures)));
    const input = new Uint8Array(144), v = new DataView(input.buffer);
    [0, 0, 1400, 900, 450, 0, 950, 900].forEach((x, i) => v.setFloat32(8 + 4 * i, x, true));
    v.setFloat32(60, 1, true); v.setFloat32(64, 1, true);
    [0, 0, 1400, 900, 700, 886].forEach((x, i) => v.setFloat32(68 + 4 * i, x, true));
    let words = new Uint32Array(16);
    for (let t = 0; t < 3; t++) {
      v.setFloat64(0, t, true);
      const made = frame({ input: input.buffer, events: new ArrayBuffer(0), seed: 7n, garden: Uint32Array.of(3, 4), pages: 7, lift: true }, words);
      words = made.words;
      assert.ok(made.words[0] > 30, `${name}: shapes on the desk`);
      assert.equal(made.words[1], window, `${name}: shapes in the window`);
      assert.equal(made.words[6], asks, `${name}: whether it asked to carry titles`);
    }
  }
});
