# Caprice Web

Caprice in the browser: an independent copy of the desktop word processor (`~/Projects/caprice`,
copied at `5c87d3d`), to be built for WebAssembly. The desktop Caprice is never changed from here.
The plan, step by step, is `WEB_PLAN.md`. The native build stays: it runs the tests, and keeps its
data in its own folder (`caprice-web`), apart from the desktop app's.

A word processor in Rust on egui/eframe 0.36. **Read `ARCHITECTURE.md` before changing
behaviour**: its rules (§4) say what depends on what, and which tests guard each rule.

## Commands

- `cargo build`, `cargo test` (headless, about 13 s), `cargo clippy --all-targets`
- Browser: `web/build.sh` (nightly, std built with atomics; into `web/dist`), then
  `node web/serve.mjs 8080` (it sends the two headers workers need) and open http://localhost:8080/.
  `node web/smoke.mjs <dir>` with the server running: headless Chromium, screenshots, page errors.
  `cd web && npm install` once for it. Trunk and `wasm-bindgen-cli` 0.2.130 come from `cargo install`.
- Deploy, by hand only: `gh workflow run deploy.yml` (or Actions › Deploy): it deploys the build
  `ci.yml` tested for the newest commit on `main`, checks the live site with the smoke test, and
  rolls back if that fails. From this machine, as a last resort: `web/build.sh`, then
  `cd web && npx wrangler deploy` (Cloudflare Workers,
  static files from `web/dist`, headers from `web/_headers`; see `DEPLOY_PLAN.md`). Live at
  https://caprice-web.caprice-web-checks.workers.dev; check it with
  `CAPRICE_URL=<that address> node web/smoke.mjs <dir>`. Never deploy without being asked.
- Stories API (`web/api`, R33): `cd web && node --test api/api.test.mjs` (Miniflare: the Workers runtime, R2 and
  the Durable Object in memory; about 16 s). Its limits are `vars` in `web/wrangler.jsonc`.
- `cargo mutants` for untested code. `/tmp` is a small tmpfs here: set `TMPDIR` to a dir on disk.
- Word output check: `CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`, then
  `soffice --headless --convert-to pdf out.docx`
- Print output check: `CAPRICE_SAMPLE_PDF=out.pdf cargo test sample_pdf`, then `pdftoppm -png out.pdf page`

## Done means

- `cargo test` passes, and clippy has no warnings beyond the ones already there (15 today).
- `web/build.sh` builds, and `web/smoke.mjs` passes (it exits 1 on any failed check or page error).
- `node --test web/api/api.test.mjs` passes when `web/api` or `web/wrangler.jsonc` changed.
- CI (`.github/workflows/ci.yml`) runs all of these on every push, plus `cargo deny check`
  (`deny.toml`: vulnerabilities, licences, sources). A new advisory is looked at, not ignored
  blindly: fix it, or name it in `deny.toml` with the reason it does not reach Caprice.
- The browser build uses the nightly named in `web/build.sh`; change the date there only, with
  `rustup toolchain install <it> -c rust-src -t wasm32-unknown-unknown`.
- Behaviour you can see in the app has an app-level test (`Harness` in `main.rs`). A bug fix has a
  test that fails without the fix: check that it does.
- If a change alters a rule in `ARCHITECTURE.md`, or adds one, update the file in the same commit.

## Keeping the rules growing

`ARCHITECTURE.md` §4 should cover every part of the code, and it grows as the code does. Without
being asked, whenever you:

- find a bug whose cause was an unwritten dependency between parts of the code,
- add a feature that other code must now keep in step with (a cache, a derived value, a new kind
  of page or character, a new file field, a new Word mapping),
- or notice that a rule there is wrong, too narrow, or names code or tests that no longer exist,

then add or adjust the rule in the same change: what holds, why, what to check when touching it,
and the tests that guard it (write the test if there is none). Add a line to §5 if it is a common
kind of change, and to §7 if it records a design decision. Keep the "Rules that bite most often"
list below in step when a rule is important enough. Say in your summary which rules you added or
changed.

## Code style

- **Do not run `cargo fmt`.** The code is not in rustfmt's default style; it would rewrite
  hundreds of places. Match the surrounding code by hand: lines up to about 130 characters,
  short ones preferred.
- Comments are plain English prose that say why, in the voice of the existing ones ("The caret
  goes before or after the block, like a picture."). Module comments (`//!`) explain the idea.
- Test names are sentences of the behaviour (`the_caret_stays_in_the_story_around_long_contents`).
  Test what the user sees (page shown, caret, text) rather than internals.
- Commit messages: `Area: what changed`, then a blank line and bullets or a short paragraph.

## Rules that bite most often

Full list with reasons and tests in `ARCHITECTURE.md` §4.

1. **Text changes go through `Doc::apply`** (via `apply_edit` / `replace_range`). Page settings
   and picture size/rotation are the deliberate exceptions: `full_paginate`, not undoable.
2. **Paragraph formatting is on the terminator**, so an edit can reformat its paragraph from the
   start. Reflow starts at the page where the edited paragraph begins.
3. **Incremental pagination must equal full pagination.** Add a test that compares them for
   anything new whose size depends on text elsewhere.
4. **Pages are measured at page size.** Anything drawn zoomed takes its height from the page-size
   layout × zoom.
5. **A page index is not a caret position.** Show a page with `turn_to` / `go_to_page`, never with
   `set_caret(spans[i].start)`. Contents pages hold no text.
6. **Derived things must not depend on their own results** (the contents' page count comes from
   the titles only).
7. **Notes, scenes, text boxes and page numbering changes are char positions outside the text.**
   Anything that rewrites text without `Doc::apply` must move them. Page numbers come from
   `Doc::page_numbers`, never `i + 1`.
8. **Files hold only stored state.** New fields get `#[serde(default)]`; old formats are converted
   in `DocFile::into_doc`.
   The garden's most pages (`Doc::garden`) are stored: history, and never counted as unsaved.
9. **Every formatting feature needs a Word mapping** in `export.rs`, or a decision that it has none.
10. **Tests never call Claude** unless marked `#[ignore]` and calling `claude::allow_live()`.
11. **Print draws `App::underlay`, `PageLayout::ink` and `App::furniture`**, in that order (boxes behind
    the text; the text and pictures; header, footer, boxes in front). Anything new
    drawn on a page goes into one of them, or it is not printed. None of them moves a page break.
    Tests never reach the system's printers: they set `App::print.spooler`.
    The magic themes' garden, houses, ships, balloon and dragon (`eggs.rs`) are deliberately in none of them, and move no text.
12. **A story with a file saves itself; every story keeps versions by its id.** Anything that
    starts another story sets `story_id` and calls `Keeper::opened`. Leaving still offers to drop
    the changes since saving by hand (`as_saved`). `Harness` turns autosave, versions, the
    word goal's file, the list of stories last opened, the theme's file and the page bar's off: tests write
    nothing outside their own folders.
13. **Colours around the page come from `theme::pal()`**, never literals; the page itself (`PAPER`, `INK`)
    is the same in every theme.
    Pictures (the emblem, the garden, houses, ships and sea, balloon and dragon) keep their own colours.

14. **The desktop and the browser differ only in `platform.rs`.** Background work is
    `platform::spawn` with a `platform::Waker`, never a `Context` or a lock shared with the frame;
    files are `platform::fs` (IndexedDB in the browser); file dialogs are a `platform::Dialog` to
    `ask_path` (in the browser: the shelf, the page's picker, or a download); the time is
    `platform::since_epoch`; requests to Caprice's own server are `platform::send`, through
    `cloud::Server` (a stand-in in tests). What must happen while the page is hidden goes in `App::logic`.
    `clippy.toml` flags the `std` calls that panic or fail in the browser. The browser's
    Content-Security-Policy forbids evaluating code: no library may (`web/csp.mjs`).

## Files

- Tracked: `README.md`, `ARCHITECTURE.md`, `CLAUDE.md`.
- Local, untracked planning notes: `TECHNICAL_DESCRIPTION.md` (older, superseded by
  `ARCHITECTURE.md`), `FORK_REQUIREMENTS.md` (when to fork egui), `CHAPTERS_PLAN.md`, `WEB_PLAN.md`.
