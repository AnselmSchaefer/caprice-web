// Opens Caprice in headless Chromium and goes through what the browser version can do so far,
// taking screenshots: node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
// It prints what it found, and every error on the page.
import { chromium } from "playwright-core";

const out = process.argv[2] ?? ".";
const url = process.env.CAPRICE_URL ?? "http://localhost:8080/";
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", e => errors.push(e.message.split("\n")[0]));
page.on("console", m => { if (m.type() === "error") errors.push(m.text().split("\n")[0]); });

// egui opens a menu on a press and a release in different frames, which an instant click is not.
async function click(x, y) {
  await page.mouse.move(x, y);
  await page.waitForTimeout(150);
  await page.mouse.down();
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(800);
}

// The paths of the files the browser keeps for Caprice (IndexedDB).
const kept = () => page.evaluate(() => new Promise((done, failed) => {
  const r = indexedDB.open("caprice", 1);
  r.onsuccess = () => {
    const req = r.result.transaction("files", "readonly").objectStore("files").getAllKeys();
    req.onsuccess = () => done(req.result);
    req.onerror = () => failed(req.error);
  };
  r.onerror = () => failed(r.error);
}));

await page.goto(url);
console.log("crossOriginIsolated:", await page.evaluate(() => crossOriginIsolated));
await page.waitForTimeout(4000);
await page.screenshot({ path: `${out}/1-welcome.png` });

// "Open another…" asks for a file in the background: a Web Worker, which chooses nothing yet.
const workers = [];
page.on("worker", w => workers.push(w.url()));
await click(1030, 578);
console.log("workers started:", workers.length);

// The welcome's "Blank page", then a story.
await click(280, 340);
await page.keyboard.type("Once upon a time, in a city of towers, a dragon carried the title of a story.", { delay: 15 });
await page.waitForTimeout(1000);
await page.screenshot({ path: `${out}/2-typed.png` });

// File → Versions… keeps a version of the story, in the browser's files.
await click(85, 38);
await click(100, 265);
await page.waitForTimeout(1000);
await page.screenshot({ path: `${out}/3-versions.png` });
console.log("files kept:", await kept());

// After a reload the version is still there: a new blank story lists it as a story never saved.
await page.reload();
await page.waitForTimeout(4000);
await click(280, 340);
// The sheet flies onto the desk first: a click before it lands can be lost.
await page.waitForTimeout(1500);
await click(85, 38);
await click(100, 265);
await page.waitForTimeout(1000);
await click(560, 217);
await page.screenshot({ path: `${out}/4-versions-after-reload.png` });
console.log("files kept after reload:", await kept());

console.log(errors.length ? "errors:\n  " + [...new Set(errors)].slice(0, 8).join("\n  ") : "no errors");
await browser.close();
