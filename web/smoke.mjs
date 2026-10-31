// Opens Caprice in headless Chromium, starts a new story, types into it and takes screenshots:
// node web/smoke.mjs [out dir]. Serve web/dist first (node web/serve.mjs 8080).
import { chromium } from "playwright-core";

const out = process.argv[2] ?? ".";
const url = process.env.CAPRICE_URL ?? "http://localhost:8080/";
const browser = await chromium.launch({ args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", e => errors.push(e.message.split("\n")[0]));
page.on("console", m => { if (m.type() === "error") errors.push(m.text().split("\n")[0]); });

await page.goto(url);
console.log("crossOriginIsolated:", await page.evaluate(() => crossOriginIsolated));
await page.waitForTimeout(4000);
await page.screenshot({ path: `${out}/1-welcome.png` });

// "Open another…" asks for a file in the background: a Web Worker, which chooses nothing yet.
const workers = [];
page.on("worker", w => workers.push(w.url()));
await page.mouse.click(1030, 578);
await page.waitForTimeout(1500);
console.log("workers started:", workers.length);

// The welcome's "Blank page".
await page.mouse.click(280, 340);
await page.waitForTimeout(1500);
await page.screenshot({ path: `${out}/2-clicked.png` });
await page.keyboard.type("Once upon a time, in a city of towers, a dragon carried the title of a story.", { delay: 15 });
await page.waitForTimeout(1500);
await page.screenshot({ path: `${out}/3-typed.png` });

console.log(errors.length ? "errors:\n  " + [...new Set(errors)].slice(0, 8).join("\n  ") : "no errors");
await browser.close();
