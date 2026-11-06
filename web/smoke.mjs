// Opens Caprice in headless Chromium and goes through what the browser version can do so far,
// taking screenshots: node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
// It says what it found, and fails (exit code 1) if any check fails or the page reports an error.
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

// Each check says what it found; any that fails, or any error on the page, fails the run (CI).
const failed = [];
function check(name, ok, seen = "") {
  console.log(`${ok ? "✓" : "✗"} ${name}${seen === "" ? "" : `: ${seen}`}`);
  if (!ok) failed.push(name);
}
page.on("worker", w => workers.push(w.url()));
// Every request to Caprice's own server (R33), with the header that says it is the app asking.
const asked = [];
page.on("request", r => { if (new URL(r.url()).pathname.startsWith("/api/")) asked.push([r.method(), new URL(r.url()).pathname, r.headers()["x-caprice"]]); });
// web/serve.mjs has no server behind /api: the page is told Google sign-in is there.
await page.route("**/api/providers", r => r.fulfill({ status: 200, contentType: "application/json", body: '["google"]' }));

// egui opens a menu on a press and a release in different frames, which an instant click is not.
async function click(x, y) {
  await page.mouse.move(x, y);
  await page.waitForTimeout(150);
  await page.mouse.down();
  await page.waitForTimeout(100);
  await page.mouse.up();
  await page.waitForTimeout(800);
}

// The app has started once the page's "Caprice is opening…" is gone (it may first download several
// MB from far away), and drawn its first frames a moment later.
async function started() {
  await page.waitForSelector("#loading", { state: "detached", timeout: 120000 });
  await page.waitForTimeout(2500);
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
// What a file kept in IndexedDB says.
const keptText = path => page.evaluate(path => new Promise((done, failed) => {
  const r = indexedDB.open("caprice", 1);
  r.onsuccess = () => {
    const req = r.result.transaction("files", "readonly").objectStore("files").get(path);
    req.onsuccess = () => done(req.result ? new TextDecoder().decode(req.result.bytes) : "");
    req.onerror = () => failed(req.error);
  };
  r.onerror = () => failed(r.error);
}), path);

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
// The File menu's items, a row each: New, Open, Save, Save as, Export, Print, Versions, Drop changes,
// Document info, Download a copy, Stories online.
const FILE = [85, 38], NEW = [80, 75], OPEN = [93, 113], SAVE = [84, 151], EXPORT = [140, 227], PRINT = [90, 265];
const VERSIONS = [100, 303], DOWNLOAD = [120, 417], ONLINE = [110, 455];

await page.goto(url);
check("the page is cross-origin isolated, so the workers share memory", await page.evaluate(() => crossOriginIsolated));
// The Content-Security-Policy is in force: an inline script that is not one of the page's own is
// refused, and the browser says so. (Playwright's own evaluate is exempt from the policy.)
const errorsBefore = errors.length;
check("the Content-Security-Policy refuses a stranger's script", await page.evaluate(() => new Promise(done => {
  addEventListener("securitypolicyviolation", e => done(e.violatedDirective.startsWith("script-src")), { once: true });
  const s = document.createElement("script");
  s.textContent = "window.stranger = true;";
  document.body.append(s);
  setTimeout(() => done(false), 1000);
})));
await page.waitForTimeout(300);
errors.splice(errorsBefore); // the refusal, reported as it should be
await started();
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
const pdf = await download(FILE, PRINT, [691, 286]);
check("Save PDF downloads the pages", pdf.endsWith(".pdf"), pdf);
// (Headless Chromium shows a PDF tab with no address, so what the page opens is noted on the way.)
await page.evaluate(() => {
  const open = window.open;
  window.opened = [];
  window.open = (url, ...rest) => (window.opened.push(String(url)), open.call(window, url, ...rest));
});
await click(...FILE);
await click(...PRINT);
await click(770, 286);
const opened = await page.evaluate(() => window.opened.map(u => u.split(":")[0]));
check("Print… opens the PDF in a new tab", opened[0] === "blob", opened.join(", "));
for (const tab of page.context().pages().filter(p => p !== page)) await tab.close();

// File → Versions… keeps a version of the story, in the browser's files.
await click(...FILE);
await click(...VERSIONS);
await page.waitForTimeout(1000);
await page.screenshot({ path: `${out}/4-versions.png` });
const versions = (await kept()).filter(k => k.startsWith("/caprice/versions/")).length;
check("versions are kept in IndexedDB", versions > 0, versions);

// After a reload the version is still there: a new blank story lists it as a story never saved.
await page.reload();
await started();
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
check("Save keeps the story in the browser under its name",
  (await stories()).includes("/caprice/stories/The dragon came at dusk.caprice"));

// A copy of the story, and the story as Word, are downloads.
const copy = await download(FILE, DOWNLOAD);
const word = await download(FILE, EXPORT);
check("a copy and the Word file download", copy.endsWith(".caprice") && word.endsWith(".docx"), `${copy}, ${word}`);

// Open lists the stories kept, and a file from the computer can be brought in.
await click(...FILE);
await click(...OPEN);
await page.waitForTimeout(800);
await page.screenshot({ path: `${out}/6-open.png` });
await upload(`${out}/${copy}`, await uploadButton());
check("an uploaded story is kept beside the one of the same name",
  (await stories()).includes("/caprice/stories/The dragon came at dusk 2.caprice"));

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
check("the scene is drawn in a Web Worker", workers.length > 0, workers.length);

// Caprice's shortcuts are not the browser's: Ctrl+R aligns right rather than reloading the page,
// and Ctrl+F finds in the story.
await click(640, 500);
await page.keyboard.type(" Then night fell.", { delay: 15 });
await page.evaluate(() => { window.stillHere = true; });
await page.keyboard.press("Control+r");
await page.waitForTimeout(1000);
check("Ctrl+R aligns right rather than reloading the page", await page.evaluate(() => window.stillHere === true));
await page.keyboard.press("Control+f");
await page.waitForTimeout(800);
await page.screenshot({ path: `${out}/8-shortcuts.png` });
await page.keyboard.press("Escape");

// With changes autosave has not taken yet, the browser asks before leaving: Caprice cancels the
// page's beforeunload. (Headless Chromium shows no "Leave site?" itself, so the event is sent here.)
const leavingAsks = () => page.evaluate(() => {
  const e = new Event("beforeunload", { cancelable: true });
  dispatchEvent(e);
  return e.defaultPrevented;
});
await page.waitForTimeout(6000);
check("leaving with changes not saved asks first", await leavingAsks());

// Hidden (another tab, or closing), the story is saved at once.
await page.evaluate(() => {
  Object.defineProperty(Document.prototype, "hidden", { get: () => true, configurable: true });
  document.dispatchEvent(new Event("visibilitychange"));
});
await page.waitForTimeout(1500);
check("hiding the page saves the story", (await keptText("/caprice/stories/With a scene.caprice")).includes("night fell"));
await page.evaluate(() => {
  Object.defineProperty(Document.prototype, "hidden", { get: () => false, configurable: true });
  document.dispatchEvent(new Event("visibilitychange"));
});
// The next look at the story finds it saved, and leaving asks no more.
await page.waitForTimeout(6000);
check("once saved, leaving asks nothing", !(await leavingAsks()));

// Typing beyond ASCII, through an input method too (a composition, then what it settles on),
// and copying to the computer's clipboard.
await page.context().grantPermissions(["clipboard-read", "clipboard-write"], { origin: new URL(url).origin });
await click(640, 500);
await page.keyboard.press("Control+End");
await page.keyboard.type(" Café, naïve, Größe, «Ελληνικά» ", { delay: 15 });
const cdp = await page.context().newCDPSession(page);
await cdp.send("Input.imeSetComposition", { text: "ni", selectionStart: 2, selectionEnd: 2 });
await cdp.send("Input.insertText", { text: "你好" });
await page.waitForTimeout(500);
await page.keyboard.press("Control+a");
await page.keyboard.press("Control+c");
await page.waitForTimeout(500);
const copied = await page.evaluate(() => navigator.clipboard.readText());
check("typing beyond ASCII and through an input method, and copying", copied.endsWith("Café, naïve, Größe, «Ελληνικά» 你好"));
await page.waitForTimeout(6000);

// Signed out, nothing was asked of the server; File → Stories online asks it how to sign in.
check("signed out, nothing is asked of the server", asked.length === 0, asked.map(a => a.slice(0, 2).join(" ")).join(", "));
await click(...FILE);
await click(...ONLINE);
await page.waitForTimeout(800);
await page.screenshot({ path: `${out}/9-online.png` });
const providers = asked.find(([m, p]) => m === "GET" && p === "/api/providers");
check("Stories online asks how to sign in, as the app", providers?.[2] === "1", asked.map(a => a.join(" ")).join(", "));

// File → New: the story has changes since it was saved by hand, so it asks about them first.
await click(...FILE);
await click(...NEW);
await page.waitForTimeout(800);
await page.screenshot({ path: `${out}/9-new.png` });

// Opened again, the welcome lists the stories last opened or saved.
await page.reload();
await started();
await page.screenshot({ path: `${out}/10-welcome-again.png` });

check("no errors on the page", errors.length === 0, [...new Set(errors)].slice(0, 8).join("\n  "));

// The page itself is never kept, so a new build is what opens; and a page kept from a build before,
// naming files a new one took away, reloads itself once and opens rather than "opening" for good.
check("the page itself is never kept", ((await fetch(url)).headers.get("cache-control") ?? "").includes("no-store"));
// The snippets keep their names across builds: asked about each time, never kept for good.
const snippet = (await (await fetch(url)).text()).match(/snippets\/[^"]+\.js/)?.[0];
check("the snippets, whose names stay, are asked about each time", snippet !== undefined && ((await fetch(new URL(snippet, url))).headers.get("cache-control") ?? "").includes("no-cache"), snippet);
const stale = await browser.newPage({ viewport: { width: 1280, height: 860 } });
let fetched = 0;
await stale.route(/caprice-web-[0-9a-f]+\.js$/, r => fetched++ === 0 ? r.fulfill({ status: 404, body: "" }) : r.continue());
await stale.goto(url);
const recovered = await stale.waitForSelector("#loading", { state: "detached", timeout: 120000 }).then(() => true, () => false);
check("a file of it gone at first is fetched again, and Caprice opens", recovered && fetched >= 2, `fetched ${fetched} times`);
await stale.close();
// A snippet the browser kept from a build before, which fails the integrity the new page asks.
const old = await browser.newPage({ viewport: { width: 1280, height: 860 } });
// Every snippet is an old copy the first time it is asked for, as when the browser kept them all.
let served = 0;
const once = new Set();
await old.route(/snippets\/.+\.js$/, r => (served++, once.has(r.request().url())) ? r.continue() : (once.add(r.request().url()), r.fulfill({ status: 200, contentType: "text/javascript", body: "// an older copy" })));
await old.goto(url);
const renewed = await old.waitForSelector("#loading", { state: "detached", timeout: 120000 }).then(() => true, () => false);
check("an old copy of a file failing its integrity is fetched afresh, and Caprice opens", renewed, `served ${served} times`);
await old.close();

// When it cannot open, the page says why instead of "opening" for good: a phone has no console.
const broken = await browser.newPage({ viewport: { width: 412, height: 839 } });
await broken.route(/_bg\.wasm$/, r => r.fulfill({ status: 200, contentType: "application/wasm", body: Buffer.from("not wasm") }));
await broken.goto(url);
await broken.waitForFunction(() => document.getElementById("loading")?.className === "failed", null, { timeout: 30000 }).catch(() => {});
const told = await broken.evaluate(() => document.getElementById("loading")?.textContent ?? "");
check("when it cannot open, the page says why", told.startsWith("Caprice could not open:") && told.includes("WebAssembly"), told);
await broken.screenshot({ path: `${out}/11-cannot-open.png` });
await broken.close();
await browser.close();
console.log(failed.length ? `${failed.length} failed` : "all passed");
process.exit(failed.length ? 1 : 0);
