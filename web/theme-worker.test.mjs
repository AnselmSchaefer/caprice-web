// The browser's host for theme programs (theme-worker.js) makes the words the tests' host makes
// (src/platform/program.rs, `lift`, under wasmi): the echo theme asked what the Rust test asked,
// word for word (tests/fixtures/echo.words.json, written by that test). node --test web/theme-worker.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { load } from "./theme-worker.js";

const fixtures = new URL("../tests/fixtures/", import.meta.url);

test("the browser's host makes the same words of a frame as the tests' host", async () => {
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
  const frame = await load(fs.readFileSync(new URL("fireflies.wasm", fixtures)));
  const input = new Uint8Array(120), v = new DataView(input.buffer);
  [0, 0, 1400, 900, 450, 0, 950, 900].forEach((x, i) => v.setFloat32(8 + 4 * i, x, true));
  v.setFloat32(60, 1, true); v.setFloat32(64, 1, true);
  let words = new Uint32Array(16);
  for (let t = 0; t < 3; t++) {
    v.setFloat64(0, t, true);
    const made = frame({ input: input.buffer, events: new ArrayBuffer(0), seed: 7n, garden: Uint32Array.of(3, 4), pages: 7, lift: true }, words);
    words = made.words;
    assert.ok(made.words[0] > 30 && made.words[5] === 1, "shapes, and it asked to carry titles");
  }
});
