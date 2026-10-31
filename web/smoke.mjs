// Opens Caprice in headless Chromium and goes through what the browser version can do so far,
// taking screenshots: node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
// It prints what it found, and every error on the page.
import { chromium } from "playwright-core";
import fs from "node:fs";
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
    page.waitForEvent("filechooser", { timeout: 5000 }).catch(async e => {
      await page.screenshot({ path: `${out}/failed-upload.png` });
      throw e;
    }),
    (async () => { for (const [x, y] of clicks) await click(x, y); })(),
  ]);
  await chooser.setFiles(file);
  await page.waitForTimeout(1500);
}

// The shelf's upload button, below a row for each story kept.
const uploadButton = async () => [565, 190 + 38 * Math.max(0, (await stories()).length - 1)];
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

// Print: the pages as a PDF are a download, or open in a new tab for the browser to print.
console.log("PDF:", await download(FILE, PRINT, [691, 286]));
// (Headless Chromium shows a PDF tab with no address, so what the page opens is noted on the way.)
await page.evaluate(() => {
  const open = window.open;
  window.opened = [];
  window.open = (url, ...rest) => (window.opened.push(String(url)), open.call(window, url, ...rest));
});
await click(...FILE);
await click(...PRINT);
await click(770, 286);
console.log("printing in a tab:", await page.evaluate(() => window.opened.map(u => u.split(":")[0])));
for (const tab of page.context().pages().filter(p => p !== page)) await tab.close();

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
await upload(`${out}/${copy}`, await uploadButton());
console.log("stories kept after the upload:", await stories());

// A story with a scene in it: the scene is drawn behind the page in a Web Worker.
const story = JSON.parse(fs.readFileSync(`${out}/${copy}`, "utf8"));
const circles = [...Array(40).keys()].map(k => `<circle cx="${(k * 37) % 400}" cy="${(k * 53) % 560}" r="${10 + k % 30}" fill="hsl(${k * 9} 60% 55%)"/>`);
story.scenes = [{ at: 0, versions: [`<svg xmlns="http://www.w3.org/2000/svg" width="400" height="560">${circles.join("")}</svg>`] }];
fs.writeFileSync(`${out}/With a scene.caprice`, JSON.stringify(story));
// The story opened before has its title come in first.
await page.waitForTimeout(2000);
await upload(`${out}/With a scene.caprice`, FILE, OPEN, await uploadButton());
await page.waitForTimeout(2000);
await page.screenshot({ path: `${out}/7-scene.png` });
console.log("workers started:", workers.length);

// Opened again, the welcome lists the stories last opened or saved.
await page.reload();
await page.waitForTimeout(4000);
await page.screenshot({ path: `${out}/8-welcome-again.png` });

console.log(errors.length ? "errors:\n  " + [...new Set(errors)].slice(0, 8).join("\n  ") : "no errors");
await browser.close();
