// Opens Caprice in headless Chromium and goes through what the browser version can do so far,
// taking screenshots: node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
// It prints what it found, and every error on the page.
import { chromium } from "playwright-core";
import path from "node:path";
import { fileURLToPath } from "node:url";

const out = process.argv[2] ?? ".";
const url = process.env.CAPRICE_URL ?? "http://localhost:8080/";
const icon = path.join(path.dirname(fileURLToPath(import.meta.url)), "../assets/icon-256.png");
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", e => errors.push(e.message.split("\n")[0]));
page.on("console", m => { if (m.type() === "error") errors.push(m.text().split("\n")[0]); });
const workers = [];
page.on("worker", w => workers.push(w.url()));

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
const stories = async () => (await kept()).filter(k => k.startsWith("/caprice/stories/"));

// Clicks that give a download; saved to `out`, its name returned.
async function download(...clicks) {
  const [d] = await Promise.all([
    page.waitForEvent("download", { timeout: 5000 }),
    (async () => { for (const [x, y] of clicks) await click(x, y); })(),
  ]);
  await d.saveAs(`${out}/${d.suggestedFilename()}`);
  return d.suggestedFilename();
}

// Clicks that open the page's file picker, which is given `file`.
async function upload(file, ...clicks) {
  const [chooser] = await Promise.all([
    page.waitForEvent("filechooser", { timeout: 5000 }),
    (async () => { for (const [x, y] of clicks) await click(x, y); })(),
  ]);
  await chooser.setFiles(file);
  await page.waitForTimeout(1500);
}

const FILE = [85, 38], OPEN = [93, 75], SAVE = [84, 113], EXPORT = [140, 189], PRINT = [90, 227];
const VERSIONS = [100, 265], DOWNLOAD = [120, 379];

await page.goto(url);
console.log("crossOriginIsolated:", await page.evaluate(() => crossOriginIsolated));
await page.waitForTimeout(4000);
await page.screenshot({ path: `${out}/1-welcome.png` });

// The welcome's "Blank page", then a story. The sheet flies onto the desk first.
await click(280, 340);
await page.waitForTimeout(1000);
await page.keyboard.type("Once upon a time, in a city of towers, a dragon carried the title of a story.", { delay: 15 });
await page.waitForTimeout(1000);
await page.screenshot({ path: `${out}/2-typed.png` });

// A picture from the computer goes into the story.
await page.keyboard.press("Enter");
await upload(icon, [797, 38], [803, 75]);
await page.screenshot({ path: `${out}/3-picture.png` });

// The Print window asks for printers in the background (a Web Worker); the pages as a PDF are a download.
console.log("PDF:", await download(FILE, PRINT, [758, 389]));
console.log("workers started:", workers.length);

// File → Versions… keeps a version of the story, in the browser's files.
await click(...FILE);
await click(...VERSIONS);
await page.waitForTimeout(1000);
await page.screenshot({ path: `${out}/4-versions.png` });
console.log("versions kept:", (await kept()).filter(k => k.startsWith("/caprice/versions/")).length);

// After a reload the version is still there: a new blank story lists it as a story never saved.
await page.reload();
await page.waitForTimeout(4000);
await click(280, 340);
await page.waitForTimeout(1500);
await click(...FILE);
await click(...VERSIONS);
await page.waitForTimeout(1000);
await click(560, 217);
await page.screenshot({ path: `${out}/5-versions-after-reload.png` });
await click(782, 107);

// Saving in the browser asks for a name; the story is kept on the shelf.
await click(640, 300);
await page.keyboard.type("The dragon came at dusk.", { delay: 15 });
await click(...FILE);
await click(...SAVE);
await page.keyboard.press("Enter");
await page.waitForTimeout(800);
console.log("stories kept:", await stories());

// A copy of the story, and the story as Word, are downloads.
const copy = await download(FILE, DOWNLOAD);
console.log("downloaded:", copy, "and", await download(FILE, EXPORT));

// Open lists the stories kept, and a file from the computer can be brought in.
await click(...FILE);
await click(...OPEN);
await page.waitForTimeout(800);
await page.screenshot({ path: `${out}/6-open.png` });
await upload(`${out}/${copy}`, [565, 190]);
console.log("stories kept after the upload:", await stories());

// Opened again, the welcome lists the stories last opened or saved.
await page.reload();
await page.waitForTimeout(4000);
await page.screenshot({ path: `${out}/7-welcome-again.png` });

console.log(errors.length ? "errors:\n  " + [...new Set(errors)].slice(0, 8).join("\n  ") : "no errors");
await browser.close();
