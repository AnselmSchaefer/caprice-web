// Opens Caprice in headless Chromium and goes through what the browser version can do so far,
// taking screenshots: node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
// It says what it found, and fails (exit code 1) if any check fails or the page reports an error.
import { chromium } from "playwright-core";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { check as galleryCheck } from "./api/theme-file.js";

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
async function click(x, y, on = page) {
  await on.mouse.move(x, y);
  await on.waitForTimeout(150);
  await on.mouse.down();
  await on.waitForTimeout(100);
  await on.mouse.up();
  await on.waitForTimeout(800);
}

// The app has started once the page's "Caprice is opening…" is gone (it may first download several
// MB from far away), and drawn its first frames a moment later.
async function started(on = page) {
  await on.waitForSelector("#loading", { state: "detached", timeout: 120000 });
  await on.waitForTimeout(2500);
}

// The paths of the files the browser keeps for Caprice (IndexedDB).
const kept = (on = page) => on.evaluate(() => new Promise((done, failed) => {
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
// The File menu's items, a row each: New, Close story, Open, Save, Save as, Export, Print, Versions,
// Drop changes, Document info, Download a copy, Stories online.
const FILE = [85, 38], NEW = [80, 75], CLOSE = [95, 113], OPEN = [93, 151], SAVE = [84, 189], EXPORT = [140, 265];
const PRINT = [90, 303], VERSIONS = [100, 341], DOWNLOAD = [120, 455], ONLINE = [110, 493];

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

// A story begun has an entry in the page's history: the back button (a phone's too) closes it and
// shows the welcome again, and the page stays. Where the File menu is on the desk, the welcome has
// none.
const corner = () => page.screenshot({ clip: { x: 60, y: 28, width: 50, height: 20 } });
const welcomeCorner = await corner();
await click(280, 340);
await page.waitForTimeout(1500);
const storyCorner = await corner();
await page.evaluate(() => { window.stillHere = true; history.back(); });
await page.waitForTimeout(1500);
await page.screenshot({ path: `${out}/11-back.png` });
const backCorner = await corner();
check("back closes the story and the welcome shows, and the page stays",
  !storyCorner.equals(welcomeCorner) && backCorner.equals(welcomeCorner) && await page.evaluate(() => window.stillHere === true));

// A theme of one's own (own.rs), put on the shelf as importing one does: its picture covers the
// desk once a worker has read and rendered it, and moves.
const template = fs.readFileSync(path.join(path.dirname(fileURLToPath(import.meta.url)), "../assets/theme-template.svg"));
const colours = { accent: "#ffcf7a", button: "#262a38", button_hover: "#393d49", caret: "#a6874f", desk: "#1c2433", desk_top: "#151e3c", dock: "#101524", dock_edge: "#363a47", field: "#0c101b", light: "no", on_accent: "#080b0f", text: "#e3e4e8", text_dim: "#9da1ad", text_hot: "#ffffff", thumb: "#99825a" };
const shelf = [{ id: "smoketheme", name: "Moonlit Bay", about: "", author: "", colours, grows: true, carries: true, added: 0 }];
// Files put into the browser's own (IndexedDB, as `platform::fs` keeps them), then the page
// loaded again to read them and a blank story opened.
const keptInBrowser = (files) => page.evaluate(([files]) => new Promise((done, failed) => {
  const r = indexedDB.open("caprice", 1);
  r.onsuccess = () => {
    const t = r.result.transaction("files", "readwrite");
    for (const [p, bytes] of files) t.objectStore("files").put({ bytes: Uint8Array.from(bytes), modified: 0 }, p);
    t.oncomplete = done;
    t.onerror = () => failed(t.error);
  };
  r.onerror = () => failed(r.error);
}), [files.map(([p, b]) => [p, Array.from(Buffer.from(b))])]);
const keptAndOpened = async (files, wait) => {
  await keptInBrowser(files);
  await page.reload();
  await started();
  await click(280, 340);
  await page.waitForTimeout(wait);
};
// Beside the page, low on the desk.
const beside = () => page.screenshot({ clip: { x: 20, y: 560, width: 220, height: 200 } });
// The screen's pixel, from a screenshot of it: a WebGL canvas cannot be read back.
const pixelAt = async (x, y, on = page) => {
  const shot = (await on.screenshot({ clip: { x, y, width: 1, height: 1 } })).toString("base64");
  return on.evaluate(async png => {
    const img = new Image();
    img.src = `data:image/png;base64,${png}`;
    await img.decode();
    const g = document.createElement("canvas").getContext("2d");
    g.drawImage(img, 0, 0);
    return Array.from(g.getImageData(0, 0, 1, 1).data);
  }, shot);
};
// Theme programs never come from files (bundle.rs; the owner's decision, 2026-11-16): they are made
// with Claude or taken from the gallery. The picker in the gallery's Yours offers drawings only, and
// a theme file given it anyway is not added. (Page → Theme → Yours → Import a theme…, where they are
// while Midnight Ink is in use.)
await click(280, 340);
await page.waitForTimeout(1500);
const themesBefore = (await kept()).filter(k => k.startsWith("/caprice/themes/"));
const [chooser] = await Promise.all([
  page.waitForEvent("filechooser", { timeout: 5000 }),
  (async () => { for (const [x, y] of [[737, 38], [810, 113], [992, 113], [930, 165]]) await click(x, y); })(),
]);
const accept = await chooser.element().getAttribute("accept");
await chooser.setFiles(path.join(path.dirname(fileURLToPath(import.meta.url)), "../tests/fixtures/snow.caprice-theme"));
await page.waitForTimeout(3000);
await page.screenshot({ path: `${out}/12-theme-file-refused.png` });
const taken = (await kept()).filter(k => k.startsWith("/caprice/themes/") && !themesBefore.includes(k));
check("the gallery's import offers drawings only, and a theme file given it anyway is not added",
  accept === ".svg" && taken.length === 0 && !page.workers().some(w => w.url().endsWith("/theme-worker.js")), `accept ${accept}; ${taken.join(" ")}`);

await keptAndOpened([["/caprice/theme.json", '{"own":"smoketheme"}'], ["/caprice/themes/index.json", JSON.stringify(shelf)], ["/caprice/themes/smoketheme.svg", template]], 6000);
await page.screenshot({ path: `${out}/12-own-theme.png` });
// The cottage and the pines, not the plain desk.
const first = await beside();
await page.waitForTimeout(700);
const later = await beside();
const pixel = await pixelAt(38, 258);
// The sky left of the page is a deep blue (#1d2858 or so); Midnight's desk is all but grey.
check("a theme of one's own covers the desk with its picture", pixel[2] > pixel[0] + 30, pixel.join(","));
check("and its picture moves", !first.equals(later));

// A theme that is a program (R36), on the shelf as one of one's own: it plays in a worker of its
// own and draws the desk, its fireflies moving; one whose frame never ends is ended, and the
// usual desk shows.
const fixtures = path.join(path.dirname(fileURLToPath(import.meta.url)), "../tests/fixtures");
// A grey palette, so the usual desk is told from the program's.
const grey = { ...colours, desk: "#404040", desk_top: "#404040" };
const programs = [
  { id: "smokeprogram", name: "Fireflies", about: "", author: "", colours: grey, grows: true, carries: true, added: 0, program: true },
  { id: "smokespin", name: "Spin", about: "", author: "", colours: grey, grows: true, carries: true, added: 0, program: true },
  { id: "smokesnow", name: "Snow", about: "", author: "", colours: grey, grows: true, carries: false, added: 0, program: true },
  { id: "smokevineyard", name: "Vineyard", about: "", author: "", colours: grey, grows: true, carries: false, added: 0, program: true },
];
// The desk left of the page, where fireflies drift.
const leftOfPage = () => page.screenshot({ clip: { x: 20, y: 100, width: 360, height: 640 } });
const themeWorkers = () => page.workers().filter(w => w.url().endsWith("/theme-worker.js")).length;
await keptAndOpened([["/caprice/theme.json", '{"own":"smokeprogram"}'], ["/caprice/themes/index.json", JSON.stringify(programs)],
  ["/caprice/themes/smokeprogram.wasm", fs.readFileSync(path.join(fixtures, "fireflies.wasm"))],
  ["/caprice/themes/smokespin.wasm", fs.readFileSync(path.join(fixtures, "spin.wasm"))],
  ["/caprice/themes/smokesnow.wasm", fs.readFileSync(path.join(fixtures, "snow.wasm"))],
  ["/caprice/themes/smokevineyard.wasm", fs.readFileSync(path.join(fixtures, "vineyard.wasm"))]], 3000);
await page.screenshot({ path: `${out}/13-program-theme.png` });
const night = await pixelAt(38, 258);
const flying = await leftOfPage();
await page.waitForTimeout(500);
// The night sky the program draws (from 8,12,32 at the top to 30,36,70 at the foot).
check("a program theme plays in a worker of its own and draws the desk", themeWorkers() === 1 && night[0] < 40 && night[2] > night[0] + 15, `${themeWorkers()} workers, ${night.join(",")}`);
check("and its fireflies move", !flying.equals(await leftOfPage()));
// The Magic Snow ported to a program (themes/snow): the snowy hillside at the foot of the desk,
// pale and blue in the dusk, and the snow falling in the window.
await keptInBrowser([["/caprice/theme.json", '{"own":"smokesnow"}']]);
await keptAndOpened([], 3000);
await page.screenshot({ path: `${out}/13-program-snow.png` });
const hillside = await pixelAt(38, 700);
const falling = await leftOfPage();
await page.waitForTimeout(500);
check("the Magic Snow as a program draws its hillside", themeWorkers() === 1 && hillside[2] > 200 && hillside[2] >= hillside[0], `${themeWorkers()} workers, ${hillside.join(",")}`);
check("and its snow falls", !falling.equals(await leftOfPage()));
// The Magic Vineyard ported (themes/vineyard), its module run here as it is nowhere in the tests:
// its own blue sky over the grey desk, and the golden slope with its vines at the foot. More than
// a hundred thousand points a frame.
await keptInBrowser([["/caprice/theme.json", '{"own":"smokevineyard"}']]);
await keptAndOpened([], 3000);
await page.screenshot({ path: `${out}/13-program-vineyard.png` });
// The sky high on the right (the low sun lights the left): the bluest of a few points there, as its
// clouds drift over one or another as time goes.
const skies = [];
for (const [x, y] of [[1250, 110], [1000, 150], [1150, 230], [950, 260], [1240, 280]]) skies.push(await pixelAt(x, y));
const sky = skies.reduce((a, b) => (b[2] - b[0] > a[2] - a[0] ? b : a));
const slope = await pixelAt(38, 790);
check("the Magic Vineyard as a program draws its own sky and its slope", themeWorkers() === 1 && sky[2] > sky[0] + 30 && slope[0] > slope[2] + 30, `${themeWorkers()} workers, sky ${sky.join(",")}, slope ${slope.join(",")}`);
await keptInBrowser([["/caprice/theme.json", '{"own":"smokespin"}']]);
await keptAndOpened([], 4000);
await page.screenshot({ path: `${out}/14-program-stopped.png` });
const usual = await pixelAt(38, 258);
check("a program theme that never answers is ended, and the usual desk shows", themeWorkers() === 0 && Math.abs(usual[2] - usual[0]) < 15, `${themeWorkers()} workers, ${usual.join(",")}`);

// A theme made with Claude (R36, the plan's step 5), in a browser of its own, signed in. Caprice's
// server is a stand-in here (web/serve.mjs has none), and Claude answers every round with the Magic
// Snow's source and palette. The tab builds it with the compiler in workers of its own (fetched the
// first time, 52 MiB) to the very module the toolchain builds anywhere (the fixture, byte for byte),
// plays it off screen and shows Claude its pictures once; kept, it is on the shelf with a picture
// and plays. The compiler is ended once there is nothing more to build.
const compiler = await fetch(new URL("compiler/toolchain.json", url)).then(r => r.ok ? r.json() : null, () => null);
if (!compiler) {
  console.log("- this build has no compiler for themes (web/build.sh found no toolchain): making a theme is not checked");
} else {
  const context = await browser.newContext({ viewport: { width: 1280, height: 860 } });
  const maker = await context.newPage();
  maker.on("pageerror", e => errors.push(e.message.split("\n")[0]));
  maker.on("console", m => { if (m.type() === "error") errors.push(m.text().split("\n")[0]); });
  const here = path.dirname(fileURLToPath(import.meta.url));
  const snow = { source: fs.readFileSync(path.join(here, "../themes/snow/theme.rs"), "utf8"), ...JSON.parse(fs.readFileSync(path.join(here, "../themes/snow/manifest.json"), "utf8")) };
  const rounds = [];
  const json = (r, v, status = 200) => r.fulfill({ status, contentType: "application/json", body: JSON.stringify(v) });
  // The gallery, as the Worker answers (web/api/gallery.js): a theme sent is checked by the Worker's
  // own code and waits for the owner (the writer here); the owner's decision is answered with a
  // signature that is not the gallery's, as no test holds its key.
  const shown = { id: "00000000000000aa", sent: null, checked: null, decided: null, signature: Buffer.alloc(64, 7).toString("base64") };
  const gallery = async (r, at) => {
    const method = r.request().method();
    if (at === "/api/gallery" && method === "POST") {
      shown.sent = r.request().postDataBuffer();
      shown.checked = await galleryCheck(new Uint8Array(shown.sent));
      return shown.checked.why ? r.fulfill({ status: 422, body: shown.checked.why }) : json(r, { id: shown.id, state: "waiting" }, 202);
    }
    const theme = shown.checked?.theme;
    if (at === "/api/gallery") return json(r, { themes: shown.decided ? [{ id: shown.id, ...theme, signature: shown.signature }] : [] });
    if (at === "/api/gallery/mine") return json(r, { themes: [] });
    if (at === "/api/gallery/waiting") return json(r, { themes: theme && !shown.decided ? [{ id: shown.id, ...theme, size: shown.sent.length }] : [] });
    if (at === `/api/gallery/waiting/${shown.id}`) {
      shown.decided = JSON.parse(r.request().postData());
      return json(r, { id: shown.id, state: "public", signature: shown.signature });
    }
    if (at === `/api/gallery/${shown.id}/theme`) return r.fulfill({ status: 200, contentType: "application/zip", body: shown.sent });
    if (at === `/api/gallery/${shown.id}/preview`) return r.fulfill({ status: 200, contentType: "image/png", body: Buffer.from(shown.checked.preview) });
    return r.fulfill({ status: 404, body: "" });
  };
  await maker.route("**/api/**", r => {
    const at = new URL(r.request().url());
    if (at.pathname === "/api/providers") return json(r, ["google"]);
    if (at.pathname === "/api/me") return json(r, { name: "Ada", provider: "google", user: "smoke", owner: true });
    if (at.pathname.startsWith("/api/gallery")) return gallery(r, at.pathname);
    if (at.pathname.startsWith("/api/theme-program")) {
      const body = r.request().postDataBuffer() ?? Buffer.alloc(0);
      rounds.push({ path: at.pathname, kind: r.request().headers()["x-caprice-kind"] ?? "begin", body });
      return json(r, { theme: "0123456789abcdef", source: snow.source, name: "Snow made with Claude", about: snow.about, colours: snow.colours, left: 9 });
    }
    if (at.pathname.startsWith("/api/stories")) return json(r, r.request().method() === "PUT" ? { version: "v1" } : at.search.includes("gone") ? { stories: [], gone: [] } : []);
    return r.fulfill({ status: 404, body: "" });
  });
  await maker.goto(url);
  await started(maker);
  await maker.evaluate(() => new Promise((done, failed) => {
    const r = indexedDB.open("caprice", 1);
    r.onsuccess = () => {
      const t = r.result.transaction("files", "readwrite");
      t.objectStore("files").put({ bytes: new TextEncoder().encode('{"user":"smoke","name":"Ada","provider":"google"}'), modified: 0 }, "/caprice/cloud.json");
      t.oncomplete = done;
      t.onerror = () => failed(t.error);
    };
    r.onerror = () => failed(r.error);
  }));
  await maker.reload();
  await started(maker);
  await click(280, 340, maker);
  await maker.waitForTimeout(3000);
  // Page → Theme → Yours → Make one with Claude…, the request, Make it.
  for (const [x, y] of [[737, 38], [810, 113], [992, 113], [1064, 165]]) await click(x, y, maker);
  await maker.screenshot({ path: `${out}/14-theme-asked.png` });
  await click(640, 425, maker);
  await maker.keyboard.type("Snow falling on igloos by a frozen lake");
  await click(423, 512, maker);
  const t0 = Date.now();
  const compiling = () => maker.workers().some(w => w.url().endsWith("/compiler/compiler.js"));
  while (rounds.length < 2 && Date.now() - t0 < 240000) await maker.waitForTimeout(500);
  const seconds = ((Date.now() - t0) / 1000).toFixed(1);
  // Claude's better version built and played too: then the compiler is ended.
  while (compiling() && Date.now() - t0 < 300000) await maker.waitForTimeout(500);
  await maker.waitForTimeout(2000);
  await maker.screenshot({ path: `${out}/14-theme-made.png` });
  const [begun, looked] = rounds;
  check("a theme is asked of Claude with the writer's words", begun?.path === "/api/theme-program" && begun.body.toString() === "Snow falling on igloos by a frozen lake", begun?.body.toString());
  check("its source is built in the tab, and Claude is shown its pictures once",
    looked?.kind === "frames" && looked.path === "/api/theme-program/0123456789abcdef" && looked.body[0] >= 3 && rounds.length === 2, `${rounds.map(r => r.kind).join(", ")} in ${seconds} s, toolchain ${compiler.id}`);
  check("and the compiler is ended once there is nothing more to build", !compiling(), `${((Date.now() - t0) / 1000).toFixed(1)} s`);
  const before = new Set(await kept(maker));
  await click(421, 635, maker);
  let built;
  const t1 = Date.now();
  while (!built && Date.now() - t1 < 15000) {
    built = (await kept(maker)).find(k => !before.has(k) && k.startsWith("/caprice/themes/") && k.endsWith(".wasm"));
    if (!built) await maker.waitForTimeout(500);
  }
  await maker.waitForTimeout(3000);
  await maker.screenshot({ path: `${out}/14-theme-kept.png` });
  const bytes = built && await maker.evaluate(path => new Promise((done, failed) => {
    const r = indexedDB.open("caprice", 1);
    r.onsuccess = () => {
      const req = r.result.transaction("files", "readonly").objectStore("files").get(path);
      req.onsuccess = () => done(req.result ? Array.from(new Uint8Array(req.result.bytes)) : []);
      req.onerror = () => failed(req.error);
    };
    r.onerror = () => failed(r.error);
  }), built);
  const all = await kept(maker);
  check("kept, it is the very module the toolchain builds anywhere, with its source and a picture, and it plays",
    bytes && Buffer.from(bytes).equals(fs.readFileSync(path.join(here, "../tests/fixtures/snow.wasm")))
      && all.includes(built.replace(/\.wasm$/, ".rs")) && all.includes(built.replace(/\.wasm$/, ".png")) && maker.workers().some(w => w.url().endsWith("/theme-worker.js")),
    built ?? "nothing kept");

  // The gallery's loop (R36, the plan's step 6) in this tab: the theme just kept is sent, and the
  // Worker's own check takes the file the tab wrote. The writer is the owner here, so it waits in
  // their Waiting tab: looked at, it is built again from its source by the compiler in the tab
  // and must be the very module sent, played, pictured, then published. Taken back with "Use", the
  // signature is checked with the key built into Caprice; the stand-in's is not the gallery's, so
  // it is refused and nothing goes on the shelf. (A theme the real key signed, taken all the way
  // onto the shelf, is left open: no test holds the key, and the owner's machine checks that
  // `own::GALLERY_KEY` is its public half. THEME_PROGRAMS_PLAN.md, step 6.)
  const fixtureHash = createHash("sha256").update(fs.readFileSync(path.join(here, "../tests/fixtures/snow.wasm"))).digest("hex");
  // Page → Theme → Yours → the theme's ▸ → Send to the gallery…, then Send it and Done.
  for (const [x, y] of [[737, 38], [810, 113], [992, 113], [1085, 267], [1170, 267]]) await click(x, y, maker);
  await click(468, 531, maker);
  const t2 = Date.now();
  while (!shown.checked && Date.now() - t2 < 15000) await maker.waitForTimeout(250);
  await maker.waitForTimeout(500);
  await maker.screenshot({ path: `${out}/15-gallery-sent.png` });
  const theme = shown.checked?.theme;
  check("a theme sent to the gallery passes the Worker's check, its module the one built",
    theme?.module === fixtureHash && theme.toolchain === compiler.id && theme.name === "Snow made with Claude",
    shown.checked?.why ?? `${theme?.name}, ${theme?.toolchain}, module ${theme?.module.slice(0, 12)}`);
  await click(462, 550, maker);
  // Page → Theme → Gallery…, Waiting, Look at it.
  for (const [x, y] of [[737, 38], [810, 113], [1113, 113], [590, 417], [812, 462]]) await click(x, y, maker);
  const t3 = Date.now();
  while (!compiling() && Date.now() - t3 < 60000) await maker.waitForTimeout(250);
  const rebuilt = compiling();
  while (compiling() && Date.now() - t3 < 180000) await maker.waitForTimeout(500);
  // Played through the preview's story, then Publish it, tried again until the button is there.
  while (!shown.decided && Date.now() - t3 < 240000) {
    await maker.waitForTimeout(2000);
    if (!shown.decided) await click(428, 750, maker);
  }
  await maker.screenshot({ path: `${out}/15-gallery-reviewed.png` });
  check("the owner's tab builds it again from its source, the same byte for byte, and publishes it",
    rebuilt && shown.decided?.publish === true && shown.decided.module === fixtureHash,
    `${JSON.stringify(shown.decided)} in ${((Date.now() - t3) / 1000).toFixed(1)} s`);
  const shelfBefore = await maker.evaluate(() => new Promise(done => {
    const r = indexedDB.open("caprice", 1);
    r.onsuccess = () => {
      const req = r.result.transaction("files", "readonly").objectStore("files").get("/caprice/themes/index.json");
      req.onsuccess = () => done(req.result ? new TextDecoder().decode(req.result.bytes) : "[]");
    };
  }));
  // New, then the theme's Use: fetched, read, its signature checked.
  let fetched = 0;
  maker.on("request", q => { if (new URL(q.url()).pathname === `/api/gallery/${shown.id}/theme`) fetched++; });
  await click(415, 417, maker);
  await maker.waitForTimeout(1000);
  await click(516, 542, maker);
  await maker.waitForTimeout(3000);
  await maker.screenshot({ path: `${out}/15-gallery-use-refused.png` });
  const shelfAfter = await maker.evaluate(() => new Promise(done => {
    const r = indexedDB.open("caprice", 1);
    r.onsuccess = () => {
      const req = r.result.transaction("files", "readonly").objectStore("files").get("/caprice/themes/index.json");
      req.onsuccess = () => done(req.result ? new TextDecoder().decode(req.result.bytes) : "[]");
    };
  }));
  const entries = JSON.parse(shelfAfter);
  check("a theme the gallery did not sign is fetched with Use and not taken onto the shelf",
    fetched === 1 && shelfAfter === shelfBefore && entries.length === 1 && !entries.some(e => e.gallery),
    `fetched ${fetched}, ${entries.length} on the shelf`);
  await context.close();
}

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

// A phone on its side (R32): its screen's short side under 600, the window wide. While the writer
// writes, the keyboard leaves a strip and the docks are away: the page shows where the toolbar was.
const sidewaysContext = await browser.newContext({ viewport: { width: 830, height: 340 }, screen: { width: 915, height: 412 }, hasTouch: true });
const sideways = await sidewaysContext.newPage();
sideways.on("pageerror", e => errors.push(e.message.split("\n")[0]));
await sideways.goto(url);
await started(sideways);
await click(130, 320, sideways);
await sideways.waitForTimeout(2500);
const bright = ([r, g, b]) => r > 220 && g > 220 && b > 220;
const toolbarBefore = await pixelAt(415, 38, sideways);
await sideways.touchscreen.tap(415, 200);
await sideways.waitForTimeout(800);
await sideways.keyboard.type("It was a dark and stormy night.");
await sideways.setViewportSize({ width: 830, height: 150 });
await sideways.waitForTimeout(1500);
const toolbarAfter = await pixelAt(415, 38, sideways);
await sideways.screenshot({ path: `${out}/13-sideways-writing.png` });
check("a phone on its side puts the docks away while writing", !bright(toolbarBefore) && bright(toolbarAfter), `${toolbarBefore} then ${toolbarAfter}`);
await sidewaysContext.close();

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
