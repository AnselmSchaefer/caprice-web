# Caprice — Architecture

How Caprice is put together, and the rules that hold it together. The rules (§4) are the part to
read before changing anything: each says what depends on what, what to check when touching it, and
which tests guard it. Keep this file true: a change that alters a rule updates it in the same commit.

*Last brought up to date: 2026-11-18 (a phone on its side: its docks, the width filled, the docks away while writing, R32).*

---

## 1. What Caprice is

A desktop word processor in Rust on egui/eframe 0.36. It shows one page at a time on a desk, with
the pages before and after it as piles, and turns pages with an animation ("Book": hinged like a
book; "Paperstack": sliding into the pile). Post-its stick out of the page's edge. Chapters get
titles, drop caps and contents pages; headers and footers in a few looks, page numbers that can
start again or be left off some pages, and text boxes placed anywhere on a page. It saves `.caprice` (JSON), exports `.docx`, and prints, to a
printer or a PDF file, the pages as it shows them. Claude Code
(the CLI) can review a passage drawn around with the pen or selected with the cursor, and paint scenes that show faintly behind the pages, each
for its part of the story, drawing the people of the story's cast the same in every scene. On the
left, Claude can review a chapter (mistakes, style, too thin, too wordy) and propose what could
happen next.

---

## 2. The one idea: one stored flow, everything else derived

```
stored (.caprice)            derived, rebuilt from the stored state, never saved
─────────────────            ───────────────────────────────────────────────────
Flow: text + one Style       ──paginate (page size, scale 1)──▶ Doc::spans (the pages)
      per char                 │                                     │
PageSetup (paper, margins,     ├─ Doc::chapters() ─▶ contents pages  │
  header, footer, drop caps,   ├─ drop caps (which paragraphs, how   │
  contents)                    │   many chars beside the cap)        ▼
images (bytes, width,          └──────────────────────────▶ layout_page(i, zoom) ─▶ drawing
  rotation)                                                 (PageLayout: galleys,     (render, book,
notes (char ranges + text)                                   hit-testing, caret)       editor, notes)
numbering (a char + change)  ──page_numbers()──▶ what {page} and the contents show on each page
text boxes (a char + place)  ──boxes_on(i)──▶ drawn over page i (App::furniture, with the header and footer),
                                               or under its text (App::underlay)
scenes (a char + drawings)   ──scene_at(caret) / page_scene(i)──▶ the drawing behind a page
cast (names, looks, sheets)  ──named_in(what is painted, or just before)──▶ the people sent with a scene
                             export::to_docx(&Doc) ─▶ .docx (Word lays it out again itself)
                             layout_page(i, 1) ─▶ underlay + PageLayout::ink + furniture ─▶ PDF ─▶ a file, or lp (CUPS)
```

- **`Flow`** (`model.rs`) is one `String` plus one `Style` per char (font, size, bold, italic,
  underline, colour, image id, paragraph attributes). Paragraph formatting sits on the paragraph's **terminator** char.
- **Pages are not stored.** `Doc::spans` is a cache: per page, a char/byte range of the flow, whether
  a hard break ends it, and, for a contents page, which part of the contents it shows.
- **Notes** are char ranges kept beside the text, not in it; **scenes** and **text boxes** are chars likewise.
- **Everything shown is recomputed** from these each frame (egui is immediate mode). Caches that do
  exist key on `Doc::version` (search matches, the scene's "read at").

---

## 3. Modules

| Module | Owns |
|---|---|
| `model.rs` | `Doc`, `Flow`, `Style`, `ParaAttrs`, `PageSetup`, `Span`, `Note`, `ImageData`, `Scene`, `Character`, `Look`; special chars; char↔byte, `page_of`, `para_start`, `term_from` |
| `edit.rs` | `Edit` (replace / restyle), `Doc::apply`, undo/redo `History`, moving notes with edits (`rebase_notes`) |
| `paginate.rs` | `full_paginate`, `paginate_after` (incremental), `next_span`: where pages break, at page size |
| `layout.rs` | One egui `LayoutJob` per paragraph piece; `layout_page`; `PageLayout` (hit, caret, rows, selection, links); `true_to_scale`, `tight_highlights` |
| `dropcap.rs` | Which paragraphs get a drop cap, and the three-piece layout (cap, lines beside it, rest) |
| `contents.rs` | `Doc::chapters`, splitting the contents over pages, laying out one page of them, migrating the old contents char |
| `editor.rs` | Caret, selection, typing, deleting, clipboard, keys, mouse, formatting commands, `set_setup`, `turn_to`, the editable page surface |
| `main.rs` | `App` state, `main()`, the frame loop, page position animation, `go_to_page`, the app-level tests and their `Harness` |
| `view.rs` | Camera: fit (to the width on a phone), zoom, panning, two-finger swipe through pages, following the caret (R32); one finger is the editor's `Finger` |
| `render.rs` | Paperstack look: paper, piles, static pages, sliding pages |
| `book.rs` | Book look: pages turning on their left edge |
| `notes.rs` | Post-its: placement, pads, drawing, editing |
| `images.rs` | Pictures: inserting, resizing, rotating, moving, textures |
| `search.rs` | Find and replace bar, matches (case, whole words) |
| `words.rs` | The Document info window: dates, file, counts (words, chapter, selection…), today's words and the daily goal, kept in the data folder |
| `ui.rs` | Top toolbar (formatting, Page and Insert menus, the text box gallery), bottom page bar and scrollbar |
| `bands.rs` | Headers and footers: their looks drawn as `Ink`, fields filled in per page, the gallery tiles, accent colours |
| `numbers.rs` | Page numbering: changes pinned to a char (start again, count on, none; figures or roman), the number of each page, setting one from a page |
| `textbox.rs` | Text boxes: drawn as `Ink`, inserted, written in, moved, resized, pinned to a char, in front of the text or behind it; their selection |
| `fileio.rs` | `.caprice` format (`DocFile`), open/save/export commands, unsaved-changes detection |
| `export.rs` | Hand-written Office Open XML for `.docx` |
| `emblem.rs` | The emblem beside the name on the welcome page: the app icon's tile and page, a pencil drawing its loop over and over |
| `leave.rs` | The question before leaving a story with changes (save or not; keep or drop): its first page, what is at stake, that a version is kept; the name suggested for a story never saved |
| `again.rs` | In the browser: a page that lost its drawing (the GPU, taken by a phone in the background) loads again, back at the story, caret and device it had (R31) |
| `versions.rs` | Autosaving a story with a file; versions of every story kept in the data folder; the Versions window |
| `print.rs` | The Print window; pages drawn into a PDF (krilla) from `PageLayout::ink`; printers and jobs through CUPS (`Spooler`) |
| `claude.rs` | Pen loop or cursor selection → passage → `claude -p` → streamed answer panel, a conversation; story notes |
| `backdrop.rs` | Scenes Claude paints (SVG), pinned to the story, faintly behind the pages; the Scene menu and list |
| `motion.rs` | Scenes that move a little: the parts Claude marks by id (drift, wing, sway, smoke, flicker, shimmer), rendered apart from the still picture and moved every frame; and the kinds a theme of one's own may use besides (float, twinkle, pulse, wander, spin, and areas of snow, rain, fireflies, leaves, bubbles or embers that Caprice fills) |
| `own.rs` | Themes of the writer's own (R35): one SVG drawing read into its desk, pieces and carrier (`Pack`), checked on import with the reasons in plain words (`check`), its palette taken from its picture (`palette_of`); the shelf of them in the data folder (`Shelf`); importing and exporting drawings (never a program), deleting, the template, the guide and the import's report (`Workshop`) |
| `bundle.rs` | A theme that is a program as it travels (R36): a `.caprice-theme`, a zip of `manifest.json` (name, words, author, palette, the world, the toolchain), `theme.wasm`, `theme.rs` and `preview.png`; read and checked with the reasons in words (`open`; a file with a source and no module, or a source alone, `from_source`, to be built), written (`write`), the same bytes for the same parts; never a file a writer picks or saves |
| `making.rs` | Making a theme with Claude (R36): the "Make a theme" window, its steps, time and lost connections told the writer without naming Claude; the request to `/api/theme-program`, the source built in the tab, checked and pictured, the compiler's words and a program that will not play sent back to Claude unseen, its pictures shown to Claude once, the last version that worked kept until the writer keeps it or asks for a change |
| `gallery.rs` | The gallery of theme programs (R33, R36): the Gallery window (New, Sent by you, and for the owner Waiting), each theme's picture, "Use" checking the gallery's signature before the shelf keeps it; sending a theme made with Claude; the owner's look, built again the same in the tab and played, and their decision |
| `cast.rs` | The cast: people found by name in what is painted, their looks and model sheets in the scene's prompt; the Cast window and its example |
| `start.rs` | The welcome page when Caprice opens without a file: kinds of new story (`Kind`), the stories last opened with what they hold, the sheet flying to the desk; the list of stories last opened, kept in the data folder |
| `peek.rs` | The cast peeking out from behind the page: the puppet asked of Claude, cut into parts, who peeks when, where they look, the peek drawn |
| `scratchpad.rs` | The scratchpad: plain text saved with the document, in a window on the right |
| `review.rs` | The Review window (upper left): a chapter or the selection read for mistakes, style, detail, wordiness; findings found again by their quote |
| `ideas.rs` | The What next window (lower left): ways the story could go on from the caret, kept in the scratchpad |
| `fonts.rs` | Installed fonts loaded on demand, a missing one drawn with a twin of the same widths |
| `theme.rs` | The themes (`Theme`, `Palette`): colours and egui visuals around the page, the palette in use (`pal`), fading from one to the next, the choice kept in the data folder (a theme's name, or one of the writer's own by its id); the page's own `PAPER` and `INK` |
| `eggs.rs` | What is hidden in the magic themes: in the Magic Garden a plant a chapter growing on the desk and a balloon flying in each new chapter title; in the Magic City a street of houses a chapter, a floor a page, and a dragon flying a new title in held in its feet; in the Magic Sea a ship a chapter, built in a yard a part a page and launched into the bay when the next chapter begins; in the Magic Forest (`eggs/forest.rs`) an old forest at night with a swarm of lanterns, a lantern a page, each chapter's swarm becoming a big lantern on a branch near the page; in the Magic Reef (`eggs/reef.rs`) a sunlit sea with a coral a chapter, a branch a page, spawning and taking in a shoal of fish when the next chapter begins, and an octopus swimming down from above with each new title; in the Magic Snow (`eggs/snow.rs`) a frozen lake at the foot of a snowy hillside and an igloo a chapter spread about the hillside, a brick a page cut out of the lake and floated onto it, the igloo finished and lit inside when the next chapter begins; in the Magic Vineyard (`eggs/vineyard.rs`) a vineyard on rolling hills on a golden afternoon, a stretch of vines a chapter, a bunch of grapes a page, the stretch before turning gold and red and dropping its leaves when the next chapter begins, and now and then a murmuration of starlings; the country it lies in is `eggs/country.rs`; and a theme of the writer's own (`eggs/own.rs`, R35): its picture covering the desk and moving, a piece a chapter, a stage a page, finished when the next chapter begins, its carrier bringing new titles; or one that is a program (`eggs/program.rs`, R36) |
| `eggs/program.rs` | Themes that are programs (R36): the check on every load, the input and events written as the canonical ABI lays them out, the frame's words read into egui's shapes (at most 450 000 points), drawn on the desk zoomed about the theme's ground and in the window unzoomed; what the program is told; its carrier |
| `eggs/preview.rs` | A theme program's pictures, made off screen (R36): played through a short story (begun, three chapters written, a chapter finishing, a title carried, zoomed out) on the desk laid out as at the fitting zoom, tessellated by egui and filled in triangle by triangle; its points counted on a phone's desk and a computer's; whether its own sky stops short of the window's edges zoomed out; the gallery's picture of a theme that came with none |
| `platform/compiler.rs` | Where a theme program is built from its source (R36): in the browser rustc and wasm-ld as WebAssembly in a worker of their own (`web/compiler/`), ended when dropped; on the desktop not yet; under `cargo test` a stand-in (the sources of `themes/` build to their fixtures, anything else to what a test says) |
| `platform/program.rs` | Where a theme program runs (R36): in the browser a worker of its own (`web/theme-worker.js`), ended when it does not answer; on the desktop not at all; under `cargo test` none, each module standing for its theme's source played natively (`played`: the input read out of Caprice's bytes, the output made into the same words) or for what a test says (`stand_in`) |
| `theme-sdk/` | What theme programs are written against (R36): the WIT world `caprice:theme@1.0.0`, its bindings made ahead of time with a runtime of its own, shapes (and Caprice's own soft lights, ovals and clean fills), colours, `Seeds`, `Tended`, easing, the `theme!` macro; no dependencies |
| `themes/` | Theme programs Caprice carries: `fireflies` (an example, and the tests' theme), `echo` (draws back what it is told, for the layouts' tests), `snow` and `vineyard` (the Magic Snow and the Magic Vineyard ported, shape for shape, R36); `build.sh` builds them into `tests/fixtures/` as the writer's tab does; `played` builds them for the machine, for the tests to play in place of their modules |
| `web/compiler/` | The theme compiler's worker (R36): the investigation's toolchain glue (its VFS, WASI, process runner, orchestrator) made JavaScript, loading the toolchain in pieces into Cache Storage (`toolchain.js`) and building one theme at a time (`compiler.js`) |
| `toolchain/` | Packing the toolchain Caprice serves (`pack.mjs`, run by `web/build.sh`): the investigation's rustc and wasm-ld, rustup's std for `wasm32-unknown-unknown` without what no theme reaches, the SDK as one rlib, the flags, cut into brotli pieces under 25 MiB named by their hash; the brotli decoder the tab uses (`unbrotli/`) |
| `platform.rs` | What the desktop and the browser do differently: background work (`spawn`, `Waker`, `keep_awake`), the time and time zone, the run's id, the data folder, file dialogs (`Dialog`), uploads and downloads, requests to Caprice's server (`send`, `go_to`) (R31, R33) |
| `platform/fs.rs` | Files: the system's on the desktop; in the browser, kept in memory by path and mirrored to IndexedDB (R31) |
| `platform/worker.rs` | In the browser, a Web Worker for each piece of background work, started from `web/worker.js` with nothing evaluated (R31) |
| `shelf.rs` | Stories kept in the browser: saving asks for a name, opening lists them (with an upload), each answering the waiting file dialog with a path (R31) |
| `cloud.rs` | Stories online: signing in and out, what to do with each story (`decide`), one request at a time, `cloud.json`, the Stories online window; a stand-in server and story rooms for tests (R33, R34) |
| `live.rs` | A story live on several devices: the room's connection, the pen, batches of edits out, following them in, the hash check and the whole story, the bar and Transfer here (R34) |
| `web/api/worker.js` | The Worker's own requests, `/api/*` only: signing in with Google or GitHub, the session cookie, stories online in R2, scenes Claude paints for the browser (R33) |
| `web/api/room.js` | The Durable Object `StoryRoom`, one per story of each writer: the devices it is open on, the pen, batches passed on (R34) |
| `web/api/failure.js` | What a failed request answers: Cloudflare's daily limit said plainly, with when it lifts; any other fault, "Something went wrong" (R34) |
| `web/api/ledger.js` | The Durable Object `Ledger`: who has signed up, the current version and history of each story online, the day's scenes, every limit (R33) |
| `web/api/tally.js` | The day's scenes counted in R2 as well, with conditional writes, for when the Ledger cannot be reached (R33, R34) |
| `web/api/gallery.js` | The gallery's requests (R33, R36): a theme file sent, checked (`theme-file.js`), queued; the list, each writer's own, the owner's queue and decision, signed with `GALLERY_SIGNING_KEY`; its files read from R2 |
| `web/api/theme-file.js` | A theme file as the gallery takes it in: unzipped within limits, its manifest, picture and module's sections checked (`wasm-check.js`), the module and source hashed; no text imported, so `smoke.mjs` checks what a real tab sends with it |
| `web/api/wasm-check.js` | R36's check of a theme's module, as the Worker makes it before anything is kept: imports, exports, sizes |

---

## 4. Rules

Each rule: what holds, why, what to check when you touch it, and the tests that guard it.

### R1 — Every change to the text goes through `Doc::apply`

`App::apply_edit` / `replace_range` build an `Edit`, `Doc::apply` applies it (moving notes, recording
undo, grouping typing, bumping `version`), then `paginate_after` repaginates and `set_caret` shows
the caret's page.

- **Exceptions, on purpose:** page settings (`set_setup`) and picture size/rotation (`images.rs`) are
  not text. They change the `Doc` directly, bump `version`, run `full_paginate`, and are **not
  undoable**. A new non-text setting should follow the same pattern.
- **If you write to `flow.text`/`flow.styles` anywhere else** (file migration is the one place), you
  must also move notes yourself and repaginate.
- Tests: `undo_redo_restore_text_and_styles`, `typing_groups_into_one_undo_step_until_a_pause`,
  `grouped_edits_undo_together_and_each_group_is_kept`, `notes_follow_edits`.

### R2 — A paragraph is formatted by its terminator, so an edit reaches back to its start

`\n` (or `\u{c}`, a hard page break) ends a paragraph and carries its `ParaAttrs` (alignment, line
spacing, list, chapter title). Changing or moving that terminator reformats the **whole** paragraph,
including the part on earlier pages.

- `paginate_after` therefore starts at the page where the edited paragraph **begins**, not where
  the edit is. Drop caps reach further still: whether a paragraph gets one depends on the
  paragraphs before it (`drop_cap_reach`).
- New paragraph-level attribute: put it in `ParaAttrs`, read it from the terminator's style.
- **Enter puts the caret before the old paragraph's terminator**, which keeps the look it had.
  Typing must not take its style from there: after text is typed (`replace_raw`) the typing style
  stays as it was, so a colour, size or bold chosen to type with goes on to the next line. Only
  moving the caret or deleting takes the style from the text (`typing_follows_caret`).
- Tests: `a_new_chapter_far_on_reflows_like_from_scratch`,
  `editing_a_title_repaginates_the_chapter_after_it_like_from_scratch`,
  `a_colour_chosen_for_typing_goes_on_to_the_next_line`.

### R3 — Incremental pagination must equal full pagination

`paginate_after` keeps the pages before the edit and stops once a new page starts where an old one
did, shifting the rest. Anything that can change a page's content from far away breaks that
shortcut unless handled: today the contents (rebuilt on every edit, R6) and drop caps (R2; the
paragraphs beside a cap are on its page, so the reflow from there covers them, R4).

- **If you add something whose size depends on text elsewhere,** make `paginate_after` account for
  it, and add an "incremental equals full" test for it.
- Tests: `incremental_matches_full`, `incremental_reuses_later_pages_correctly`,
  `incremental_handles_edits_that_remove_page_breaks`,
  `chapters_added_or_taken_away_change_the_contents_pages_like_from_scratch`.

### R4 — Pages are worked out at page size; drawing at any zoom must agree

Pagination lays out at scale 1 (page points). Drawing lays out again at the zoom scale, and egui
rounds line heights to whole pixels and can **wrap a word differently**. So:

- `true_to_scale` gives zoomed lines their page-size height × zoom, **per line**, not per paragraph.
  (Per paragraph spread or squeezed lines whenever the zoomed text wrapped into a different number
  of lines.)
- Drop caps, the contents and pictures are measured at page size and scaled.
- **A drop cap fits the lines set, top to tail** (`Doc::drop_cap`): its top is where the first
  line's letters' tops are and its baseline the last line's; a letter that hangs below its
  baseline (j, p, q, y) is made smaller instead, so its tail ends with the last line. It is
  measured at a large size (`REF`), since at text size its ink is a few whole pixels. Should a tail
  still reach into the next line, that line goes beside it too; the rest of the paragraph never
  starts under the tail with a line left empty beside it (it did). Pagination and drawing both
  take the block from `DropCap`. With nothing after the letter yet, its lines are measured with
  a stand-in letter in its style (an empty line has no baseline: the cap sat lower, then jumped up
  as the first letter was typed). Tests: `the_cap_sinks_as_many_lines_deep_as_set`,
  `a_cap_hanging_below_its_line_is_made_smaller_to_keep_its_tail_in_the_lines_set`,
  `the_cap_stays_put_when_the_first_letter_after_it_is_typed`,
  `the_drop_cap_stays_put_while_the_first_words_are_typed`.
- **A paragraph that ends beside its cap leaves the room there to the next ones** (`Float`): they
  go on beside the letter, narrower, until it ends (`DropCap::bottom`); one that runs past it goes
  on at full width below. Enter beside a cap used to send the new line below the whole letter.
  How much of a paragraph goes beside is worked out at page size (`Doc::beside_cap`) by
  `next_span` and `layout_page` alike, which carry the room from one paragraph to the next on the
  page; anything that changes how those two walk a page keeps them in step. A picture or another
  cap ends the room. Clicking right of the cap finds the piece beside it at that height (`hit`).
  Tests: `enter_beside_a_drop_cap_goes_on_to_the_next_line_beside_it` (with full pagination
  compared).
- **If you add anything drawn on a page,** measure it at page size and multiply by the scale; never
  let its height come from the zoomed layout.
- Known gap: when zoomed text wraps into one line more than at page size, its paragraph is a line
  taller on screen than pagination assumed.
- Tests: `text_fills_the_page_the_same_at_every_zoom`,
  `lines_keep_their_spacing_when_zoomed_text_wraps_differently`,
  `a_chapter_fills_the_page_the_same_at_every_zoom`, `the_contents_take_the_same_room_at_every_zoom`.

### R5 — A page index is not a caret position

`Doc::spans[i]` is page `i`. Most pages hold a stretch of the story, but the **contents pages hold
none of it** (empty span at the story's start, `Span::contents = Some(part)`). `page_of(c)` never
returns them, so the caret is always in the story.

- **To show a page, use `turn_to(i)`** (or `go_to_page`), never `set_caret(spans[i].start)`: the
  start of a contents page is on another page. Swipes, the scrollbar and Page Up/Down use
  `turn_to`; a contents link moves the caret to its chapter's page (a story page), like a link.
- **Turning pages leaves the caret where it is**, as scrolling does in other word processors: the
  shown page (`target`) and the caret's page often differ. Only Shift+Page Down/Up (extending the
  selection) takes the caret along. Anything that acts at the caret must not assume it is on the
  page shown; what shows on the page shown without the caret (the scene, R14) comes from the page.
- **`set_caret` shows the caret's page**, so typing, arrow keys and edits bring it back.
- **A reflow that is not an edit keeps the page being read**, not the caret's: page settings
  (`set_setup`) show the page the shown page's first char goes to, at once, and only keep the
  caret in range, without `set_caret` (which turned to the caret's page: with the caret at the
  end, switching A4 to Letter showed the last page). Adding the contents turns to them.
- **A click only places the caret if the spot it hits is on the clicked page** (`editor_surface`).
- Anything per page that uses `spans[i].start` (selection, notes, the pen) must cope with a page
  that holds no text.
- **A selection dragged well past the top or bottom of the writing area turns the page**
  (`drag_past_edge`: `EDGE_PAST` page points into the margin, times the zoom; nearer, it only
  selects to the first or last line),
  the caret going to a story position on the next or previous page, so `set_caret` shows it. It
  never turns onto or from the contents. The drag is the app's own (`App::sel_drag`) and survives
  the turn, when the editor surface is not drawn and egui may drop its own. Held there, it turns
  again after `EDGE_REPEAT` on the settled page, not every frame.
- Tests: `page_keys_go_through_every_page_of_long_contents`,
  `swiping_goes_through_every_page_of_long_contents`,
  `the_scrollbar_reaches_every_page_of_long_contents`,
  `clicks_on_a_later_page_of_the_contents_stay_there_or_follow_a_line`,
  `the_caret_stays_in_the_story_around_long_contents`,
  `turning_pages_leaves_the_caret_where_it_is_until_typing_or_a_click`,
  `changing_the_paper_keeps_the_page_being_read_in_view`,
  `an_empty_page_between_two_breaks_exists_and_can_hold_the_caret`,
  `dragging_a_selection_past_the_bottom_of_the_page_goes_on_to_the_next_pages`,
  `dragging_a_selection_past_the_top_of_the_page_goes_back_to_the_pages_before`,
  `dragging_a_selection_past_the_top_of_the_story_does_not_turn_onto_the_contents`.

### R6 — Derived things must not depend on their own results

The contents list page numbers, and their page count shifts every page after them. So **how many
pages the contents take depends only on the chapter titles**, never on page numbers; the numbers
are filled in when a page is drawn (`chapters(true)` in `layout_page`, `chapters(false)` while
paginating).

- Same for anything new that shows page numbers or other layout results: decide its size from
  inputs that layout does not change.
- Tests: `long_contents_go_on_over_as_many_pages_as_they_take`,
  `the_contents_list_each_chapter_and_lead_to_its_page`.

### R7 — Notes and scenes are char positions outside the text

- `rebase_notes`, `rebase_scenes`, `rebase_boxes` and `rebase_number_marks` move them with every `Edit`. A text box pin
  moves like a scene pin. A scene pin is a point: text
  typed right at it goes after it (it still starts its paragraph or sentence), and deleted around, it stays
  where the text was. A rewrite of the text that bypasses `Doc::apply`
  (file migration) must move them too (`take_out_contents_chars` returns what it removed, so
  `into_doc` can).
- Neither changes where pages break.
- **Anything new pinned to the text** goes the same way: moved in `apply_raw`, and in `into_doc`.
- Tests: `notes_follow_edits`, `replacing_across_a_notes_edges`,
  `text_boxes_stay_with_their_char_and_where_it_was_once_it_is_deleted`,
  `deleting_a_notes_text_leaves_a_point_note`, `post_its_do_not_change_where_pages_break`,
  `scenes_stay_with_their_paragraph_through_edits_and_saving`,
  `files_with_the_old_contents_char_open_with_the_setting_and_post_its_and_scenes_in_place`.

### R8 — Special characters in the flow

| Char | Meaning | Rules |
|---|---|---|
| `\n` | paragraph end | carries `ParaAttrs` (R2) |
| `\u{c}` (`PAGE_BREAK`) | paragraph end + hard page break | dropped from typed/pasted text; `editor.rs` inserts it |
| `U+FFFC` (`IMAGE_CHAR`) | a picture, `Style::image` = its id | in a paragraph of its own |
| `U+E000` (`CONTENTS_CHAR`) | **legacy only**: older files' contents | taken out on open (sets `PageSetup::contents`), filtered from pasted text |

- The final char of the flow is always a paragraph mark (`ensure_final_mark`); the caret's last
  position is just before it (`max_caret`).
- Text leaving the app (Claude, the scene, plain clipboard, chapter titles) leaves out `IMAGE_CHAR`
  and turns page breaks into line breaks.

### R9 — Files: only stored state, old files keep opening

- `.caprice` is JSON, version 2 (`DocFile`). Version 1 still loads.
- **New fields get `#[serde(default)]`** (`PageSetup` has it on the whole struct), so older files
  load and older versions of Caprice ignore what they don't know.
- **Never save derived data** (pages, contents, drop caps).
- Converting old formats happens in `DocFile::into_doc`, and must move notes and scenes (R7).
  The one exception is the scene of older files (`DocFile::scene.svg`), which needs the app: it
  becomes a scene pinned at 0 in `App::open_scene`. `scene` now only holds the description and
  whether pictures show.
- Unsaved changes = a hash of the serialized `DocFile`, so anything saved counts automatically.
  The garden is not in the file (R30), so measuring it never makes a story unsaved.
- `PageSetup::page_numbers` is read only from files from before footers: `into_doc` makes it a
  plain footer of the page number (`Band::page_numbers`) and it is never written again. (Older
  Caprice opening a newer file shows no page numbers.)
- Page numbering: `DocFile::numbers` (the changes pinned in the story, moved by `into_doc` like
  the boxes) and `PageSetup::contents_numbers`, both left out of the file while unused (R26).
- `DocFile::id` names the story for its versions (R22). Older files have none: they get one on
  opening, which counts as saved (it is written with the next save). `DocFile::created` likewise:
  older files take the file's own date (created, else modified) on opening, and keep it once saved.
- A look's peek (`Look::peek`, an SVG) and whether the cast peeks (`Doc::peeks`, on unless turned
  off: older files peek) are left out of the file while unused (R28).
- The garden (`Doc::garden`, R30) is not in the file: a story opened is counted from its pages.
  Older files hold it as `garden`; it is no longer read, and dropped with the next save.
- Tests: `roundtrip_keeps_text_styles_breaks_and_setup`, `version_1_files_still_load_as_separate_pages`,
  `a_peek_and_whether_the_cast_peeks_are_saved_and_older_files_peek`,
  `files_from_before_footers_keep_their_page_numbers_as_a_footer`,
  `chapter_titles_are_saved_and_older_files_have_none`,
  `the_contents_setting_is_saved_opened_and_exported_through_the_app`,
  `an_older_files_one_scene_opens_pinned_to_the_start_of_the_story`,
  `story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections`,
  `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`,
  `a_scene_that_names_someone_in_the_cast_is_painted_with_their_look_and_model_sheet`,
  `a_story_opened_again_shows_its_chapters_and_pages_as_they_are_not_as_they_once_were`,
  `a_story_begun_is_not_unsaved_for_its_garden`.

### R10 — Word export maps the model, not the pixels

Word lays the document out again by its own rules, so `export.rs` maps Caprice's model onto Word's
(table in its module comment): chapter titles → Heading 1, drop caps → Word drop caps, contents →
a `TOC` field (Caprice's page numbers are only its cached result, and Word updates them on opening).

- **A new formatting feature needs its Word mapping**, or a decision that it has none, in the same
  change. Check the output with LibreOffice (`CAPRICE_SAMPLE_OUT=out.docx cargo test sample_docx`,
  then `soffice --headless --convert-to pdf out.docx`). Not yet checked in Microsoft Word itself.
- Scenes are not exported (decided: they are a writing aid, too faint to print).
- Story notes, the scratchpad and the cast are not exported (decided: notes to Claude and to
  oneself, not part of the book). Nor is the garden (R30): it is the app's, not the book's.
- Headers and footers are Word's header and footer parts, their fields Word's (`PAGE`,
  `STYLEREF "Heading 1"`), their looks paragraph borders, shading, a "❦" and an
  ellipse shape (module comment of `export.rs`); Facing pages are odd and even headers
  (`evenAndOddHeaders` in `settings.xml`). Known difference: before the first chapter, Caprice
  shows no chapter, and Word's `STYLEREF` shows the first one after.
- Text boxes are text box shapes (`wps:wsp`) anchored in the paragraph of their char, at their
  place on the page, `wrapNone` (in front of the text, as in Caprice), growing to their text
  (`spAutoFit`). Quote and sidebar lines are paragraph borders inside the box.
- Page numbering is sections (R26): each change starts one at its paragraph, its `sectPr` in the
  last paragraph before (`end_section`), starting a page where a hard break was (the break is
  taken by it) and `continuous` elsewhere. `pgNumType` holds the start and `lowerRoman`. A section
  without numbers refers to an empty header or footer part (`header3.xml`, `footer3.xml`) in
  place of one showing the page number: with no reference it would carry on the one before. The
  contents end their section in an empty paragraph of their own, as Word rewrites the `TOC`
  field's paragraphs. Known difference: Word changes the numbering of a continuous section from
  the next page that begins in it, Caprice from the page its char is on.
- Character formatting maps to run properties (`run_props`): bold, italic (`<w:i/>`), underline,
  and colour (`<w:color>`; none is Word's automatic colour, as it is Caprice's usual ink).
- The sample checks (`sample_docx`, `sample_pdf`) lay the sample out in a context that has its
  font loaded; laid out in a fresh `with_ctx` it panicked once any other test had loaded it.
- Tests: in `export.rs`, plus `notes_become_comments_over_the_same_text`,
  `italic_and_coloured_text_is_drawn_saved_and_exported`.

### R11 — egui workarounds depend on egui internals

| Problem in egui 0.36 | Workaround | Where |
|---|---|---|
| Zoomed line heights rounded to pixels | Rescale rows to page-size height × zoom, per line | `layout.rs::true_to_scale` |
| Text background fills the whole line height | Trim background quads to the font height | `layout.rs::tight_highlights` |
| The caret fills the whole line height, reaching far below the letters with line spacing above 1 | Divide its height by the spacing (the letters sit at the row's top): `with_airy_lines_the_caret_is_as_tall_as_the_letters_beside_it` | `layout.rs::caret_rect` |
| No text flowing around a box | Drop cap paragraph as three pieces; text boxes lie in front of the text | `dropcap.rs`, `textbox.rs` |
| Only convex shapes are filled | Fleurons and circles built of convex pieces (`leaf`, `circle`); the emblem's tile a fan of triangles, its pencil quads | `bands.rs`, `emblem.rs` |
| A field's galley is shown from its left, whatever its alignment | Box text laid out left, rows moved within the width (`box_galley`) | `textbox.rs` |
| `TextEdit` caret height from its own font | Pass the post-it font | `notes.rs` |
| No bending of text | Tessellate galleys, move vertices | `render.rs`, `book.rs`, `notes.rs` |
| Modifiers unreliable on Hyprland | Track Ctrl from key events | `view.rs` |
| In a window that sizes itself (`Modal`, `Area`), `with_layout` takes all the height left: a row centred in it, or anything after it, makes the window grow every frame | Give such rows their height with `allocate_ui_with_layout`. `keep_or_drop_keeps_its_size_with_the_hint_under_its_buttons` | `leave.rs::leave_dialog` |
| The UI font has no symbols like ✕, → or ⋯ (drawn as an empty box) | Draw icons with lines, labelled for tests; in words, › stands for an arrow ("File › Versions"). A ⋯ standing alone (a theme's menu in Yours, the backups in Versions) is in the monospace face, Hack, which has it (`RichText::monospace`). `Harness::missing_glyphs` lists what was painted without its letters: the tests of the page numbers menu, the leaving dialog, the review and Yours check it is empty. egui's `has_glyph` says no for every letter of the face holding its empty box, which in the monospace family is all of Hack: the helper tells those by their drawing (`missing_glyph`) | `ui.rs::paint_icon`, `search.rs::bar_button`, the ✕ of a page numbering change in `ui.rs` |
| `ctx.fonts_mut` inside `ctx.fonts_mut` hangs (the context's lock is taken twice) | Build a job (`paragraph_job`, `run_format`, which ask the fonts for a row's height) before taking the fonts to lay it out. Drop caps with line spacing hung: `a_new_novel_begins_on_book_pages_at_chapter_one_and_a_journal_at_today` | `dropcap.rs::drop_cap` |
| Justified rows are lined up by their first and last letters: whitespace in front goes out to the left, a tab or space at a paragraph's end gives the caret no room | `justified_edges` moves the glyphs (and their quads) back: the whitespace in front keeps its room, a stretched row still ends at the margin, the last row is as wide as all its chars. Underlines and highlights of such a row are not moved. `justified_text_keeps_a_tab_before_it_and_after_it` | `layout.rs::justified_edges` |
| Tab moves egui's keyboard focus to the next widget, even when the page took the key | The editor cancels the move when it types a tab (`move_focus(FocusDirection::None)`); the caret is hidden while a widget has the keyboard. `a_tab_typed_beside_a_drop_cap_moves_the_caret_at_once` | `editor.rs` (keys) |
| `set_fonts` takes effect the next frame; a family named before then panics | `FontBook::ensure` in mid-frame with `immediate: false`, and draw with `family_for` (the usual font until it is there). `fonts::LOADED` is one for the process, so tests running alongside share it: a test that loads a font for the first time is the only one to | `fonts.rs`, `start.rs::welcome_page` |

Re-check every one after upgrading egui.

### R12 — Tests never call Claude

`claude::run_claude` refuses under `cfg(test)` unless a test calls `allow_live()`; those tests are
`#[ignore]` (`live_*`). Calls run `claude -p` with no tools, user settings only, no MCP, no saved
session, in the temp dir.

### R13 — Asking Claude: circling with the pen, or selecting with the cursor

With Claude on (`App::pen`), `App::picking` says how a passage is picked, chosen in the toolbar's
Claude menu:

- **Circle:** `pen_surface` takes the drags and draws loops; the editor surface is drawn but not
  interactive (`circling()`), so drags never select. A loop stays on one page.
- **Cursor:** the editor works as always, so a selection can be dragged over several pages (R5).
  Letting go of a drag that left a selection (`sel_drag`, a double or triple click too) offers the
  commands for it (`offer_commands`), a `Lasso` with no points, only `caught`.
- Either way the commands come from the one `Lasso` menu; anything new there must work for both.
- **The menu's text fields take the typing, never the passage.** The passage is still selected
  while the menu is open, so a keystroke that reaches the editor replaces it. Whichever field the
  menu shows takes focus the first frame it shows (`menu_shown`). A button that swaps the menu for
  another step (Connect the scenes → `bridging`) must stop drawing the menu that frame (`return`),
  or a field further down takes the focus and is gone the next frame.
- **Claude is sent only the marked passage**, never other text of the document: the writer
  decides what Claude sees by what they circle or select. The Review and What next windows (R19)
  are the one exception, and keep its point: their button names what it sends before it is sent. Added to it are only what the writer
  wrote for Claude: the note typed for Connect the scenes, and the story notes (`Doc::story_notes`)
  with every command about the story (`knows_story`), not with corrections (Fix, Grammar). It is
  built in one place, `App::prompt`, and kept in `Answer::first`. Connecting the scenes
  (`Command::Bridge`) expects the passage to hold both scenes, and Claude finds the gap in it.
  (It once also sent 12,000 chars of story on each side, which for most stories was all of it.)
- **An answer is a conversation.** `claude -p` keeps nothing between calls (we pass
  `--no-session-persistence`), so every reply sends `Answer::sent()`: the first prompt again,
  then each earlier answer and reply. A new kind of follow-up adds its instruction there.
  `follow_up` stops the answer being written before starting the next.
- **An answer that can take the passage's place** is `Answer::rewrites()`: Fix and Grammar, or a
  draft (`draft`, from Draft it: the passage again with the gap filled, shown as a diff; a reply
  to a draft asks for it revised). Claude sometimes answers a draft with only the new part, which
  would replace both scenes: the request names the passage's first and last words, and Apply is
  off unless the draft keeps them (`keeps_ends`). Apply and
  Pin as note both need the passage unchanged since it was asked about (`unchanged`): the range
  is not moved with edits. A pinned answer is a post-it like any other (R7, saved, a Word comment).
- Tests: `claude_is_sent_only_the_marked_text_whatever_it_is_asked` (a new command or follow-up
  goes into it), `a_loop_drawn_with_the_pen_selects_the_text_inside_it`,
  `circling_with_the_pen_takes_drags_away_from_the_cursor`,
  `asking_claude_by_cursor_offers_the_commands_for_a_selection_over_two_pages`,
  `connecting_two_scenes_sends_the_marked_scenes_with_the_writers_note`,
  `replying_to_claude_sends_the_conversation_so_far`,
  `a_draft_fills_the_gap_in_the_marked_scenes_in_one_undo_step`,
  `a_reply_to_a_draft_asks_for_it_revised`,
  `pinning_an_answer_puts_it_on_a_post_it_over_the_passage_once`,
  `story_notes_are_saved_and_sent_with_questions_about_the_story_but_not_corrections`.

### R14 — Scenes: one per pin, each covering the story up to the next; the caret's shows

`Doc::scenes` are kept with the story (saved, R9), each pinned to a char: the start of the
paragraph it was painted for (a description, a selected passage), or, following the writing, the
sentence its part of a passage begins with. `start_scene` and `pin_painting` take the pin as it
is; callers that paint for a paragraph pass `para_start` themselves.

- **A scene covers the story from its pin up to the next scene's** (`Doc::scene_at(c)`: the last
  shown, painted scene pinned at or before `c`).
- **The caret's page follows the caret** (`App::scene_of_page`): it shows the scene of the part
  the caret is in, wherever it is drawn (shown, in a pile, turning), and moving the caret into
  another scene's part fades to that scene. Every other page shows the scene at its top
  (`Doc::page_scene(i)`). So a page looks the same while turning as once turned to, and turning
  back to the caret finds its picture as it was left. Every look (`render.rs`, `book.rs`, the
  settled page in `main.rs`) passes its page to `backdrop_shapes`.
- **Following the writing groups sentences into scenes by what they say, and paints each scene
  once the next begins.** The first edit opens a scene at the sentence written in (`open_at_edit`,
  from `History::last_at`); what was written before counts as drawn. After `PAUSE` without edits,
  the sentences finished since (`sentence_starts`) go to Claude numbered, with the open scene's
  text so far (`reading_prompt`), and it answers which begin a new scene (`parse_firsts`). At each,
  the scene before ends: it is queued with its text up to there as subject (`reading_done`) and
  painted, once; the new one is open. The open scene is never painted while open.
  - **Only content decides**, never layout: dialogue, thoughts and description belong to the scene
    they happen in, and line breaks and empty lines are no scene breaks (the prompt says so).
    Lines only end sentences, since a line of dialogue often has no full stop. A capital after
    ". " begins a sentence; lower case goes on it (`"Where?" he asked.` is one).
  - The open scene and each sentence being read are unpainted scenes pinned to their starts, so
    edits move them (R7). How many of the open scene's sentences were read is a count
    (`Open::read`), so a sentence put into the open scene's middle shifts which is read next by one.
  - If Claude cannot answer, the sentences go on the open scene. `Backdrop::drawing` covers the
    reading and the queue, and "Stop painting" (`stop_painting`) ends both; the open scene stays.
- **Painting a paragraph again adds a version** to its scene (`pin_painting` finds it by `at`)
  rather than another scene; `shown` picks one. Moving a scene onto a paragraph with one already
  (Pin here) takes the other's place.
- **A scene is made when painting starts**, with no versions, so its pin follows edits made while
  Claude paints (or, following the writing, while it is open, being read, or queued). It is not
  saved (`from_doc` skips it), never shows, and is dropped once its painting ends without a
  picture (`drop_unpainted_scenes`, which keeps the open, read and queued ones).
- **Drawings are rendered on threads, when needed:** those of pages within `NEAR` of the page
  shown, of scenes that begin on them (for the caret to reach), and of the caret's scene wherever
  the pages are, and small ones while the list is open. Textures beyond those are let go. Turned
  to, a page shows its drawing at once; a change on the page shown (caret, painted, another
  version, hidden) fades.
- **A painting carries the people its subject names** (R17), their look and model sheet before
  the passage in the prompt (`scene_prompt`), kept in `Backdrop::asked`.
- **In the browser Caprice's server paints** (`/api/scene`, R33): `paint_with` sends the same
  prompt there instead of to Claude Code, when `App::claude` is false and there is a server; the
  answer, an SVG, is rendered and taken like Claude's (`painted_online`, then `take_painting`).
  `App::can_paint` says whether painting is possible at all (Claude Code, or signed in): every way
  into painting (Describe the scene, Draw, Paint again) is off without it.
- **A painting is kept the moment it arrives** (`keep_painting`), not when the story would save
  itself: it was paid for, and Claude never paints the same one twice. A story with a file is
  saved, so it goes online soon after (R33); a new story without one goes on the shelf under the
  name it suggests (in the browser; on the desktop there is no shelf to put it on); and a version
  is kept, so dropping the changes since saving (R22) can bring it back. Autosave off, only the
  version is kept. Test: `a_scene_painted_is_saved_at_once_and_a_new_story_goes_on_the_shelf_with_it`.
- **A picture that finishes after the page went is not lost** (a reload, or another story opened
  while it was painted, as `open_scene` starts the backdrop afresh). Painted by Caprice's server,
  each painting is named (`Job::online_id`) and says which story and char it is for; the server
  keeps the picture (R33) until the app says it has it (`forget_painting`, once it is kept). On
  opening a story, signed in and holding the pen, the app asks for those waiting
  (`recover_paintings`): each goes to the scene at its char's paragraph, a new one if there is
  none (an unpainted scene is not saved), and is kept at once. One arriving in the job under way
  is not taken twice, nor a picture the scene has already. Test:
  `a_scene_painted_while_the_page_went_comes_back_when_the_story_opens_again`.
- **The world in a picture moves a little; the people never do** (`motion.rs`). Claude puts
  each part that could move in a group named by how it moves (`motion::PROMPT`: `move-wing-1-310-240`
  is a wing turning about its shoulder at 310,240), and marks chimney tops, whose smoke Caprice
  draws itself. A drawing is rendered once (`layered`, on the thread, wherever `rasterize` was
  used for the page behind): the still picture is the SVG with those groups cut out, and each part
  is rendered alone, cropped, in its place. `Drawn::shapes` moves the parts by the clock as
  textures, through the same `map` as the picture, so they turn with the page. What does not fit:
  an unknown kind, a wing or sway whose point is far from its part (`NEAR_PART`), a part usvg
  cannot find, stays in the still picture; a mistake costs movement, never the picture. The
  wings of one creature (shoulders within `ONE_CREATURE`) beat together, as slowly as its largest
  wing's span says (`beat`, from `QUICKEST_BEAT` to `SLOWEST_BEAT`), and mostly glide: a burst of
  two or three smooth strokes at uneven times (`wing_lift`), never a steady beat or a sharp turn,
  which pull the eye from the text. Only full-size drawings
  are layered: the list's small pictures, Save picture and the cast's sheets are whole, still.
  The pictures rest while the writer writes (`App::pace_scenes`): parts move by
  `Backdrop::clock`, which runs at `Backdrop::pace`; an edit (`Doc::version` changing, not opening
  the story) slows it to 0 within `SETTLE`, and `CALM` seconds after the last edit it gains speed
  again over `WAKE`. Anything new that moves on a scene moves by that clock, never by `ctx` time.
  A moving picture on the page shown asks for a frame every `MOVE_FRAME` (not while minimized),
  and at rest only one for when the calm ends; nothing else does. "Pictures move" in the Scene menu (`Backdrop::moving`, kept as
  `SceneFile::still`) stops them, parts drawn where painted and no smoke. A change to what is
  marked goes in `PROMPT`, `parse_id` and `pose` together. Scenes are not printed, so print needs
  nothing. Tests: `the_parts_that_move_are_taken_out_of_the_still_picture`,
  `a_creatures_wings_beat_together_each_upward_on_its_own_side`,
  `large_wings_beat_slowly_now_and_then_and_mostly_glide`,
  `a_picture_with_nothing_marked_is_all_still`,
  `the_picture_on_the_page_shown_beats_its_wings_and_drifts_its_clouds_until_told_to_stand_still`,
  `the_picture_rests_while_writing_and_starts_moving_slowly_half_a_minute_after`.
- Moving, hiding, deleting and stepping versions change `Doc::scenes` directly: like page
  settings, they are not undoable (R1) and do not paginate.
- Tests: `the_picture_follows_the_caret_from_one_part_of_the_story_to_the_next`,
  `turning_back_to_the_caret_finds_its_picture_as_it_was_left`,
  `painting_a_paragraph_again_adds_a_version_and_keeps_the_others`,
  `a_scene_still_being_painted_is_not_saved_and_goes_if_stopped`,
  `following_the_writing_paints_each_scene_once_the_next_one_begins`,
  `following_the_writing_begins_with_the_sentence_written_in`,
  `live_reading` (ignored, calls Claude: dialogue stays in its scene), and those of R7 and R9.

### R15 — Post-its: one size, the text in sheets; a sheet shows its own rows and no more

A post-it is always `NOTE_SIZE` square. `sheets` breaks its text into sheets of whole rows (as
many as fit the text area above the counter), and two drawings must agree with it:

- **At rest** (`draw_notes`) the whole text is one `TextEdit`, shifted up to the sheet on top and
  clipped to that sheet's rows, not to the whole text area. The area has room for part of one
  more row, which would show cut in half and look like text that cannot be read.
- **Painted** (`note_shapes`: other pages, turning, the pad flipping) each sheet is laid out on
  its own (`sheet_galley`).
- A change to the font, padding, footer or row breaking goes through `sheets`, and both follow.
- **The sheet counter ("‹ 2/3 ›") is placed in one place, `sheet_counter`,** used both to draw it
  and to split the footer into back and forward at its middle. It sits at the footer's right, so
  split at the footer's own middle, both arrows turned forward.
- **A post-it goes wherever its page goes.** In a book, pages in the pile lie turned over and show
  their post-its' backs mirrored on the left (`stack`), so a page may only get there by turning
  over (`book_turn`, which carries its post-its). Adding a page in a book therefore slides the
  new page in from the right while the old one turns over onto the pile (`book_slide_in`); the
  paperstack slides it in while the old page goes into the pile as on any page forward
  (`slide_page`, with the new page as `under`), its post-its with it. Backspacing a page away is the same
  backwards: it slides out to the right while the page before turns back over it out of the pile
  (`book_slide_out`, at the same pace), and only then is that page drawn flat and editable. On a
  paperstack the page before comes back out of the pile as on any page back (`paper_slide_out`,
  through `slide_page`), and is likewise editable only once flat. Both looks take the same
  unhurried pace for a page coming or going (`book::NEW_PAGE_PACE`). Anything
  new that moves pages must carry the post-its with them, or they jump sides.
- **Only a sheet clear of the one above casts a shadow** (`sheet_at` without `text` draws its paper
  bare). A sheet's shadow (`App::paper`) is six soft layers around and below it; sheets held out
  by the dragged scrollbar lie a sheet apart, and forty of them laid theirs over one another beside
  them, black. Anything new that draws many sheets close together draws one shadow for them.
  Guarded by `in_the_paperstack_pages_held_out_by_the_scrollbar_cast_no_shadow_dark_together`.
- Tests: `a_post_it_shows_whole_rows_and_the_rest_on_its_next_sheet` (checks what is painted,
  through `Harness::painted`), `a_post_its_sheets_turn_back_as_well_as_forward`,
  `in_a_book_a_new_page_slides_in_as_the_old_one_turns_over_with_its_post_it` (follows the
  post-it's colour and the new page's paper),
  `in_a_book_a_backspaced_page_slides_out_as_the_one_before_turns_back_with_its_post_it` and
  `in_a_book_a_page_backspaced_away_between_others_turns_the_one_before_back_too` (both follow the
  post-it, the leaving page and the pace, through `assert_turns_back`),
  `typing_while_a_backspaced_page_slides_out_goes_into_the_page_turning_back`,
  `on_a_paperstack_a_backspaced_page_slides_out_as_the_one_before_comes_back_with_its_post_it`,
  `on_a_paperstack_a_new_page_slides_in_as_the_old_one_goes_into_the_pile_with_its_post_it`.

### R16 — Windows float over the pages, and what happens over them is theirs

The scratchpad, the review, the ideas, Claude's answer, the story notes, the scene panels, the cast, Print, Versions and Document info are egui windows
over the pages. They belong to the window, not to a page, so turning pages leaves them as they are. The
scratchpad's text is the document's (`Doc::scratchpad`, saved, R9); whether it shows is the app's.

- **Input over a window is the window's.** The view acts on the wheel and touchpad only over the
  pages (`ctx.layer_id_at` is a `Background` layer): `swipe_pages` for sideways swipes, and
  `page_rect` for panning up and down. Anything new the view does with the pointer checks the same.
- Keys typed into a window's field never reach the story: the editor yields while
  `egui_wants_keyboard_input()`.
- **A window sizes itself to what it holds, so a row must not ask for more than it has.** A field
  given `available_width()` less a guess for the button after it widens the window a little every
  frame, without end. Lay such a row out right to left inside `horizontal` (the button first, then
  the field at `f32::INFINITY`); `with_layout` alone takes all the height left and centres the row
  in it. (Found in the Cast window, guarded by `someone_can_keep_several_looks_and_scenes_get_the_one_in_use`.)
- **Anything that takes clicks over the page is on a layer of its own** (a window, or an egui
  `Area` like the second row of tools). The editor takes a press whenever the pointer is in its
  rect on the background layer (`contains_pointer`), whatever else was drawn over it there: a
  click on the second row's tools, drawn on the background, also moved the caret in the text
  under it. Guarded by `tools_that_do_not_fit_open_in_a_second_row_under_the_toolbar`. Text
  boxes are drawn on the background too, after the editor, so the editor leaves a press alone
  where a box lies (`box_hits`, passed to `mouse`); guarded by
  `a_text_box_is_written_in_moved_and_resized_and_goes_where_its_text_goes`. The search
  bar is still drawn on the background; it lies over the top margin, which takes no clicks, unless
  the margin is very small.
- **The toolbar is in groups (`ui::GROUPS`), and those that do not fit go into "⋯"**, the last
  first, which opens them in a second dock under the toolbar (rows as needed). Each group is
  measured as drawn (`tool_w`) and the fit is worked out from last frame's widths, so a group only
  ever put away is measured when the second row is open. **A group's measured width must not
  depend on where it is drawn**: the line before the paragraph and page groups (`has_line`,
  `LINE_W`) is drawn outside the measured scope and added in the sums (`cost`). When the line was
  measured with its group, a group starting a row was narrower than the same group inside one: it
  ran past the second row's edge, and at some widths it moved back and forth between two rows
  every frame. Guarded by `the_toolbar_settles_at_every_width_and_its_second_row_holds_its_tools`.
  The second row closes on a click on the pages, but not on one in a menu opened from it (a popup
  is on a higher layer); the search bar goes under it (`tools_bottom`). All the tools take about
  1020 points, about 1090 while a scene is painted (the Claude group grows), so they fit in the
  test window (`Harness`, 1240 wide, where the dock is its widest, `DOCK_MAX_W`) and the tests
  reach every menu; `Harness::width` narrows it. A new tool that makes them too wide shows as a
  test failing to find a menu of the last group ("Scene"). A new toggle goes in the page bar (like Fit and Scratchpad)
  or into a menu.
- **On a phone the docks are laid out for a finger (`App::holding`, from `ui::held`: `Upright`, narrower
  than `view::NARROW`, or `Sideways`; the page fills the width in both).** The toolbar holds File, Undo and Redo (`history_tools`:
  a phone has no keys for them), the name in the middle of the dock and "⋯" at its right end; all
  the other groups are in the second row, so the row has no room left empty after "⋯". The page
  bar lying down holds the page count ("2 / 9", with room for the widest it can be, so nothing
  moves as pages turn), the track, then "+", Fit and the scratchpad as a pad, and no zoom (two
  fingers set it); short of room the track goes first, then the count, the scratchpad and "+", and
  without the track what is left stands together in the middle (`phone_scrollbar`, its own sums).
  While the writer writes on a touch screen (`writing_by_finger`: `writing`, kept by
  `hold_keyboard`, and a touch screen; on a desktop typing sets `writing` too), the page bar holds
  bold, italic, underline and the paragraph's tools instead, just above the keyboard
  (`phone_strip`, sliding sideways when too short; its width measured as the toolbar's groups are);
  the keyboard put away takes them with it. They call the same tools and `apply_tools` as the
  toolbar. Something new for a phone's page bar goes into `phone_scrollbar` or `phone_strip`.
  **On its side, while the writer writes, the docks are away** (`App::docks_away`, worked out at the
  start of each frame, before the camera): the keyboard leaves a strip about 95 points high, less
  than the two docks, which covered the page and each other. Only a round button at the top left
  shows (`tools_button`, "Show the tools", on a layer of its own); it puts the keyboard away
  (`writing` to `None`), which brings the docks back, and a tap on the page puts them away again.
  `view_area` is then the whole window, and `tools_bottom` its top. Not while a field in a window
  has the keys (the search, opened from the toolbar). Anything new drawn in a dock goes away with
  it there; something that must stay in reach while writing on a phone's side goes in "⋯" or by
  the round button. `on_a_phone_on_its_side_the_page_fills_the_width_and_the_docks_go_away_while_writing`.
  Guarded by `on_a_phone_the_toolbar_holds_file_undo_and_redo_with_the_name_in_the_middle_and_the_rest_under_more`,
  `on_a_phone_the_page_bar_holds_the_count_the_track_and_small_buttons_with_no_room_left_empty`,
  `on_a_phone_while_writing_the_page_bar_holds_the_formatting_over_the_keyboard`, and at every
  width by the two tests below.
- **The document's name is always at the toolbar's right end** (on a phone, between the tools and "⋯"). The tools get the room first, but
  the name keeps about four of its characters' width; when it has less than it needs it runs past
  in a loop (`NAME_SPEED`), resting with its start showing between passes (`NAME_PAUSE`); it
  repaints about 30 times a second while it moves. Guarded by
  `the_file_name_is_always_in_the_toolbar_and_runs_past_when_there_is_little_room`.
- **Nothing in a dock is drawn over anything else or past the dock's edge, at any window width.**
  The page bar lays out from the right (Scratchpad, Fit, the zoom, New page) and, as it gets
  shorter, drops the page count, shrinks New page to "+", drops the track, the zoom, Scratchpad,
  "+" and Fit, the track taking whatever room is left (guarded too by
  `the_page_bar_leaves_no_empty_room_left_of_its_buttons`); the sums in `scrollbar` must match the
  widths drawn. The toolbar drops "⋯" too when there is no room for it. `ui::MIN_WINDOW` keeps a
  floating window wide enough for the usual buttons, but a tiling window manager ignores it, so
  the docks must give way at any width. Something new in either dock needs room in these sums and
  a place in the order things go. (Buttons used to slide out past the bar's left end, and
  Scratchpad stayed in a window narrower than the bar.) The second row's rows are as wide as the
  toolbar, but a group wider than that gets a row reaching nearly across the window, and one wider
  still is left out. Guarded by `in_a_narrow_window_the_docks_hold_their_buttons_without_overlap`
  and, for everything that takes clicks in both docks and the second row at every width from 40 to
  1300 with a short and a long name, `at_any_width_the_docks_settle_and_hold_their_tools_apart`
  (in place, inside its dock, clear of the others, the same from one frame to the next). A new
  tool is covered by it without a change.
- **The page bar lies under the pages or stands at the left or right (`ui::BarSide`), and the
  pages keep clear of it.** Its grip, at the bar's start, docks it where it is let go: the left or
  right quarter of the window, or the middle. `App::bar_rect` is its dock for each side (standing,
  it starts under the toolbar's second row when that is open), and `view_area` leaves its room, so
  Fit, panning and swiping never reach under it; anything else placed by the bottom dock (the status
  pill, the scratchpad's first place) asks `bar.side`. Standing, it holds from the foot up the
  scratchpad as a drawn pad, Fit, the zoom, New page as "+", the track running down and the page
  number at the top (`standing_scrollbar`, its own sums); the track and thumb are one piece for
  both (`page_track`), and `App::last_track` is the thumb's stretch as two points, whichever way it
  runs. A new tool in the page bar needs a place in both, and in `phone_scrollbar`. The side is kept in the data folder
  (`page_bar.json`, like the theme), and `Harness` keeps it nowhere (`BarPlace::kept_in(None)`).
  Guarded by `the_page_bar_stands_at_the_side_its_grip_is_dragged_to_and_stays_there`.
- **The thumb counts a page sliding in only as it arrives** (`page_track`: `slide_in`, and a page
  added in this very frame). New page is in the bar, drawn after `frame` starts the slide, so the
  page was there before its slide was, and the thumb jumped back for a frame, then on. Anything
  in the bar that adds or takes away pages must keep the thumb in step the same way. Guarded by
  `a_new_page_slides_the_thumb_on_without_a_jump_back_lying_or_standing` (frame by frame).
- **The thumb let go of is drawn where the pages go, in the very frame of letting go**: in the
  paperstack, letting go moves `pos` (`let_go_to`) after the thumb's place was worked out from it,
  so `page_track` works it out again. Without, the thumb jumped back to where the drag began for
  that frame. Guarded by `in_the_paperstack_the_thumb_let_go_stays_where_it_was_let_go` (frame by
  frame).
- **An item marked as chosen in a menu is `Button::selectable` (or `selectable_label`), never
  `.selected(true)` with `.frame(false)`**: the theme gives chosen items dark text meant for the
  accent highlight, and without a frame there is none, so it was dark on the dark menu (the text
  box's look). Guarded by `the_look_a_text_box_has_reads_clearly_in_its_menu` (`painted_contrast`).
- **A popup that needs more input (a colour picker) holds it in itself**, never in a popup of its
  own: a click in a popup opened from a popup counts as outside the first, which closes both (the
  "Other colour" picker did nothing). Guarded by
  `any_colour_can_be_picked_and_black_goes_back_to_the_usual_ink`.
- Tests: `the_scratchpad_stays_on_the_right_as_it_is_while_pages_turn_and_is_saved`,
  `scrolling_over_the_scratchpad_scrolls_it_and_not_the_page`.

### R17 — The cast: people are found by name in what is painted, or just before it, and sent with that scene only

`Doc::cast` holds the people of the story (`Character`: name, other names, and one or more
`Look`s, each words and a model sheet), saved with the document (R9), never exported (R10), not
undoable and not pinned to the text.

- **One look is in use at a time** (`Character::active`, `look()`), chosen by hand in the Cast
  window, and every painting from then on gets that one; others are kept, sheets and all, to switch
  back to. Everything that sends or draws a look goes through `look()` or a look's id, never
  `looks[0]`. A new look starts from the words of the one in use, and is not put in use. Deleting
  the look in use puts the next one in use; the last look cannot be deleted. Scenes already
  painted keep their pictures, and do not record which look they got.
- **A model sheet belongs to a look** (`SheetJob` holds the person's and the look's id), so a
  sheet arriving after its look was deleted is dropped, and one drawn for a look not in use waits
  there. Older files hold one look as `look`/`sheet` on the person; `into_doc` makes it their only
  look (`people_from_files_before_looks_open_with_their_one_look_in_use`).

- **Who is in a scene comes from its subject** (`Doc::named_in`: the passage or the description
  painted), from the paragraphs just before a passage that names no one (below), or from the
  writer's choice (`people_for`). Only the subject and the cast are sent, so, as in R13, Claude
  sees only what the writer marked plus what the writer wrote for Claude. Every way of painting
  goes through `paint_with` → `scene_prompt`; a new one must too.
- **A passage that names no one carries on those named just before it** (`carried_people`): the
  sentences before it in its own paragraph (a scene may begin partway into one), else the
  nearest paragraph before it that names anyone, `CARRY_BACK` paragraphs back at most, never across
  a break in the story (chapter title, page break, an empty or starred line). Only passages carry;
  a description is all the writer wants painted. It reads text the passage does not hold, but only
  to choose among the cast: that text is never sent (R13).
- **A scene keeps who it was painted with** (`Scene::people`, by name, saved), set when its
  painting arrives (`Job::people`), so a stopped or failed painting changes nothing. The list of
  scenes shows them; choosing others there (`chosen`) makes every later painting of that scene
  use them instead of looking again. "Paint again" paints with exactly those shown
  (`paint_with`). A renamed or deleted person is left out of the prompt (`cast_by_names`).
- **Names are whole words, case counts but for the first letter** (`find_word`): "Mara's" names
  Mara, "Maradona" does not, "rose" is not Rose, and "The miller" is "the miller". Changing this
  changes who appears in pictures already painted the next time they are repainted.
- **A scene gets everyone's look but at most `MOST_SHEETS` model sheets**, those named first: each
  sheet is 15 to 30 KB of SVG, and more people in one picture get mixed up. Sheets name their ids
  after the person (`slug`), so two in one picture do not clash.
- **The passage decides the pose** (`cast_prompt`): told only to reuse the sheet, Claude copied a
  standing figure into a passage where she sat. Sheets have a sitting view for the same reason.
  Known gap: a pose the sheet shows from another side (sitting with her back to us) still came
  out standing.
- A model sheet is kept only if it renders (`draw_sheet`). The example picture is taller than some
  graphics cards take, so it is scaled to `max_texture_side` when read.
- Tests: `a_scene_that_names_someone_in_the_cast_is_painted_with_their_look_and_model_sheet`,
  `a_model_sheet_is_drawn_from_the_look_and_the_example_opens_from_the_cast`,
  `a_passage_that_names_no_one_is_painted_with_those_named_just_before_it`,
  `the_list_of_scenes_says_who_each_was_painted_with_and_paints_again_with_those_chosen`,
  `someone_can_keep_several_looks_and_scenes_get_the_one_in_use`, and in `cast.rs`
  `people_are_found_by_any_of_their_names_as_whole_words`,
  `a_scene_tells_how_the_people_it_names_look_and_sends_the_first_sheets`,
  `a_model_sheet_is_asked_for_with_ids_from_the_name`.

### R18 — At a soft wrap one position is two places; `prefer_next` says which

Rows of a paragraph are contiguous (`PageLayout::rows`): a row ends where the next one starts, so
the position at a wrap is both the end of one line and the start of the next. `App::prefer_next`
says which: true (set by `set_caret`, Home) the start of the next line, false (End) the end of
this one.

- **Everything that asks which line the caret is on goes through `row_of` / `caret_rect` with
  `prefer_next`**, and they must agree with what is drawn: egui draws the caret by
  `CCursor::prefer_next_row` the same way. Taking the wrap as the end of the line above while the
  caret shows at the start of the next made Down stay put and Up skip a line.
- Something new that moves the caret line by line, or to a line's ends, sets `prefer_next` for
  where it means the caret to be.
- Tests: `up_and_down_from_the_start_of_a_wrapped_line_move_one_line`,
  `arrow_keys_treat_a_drop_cap_and_the_line_beside_it_as_one_line`.

### R19 — Review and What next: the button names what is read; findings are found again by their words

Two windows on the left (`review::left_dock`: the review above, the ideas below), opened from the
Claude menu, not saved as open, their results not saved either.

- **What is sent is named on the button that sends it**, so the writer still decides what Claude
  reads (R13). Review: the selection, else the caret's chapter (from its title to the next), else
  the whole story, at most `review::MOST` chars (whole paragraphs around the caret beyond that), in
  `review_stretch`, which both the label and `start_review` use. What next: the story before the
  caret (`ideas_from`, at most `ideas::BACK`, from a paragraph's start; the tip says how many
  words), the story notes, the writer's wish, and the scratchpad while "Use the scratchpad" is
  ticked. Never text after the caret.
- **A finding holds the words it is about, not a range.** The text is edited while the findings
  show, so `find_quote` looks for the quote each frame, the place nearest where the stretch began
  if it is there more than once; a finding whose words are gone cannot be shown or applied.
  Applying goes through `replace_range` (R1: one undo step). The review asks for a quote short
  enough to be found, and copied exactly.
- **Both read Claude's whole reply as a JSON array** (`claude::json_list`, tolerant of words or a
  code fence around it), through `claude::Pending`, a request on a thread that stops when dropped.
  Unknown kinds and empty entries are left out (`parse`).
- An idea is kept by adding it to the end of the scratchpad (saved, R9), not on a post-it: it is
  about where the story goes, not about a passage.
- Tests: `the_review_reads_the_carets_chapter_shows_what_it_found_and_applies_a_fix_in_one_undo_step`,
  `what_next_reads_the_story_before_the_caret_and_keeps_an_idea_in_the_scratchpad`, and in
  `review.rs` / `ideas.rs` the parsing (`findings_are_read_from_the_reply_whatever_is_around_it`,
  `ideas_are_read_from_the_reply_whatever_is_around_it`).

### R20 — Print draws what the screen draws, from the same layout

`App::pdf` lays every page out at page size (`layout_page(i, 1.0)`, no highlights) and draws
`PageLayout::ink`, the list `paint_layout` draws on screen, between `App::underlay` (text boxes
behind the text) and `App::furniture` (header, footer, text boxes in front), which every look
draws too, in that order (R24). So a
printed page breaks, wraps and justifies where the page shown does.

- **Anything new drawn on a page goes into `PageLayout::ink`**, not straight onto the painter in
  `paint_layout`, or it shows on screen and not on paper. A new kind of `Ink` needs drawing in both.
  Text is printed from the galleys' own sections (font, colour, underline, slant), so new character
  formatting done through `run_format` prints without more work; one drawn by other shapes does not.
- **Fonts are embedded from `FontBook::defs`**, the same files egui draws with, each char in the
  first face of its family's chain that has it, as egui picks. A font loaded some other way must
  go into `defs` too.
- **`Ink::Fill` and `Ink::Line`** are the shapes of headers, footers and text boxes: a fill must be
  convex (egui fills nothing else). On screen and on a turning page they go through `Ink::shape`;
  print draws them as paths (`draw_shape`).
- **Letters follow their own widths** (`places`): at page size egui puts each glyph on a whole
  point, which printed looked like "bul let". Lines keep egui's start and, justified, its end.
  Chars egui shaped into one glyph (a ligature) are printed one by one, sharing its width.
- Not printed (decided): the scene behind the page, post-its, search and note highlights, the
  paper's colour. Pictures are printed turned, from `export_media`.
- **Printers are CUPS's** (`lpstat`, `lp`, the same on macOS and Linux) behind the `Spooler` trait;
  setting one up is the system's (the window opens its printer settings). Asked on a thread each
  time the window opens; a job is written to a temporary PDF that `lp` reads before it answers.
  **Tests never reach the system's printers**: they put a `Spool` in `App::print.spooler`.
- **In the browser there are no printers to ask** (a page cannot reach them): the window has no
  printer row, "Save PDF" downloads the pages (R31), and "Print…" opens the same PDF in a new tab
  (`platform::show_pdf`), whose viewer prints with the browser's own dialog. Checked by
  `web/smoke.mjs`.
- Ctrl+P opens it, but not with Shift: Ctrl+Shift+P takes up the pen (guarded by
  `a_loop_drawn_with_the_pen_selects_the_text_inside_it`).
- Tests: `ctrl_p_prints_the_page_shown_on_the_default_printer`,
  `print_sends_copies_of_some_pages_to_the_printer_chosen_from_the_menu`,
  `with_no_printer_set_up_print_says_so_and_saves_every_page_as_a_pdf`,
  `each_printed_page_holds_the_text_of_that_page_and_its_number`, `a_picture_is_printed_with_its_page`,
  `a_header_and_footer_from_the_insert_menu_show_each_pages_chapter_and_number_and_print`,
  and in `print.rs` the reading of `lpstat` and `places`. Check the output by eye:
  `CAPRICE_SAMPLE_PDF=out.pdf cargo test sample_pdf`, then `pdftoppm -png out.pdf page`.

### R21 — A font is stored by name and drawn with what is installed: itself, or its twin

`Style::font` is the name the writer chose, saved (R9) and exported (R10) as it is. `FontBook::ensure`
loads the installed family that draws it (`drawn_with`): the font itself, else a twin from
`TWINS` made to take the same room char for char (Times New Roman, Liberation Serif, Tinos;
Arial, Liberation Sans, Arimo; Courier New, Liberation Mono, Cousine; Calibri, Carlito; Cambria,
Caladea). So a story written on a Mac in Times New Roman breaks into the same pages on Linux with
Liberation Serif, and prints the same (R20 embeds the twin, as drawn).

- **Fonts load under the stored name** (`face_key(font, …)`), whatever family draws them, and
  everything asks for them by that name (`family_for`). Never store the installed family's name.
- A font of the document that is not offered yet (not common, or a twin's name) is added to
  `families` on loading, so the toolbar shows it.
- With neither the font nor a twin installed, it is drawn with egui's own font, and the pages
  break differently. Only add a twin that is truly metric-compatible: one that is merely alike
  moves page breaks without saying so.
- **A font loaded is drawn from the next frame**, so pages measured before then are measured with
  egui's own font. The frame in which fonts become usable (`FontBook::activate_pending` says so)
  works the pages out again (`full_paginate`). Opening a story in a font not loaded yet used to
  keep the wrong pages (a third more text on each) until something else laid them out. In the
  browser a font's files come later still, fetched; the same frame takes them. Until then
  `FontBook::settled` is false, and nothing kept may be measured from the pages (the garden, R30).
  A family whose files could not be fetched waits no more, and is drawn with egui's font.
- **In the browser no font is installed.** Liberation Serif, Sans and Mono (the twins of Times New
  Roman, Arial and Courier New) and Noto Serif come with the app (`web/fonts`, with their
  licences) and count as installed (`SHIPPED`); a family's four faces are fetched together the
  first time it is wanted, and loaded once they are all in (`take_fetched`, after the fonts of
  the frame before are made usable, never in the same frame: egui panics on a family it has not
  got yet).
- **A font on its way must come, or be given up, soon**: until then every theme's garden waits
  (R30). A demo on a hall's stalled Wi-Fi showed the desks of all themes bare until the request
  happened to end. So the four faces are asked for at once (`platform::fetch_each`); a request
  that has gone quiet for ten seconds is given up and asked again, three times in all, while a
  slow one that is moving is waited for (`fetch_bytes` in `platform.rs`). The fonts are in a folder
  named for their contents (`web/build.sh` names it in the page's `caprice-fonts` meta, which
  `platform::font_url` reads), kept for good (`web/_headers`), and the page preloads Liberation
  Serif's faces with the app. A font link failing never reloads the page (`data-font`, in
  `web/index.html`'s error handler, which reloads for the app's own files). A font added to
  `web/fonts` needs nothing more; a family the app starts in, other than Liberation Serif, is
  preloaded in `web/index.html` too. Test: the smoke test's
  "a font whose first request stalls is asked for again, and the garden grows without a reload".
- Tests: `a_story_opened_in_a_font_not_yet_loaded_has_the_pages_that_font_gives`,
  `a_story_opened_before_its_font_has_come_grows_its_garden_with_every_page_added`,
  `a_story_in_a_font_not_installed_takes_the_pages_it_takes_in_its_twin_and_keeps_its_name`
  (it finds a pair with only the twin installed, and passes quietly on a machine that has none),
  and in `fonts.rs` `a_missing_font_is_drawn_with_an_installed_twin_of_the_same_widths`.

### R22 — A story with a file saves itself; every story keeps versions of itself

`App::keep_safe` looks at the story every `CHECK_EVERY` seconds (it serializes the whole of it,
as `unsaved` does, so not every frame). Changes `SAVE_AFTER` old are saved to the story's file,
quietly. Opening another story or closing saves a story with a file first; then, if it changed
since it was opened or saved by hand (`App::as_saved`, which autosave leaves alone), it asks
whether to keep the changes or drop them back to that (`drop_changes`, also in the File menu). A
story never saved asks Save / Don't save as before. Closing to throw away an afternoon's changes
must keep working: writers do it. A version (the whole `.caprice` file) is kept at most every
`KEEP_EVERY` while the story changes, on opening, before going back to another, and on exit
(`on_exit`), so "Don't save" on a story never saved still leaves it in the versions.

- **Versions belong to the story's id**, not its path: `<data>/<id>/<unix seconds>.caprice`, with
  `story.json` saying its name and file (none: never saved, listed under "Stories never saved").
  Anything that starts another story (open, going back to one never saved) sets `App::story_id`
  and calls `Keeper::opened`, or its versions mix with the last story's.
- **Changed means as saving counts it** (`unsaved`, R9), never a hash of what a version keeps:
  `keep_safe` once compared the whole file, garden and all (when the file still held the garden),
  with `saved_hash`, so any story with a garden counted as changed for good (autosaving the same text
  every half minute, keeping a version of a blank story, and in the browser asking before
  leaving always). Tests: `a_story_just_saved_has_no_changes_to_lose`,
  `a_fresh_story_nobody_wrote_in_keeps_no_version`.
- **The same state is not kept twice** (`kept_hash`), and a fresh story with nothing in it never.
  Two versions in one second get the next free second's name rather than writing over each other
  (dropping changes, then opening the list, did).
  Of older versions, `kept` keeps the newest `KEEP_NEWEST`, then the last of each day for
  `KEEP_DAYS` days. A story with many pictures takes room: each version holds them all.
- **Backups are words another browser's replaced, kept a week, out of the way** (`keep_backup`,
  in the story's folder under `backups/`, R33). They are never versions (`versions_in` reads only
  the folder itself) and never thinned by `kept`; `backups_in` deletes those older than
  `BACKUP_DAYS` whenever it reads them. The Versions window lists them only behind "⋯"
  (`Keeper::show_backups`), and going back to one is going back to a version. A story's folder
  first named by a backup says its file (`About::path`), or the story would show among "Stories
  never saved". Tests: `backups_are_kept_a_week_and_then_go`,
  `a_story_changed_here_and_in_another_browser_is_kept_once_as_the_newer_and_the_other_is_a_backup`.
- **The question says what is at stake, and keeps its word** (`leave.rs`): the story's words and
  chapters, how many words changed since it was saved by hand and when (`App::saved_at`, set with
  `as_saved`), or when it was begun; and that a version is kept whichever is chosen. So "Don't
  save" keeps a version (`keep_now`) before going on, when opening another story too, not only on
  exit; dropping changes keeps one already. What it tells is worked out once per document version
  (`App::leave_about`), not each frame. A story never saved is offered a file name from its first
  chapter's title or first words (`suggested_name`), in this question and in Save as.
  Tests: `leaving_a_story_never_saved_tells_what_is_at_stake_and_keeps_a_version`,
  `leaving_after_saving_tells_how_much_changed_since`.
- **Going back and dropping changes are not undoable** (like opening, R1): the story as it was
  is kept as a version first, so going back is undone by going back to that one. Going back to this story's version
  keeps its file, so autosave then writes the older text over it.
- Timing uses the frame clock (`ctx.input time`), so tests move it (`Harness::wait`); version names
  use the wall clock, or `Keeper::clock` in tests. Nothing waits on a frame: while saving or keeping
  is due, `keep_safe` asks for one.
- **Tests never write behind your back**: `Harness::new` turns autosave off and sets no folder (nor
  one for the stories last opened, R27); the tests about it turn them on with a folder of their own (`test_dir`).
- Tests: `a_story_with_a_file_saves_itself_half_a_minute_after_a_change`,
  `closing_a_story_with_a_file_asks_whether_to_keep_the_changes_already_saved`,
  `closing_can_drop_the_changes_since_saving_and_they_stay_in_the_versions`,
  `changes_since_saving_can_be_dropped_from_the_file_menu_and_a_story_never_saved_still_asks`,
  `versions_are_kept_while_writing_and_going_back_keeps_the_story_as_it_was`,
  `a_story_never_saved_is_kept_when_caprice_closes_and_opens_again_from_versions`, and in
  `versions.rs` `kept` and `ago`.
- **Beginning a new story is leaving this one** (`Leaving::New`, File › New, only with a shelf,
  so in the browser: the desktop begins one from the welcome). It asks as opening another does,
  then `begin`s a blank story, with an id of its own (R12). No Ctrl+N: the browser keeps it.
  Tests: `in_the_browser_new_offers_to_save_the_story_then_begins_a_blank_one`,
  `in_the_browser_new_can_leave_the_story_unsaved_and_with_nothing_to_save_asks_nothing`,
  `on_the_desktop_the_file_menu_has_no_new`.
- **In the browser, leaving is the page going**, with no question of Caprice's own (R31): hidden
  (another tab, or closing), a story with a file is saved and a version kept at once
  (`App::page_hidden`), and while changes are not saved the browser asks "Leave site?"
  (`Keeper::has_changes`). Both are in `App::logic`, which eframe runs while the page is hidden
  too (it draws no frame then), and which `Harness` runs before each frame as eframe does.
  Test: `in_the_browser_hiding_the_page_saves_the_story_and_keeps_a_version_at_once`.

### R23 — Find and replace keep the story's formatting and undo as one; document info never touches the story

- **A replacement takes the style of the first char it replaces** (`App::replacement`), so a bold
  or coloured name stays so. Replace one goes through `apply_edit`; replace all applies every
  edit as one `Doc::apply_group` (one undo step, notes and scenes moved, R1, R7), from the last
  match back so the places found stay true, then `full_paginate`. After replacing one, the next
  match is the first from the end of the replacement (`resume_at`): the replacement may match too.
- Whole words: neither end of a match is inside a word (letters, digits, `_`), so "ann's" names
  Ann and "Anne" does not, as in R17. Finding starts at the first match from the caret.
- Ctrl+H is Ctrl on a Mac too: Cmd+H hides the app there.
- **Word counts are in File > Document info, not in sight while writing** (asked for: a count
  always showing distracts). The window also shows the file, when the story was begun
  (`App::created`, saved) and last saved (the file's time), the versions kept, and what the story
  holds. Something new to know about a story goes there.
- **Counting words changes nothing saved.** The story's count is cached by `Doc::version`; the
  daily goal and each story's count at its first sight today live in the data folder
  (`progress.json`), the writer's, not the story's. In the story's file they would make it
  unsaved on opening, and the goal is a habit across stories. "Today" is every story's words since
  first seen that day (deleting counts against it), the day being the system clock's local one
  (`date +%z`, asked once). `Harness::new` sets no file and an empty progress.
- Tests: `ctrl_h_renames_someone_once_and_everywhere_in_one_undo_step_keeping_the_bold`,
  `document_info_tells_the_words_of_the_story_and_selection_and_todays_towards_the_goal`,
  `a_story_keeps_when_it_was_begun_and_an_older_file_takes_its_own_date`, and in
  `search.rs` / `words.rs` finding, counting, the chapter around the caret and today's words.

### R24 — Headers, footers and text boxes lie on the page; they never move the text

Headers and footers (`PageSetup::header`, `footer`, a `Band` each: a look, a line of text, a
colour) sit in the middle of the top and bottom margins; text boxes (`Doc::boxes`) lie wherever
they were put, in front of the story's text or behind it. Neither changes where pages break, so
pagination and R3 do not know them, and a header or footer change does not paginate (`set_band`).

- **Everything they draw is `Ink`, from two places**: `App::underlay` (boxes behind the text) and
  `App::furniture` (header, footer, boxes in front). A page is drawn in layers: the scene, the
  underlay, the story's text and its pictures (pictures are in the text, so always level with
  it), then the furniture. Every path keeps that order: the page shown (`draw_bands`,
  `text_boxes_behind`, the editor, then `text_boxes`), the static and sliding pages
  (`draw_underlay`, `paint_layout`, `draw_furniture`), a book's turning page (the underlay in the
  text's mesh before the text; the furniture in a mesh of its own after the pictures, which are
  quads of their own) and print (R20). Something new drawn on a page that is not the story's text
  goes into one of them.
- **`Doc::boxes` is in stacking order**, bottom first; `behind` says which layer a box is in.
  "Bring to front" and "Send to back" (`stack_text_box`) move a box into the front or back layer
  and to the top or bottom of the list. Word gets the same: `behindDoc`, and `relativeHeight` from
  the box's place in the list.
- **A box behind the text leaves the text to the story.** It takes the pointer only where no row
  of text or picture lies (`PageLayout::covers`, `takes_pointer`), unless it is selected; the
  editor leaves presses alone where `box_hits` says a box takes them, so both ask the same thing.
  egui hit-tests with the last frame's widgets, so this is one frame behind the pointer.
- **Transparent (`clear`) changes the fill only**: none for a simple box, its tint for a shaded
  box or a sidebar laid on with alpha, so on blank paper it looks the same. Looks without a fill
  do not offer it. In Word: `noFill`, or `a:alpha` on the fill.
- **A band shows one thing**, chosen with radio buttons: the page number (`{page}`, as numbered
  by R26; a band showing it is not drawn at all, look and all, on a page without one), the
  chapter's title (`{chapter}`: the chapter that starts on the page, else the last before it,
  `chapter_of_page`), or text of the writer's own. The fields are filled in when drawn
  (`bands::filled`). There is no page count (not wanted). Their size never
  feeds back into layout (R6). A header showing the chapter asks for `chapters(false)` once a page;
  print asks once for all pages.
- **A text box is pinned to a char and shows on its page** (`boxes_on`): pushed onto the next
  page with its text, it goes along; moved by hand, it is pinned again to the char under its
  top-left corner (`pin_box`). Its height grows to hold its text (`box_height`, at page size, R4).
  Its text is one style; while a box is selected the toolbar's font, size, letters, colour and
  alignment format the whole box (`format_box`, `tool_style`), and Ctrl+B/I/U do too.
- **Clicking a box writes in it; the story keeps its caret.** A selected box not being written in
  takes Delete, Backspace and Escape (`box_key`); any other key, a typed letter or a click on the
  story lets it go. Changes to headers, footers and boxes are not undoable (R1).
- **A right click anywhere on a box opens its menu** (look, colour, transparent, front or back, delete). While it is written
  in, its text field lies over it and takes the pointer there, so the field gets the same menu
  (`box_menu` on both); with it on the box alone, only the thin edge around the text opened it.
- Tests: `a_header_and_footer_from_the_insert_menu_show_each_pages_chapter_and_number_and_print`,
  `a_text_box_is_written_in_moved_and_resized_and_goes_where_its_text_goes`,
  `a_right_click_anywhere_on_a_text_box_offers_its_menu_whether_written_in_or_not`,
  `a_text_box_sent_to_the_back_lies_behind_the_text_and_pictures_and_comes_to_the_front_again`,
  `text_boxes_stay_with_their_char_and_where_it_was_once_it_is_deleted`,
  `files_from_before_footers_keep_their_page_numbers_as_a_footer`, and in `bands.rs`
  `fields_are_filled_in_for_each_page`, `a_leaf_is_convex_and_reaches_from_end_to_end`.

### R25 — Copying formatting: the letters' look always, the paragraph's only onto whole paragraphs

The brush in the letters group (`toggle_brush`) takes up a `Style` in `App::brush`: the selected
text box's, else the look at the start of the selection (at the caret, the typing style), with
the paragraph's `ParaAttrs` in `para`. Letting go of the next mouse selection puts it down
(`paint_format`, from `mouse`), as does a click: on the word under it, or on a text box (its look
and alignment, `textbox.rs`). Then the brush is put down; Escape or the button again do so too.

- **The look goes through `Style::take_look`**: everything but `para` and `image`. A new field of
  `Style` that is character formatting is copied without more work; one that is not (like
  `image`) must be kept there.
- **Paragraph format goes only onto paragraphs selected whole** (first char to terminator, R2),
  all of `ParaAttrs`, chapter title too. A word painted inside a title would otherwise make its
  paragraph a chapter.
- It is one `Edit::Restyle` through `Doc::apply` (one undo step, R1) and `paginate_after` from
  the first paragraph touched, like `set_para`. No Word mapping of its own (R10): it copies
  formatting that has one.
- Taking up the selection with a key and pasting it (Word's Ctrl+Shift+C/V) is not offered:
  egui-winit turns any Ctrl+C/V into `Copy`/`Paste` events without the key, and sends no paste at
  all when the clipboard holds no text.
- Tests: `formatting_copied_from_one_place_goes_onto_the_text_selected_next_and_whole_paragraphs_take_its_alignment`,
  `formatting_copied_from_the_story_goes_onto_a_text_box_clicked`.

### R26 — Page numbering changes are pinned to the text and hold until the next one

Every page counts, from 1 on the first (contents included). A change (`numbers::Numbering`:
count on, start again at a number, or no numbers; figures or small roman) holds from the page it
is set on until the next change. "No numbers" hides them but the count goes on underneath, as in
Word. Set in Page > Page numbers for the page shown, in the writer's terms rather than the
model's: "Show page numbers" (on is `On`, off is `Off`), the page's number (a new one is
`From`, and shows them; typed in figures or roman, `numbers::parse`) and the figures. Below, every
change, each turning to its page or undone with ✕. With no header or footer showing `{page}`, the
menu says so first and adds the usual footer (`Band::page_numbers`) in one click.

- **A change is pinned to the first char of the page it was set on** (`NumberMark`, in
  `Doc::number_marks`, kept in the story's order) and holds from the page that char is on
  (`page_of`), so an appendix without numbers stays so wherever its text goes. It moves with
  edits like a text box (R7). The contents pages hold no text: theirs is
  `PageSetup::contents_numbers`, which holds from the first contents page; the story goes on from it
  unless a change is pinned at its start.
- **Everything that shows a page's number asks `Doc::page_numbers` / `page_number`**: bands
  (`bands_ink`, R24, and the Facing look's sides, by the count), the contents' entries
  (`Entry::number`, shown even for a page whose own number is hidden, as Word's `TOC` does), and
  Word (R10). The page bar and the Print window count pages as they lie (1 to n), not by these
  numbers. Something new showing a page number uses `page_number`, not `i + 1`.
- **Setting a change that changes nothing keeps nothing** (`set_numbering` compares the numbers
  with and without it), and a page has at most one; a page without one shows the numbering in
  force there (`numbering_on`). Not undoable, like page settings (R1).
- Numbers never change where pages break (R6): the contents keep room for four figures, and a
  wider roman number runs into the dots.
- Tests: `page_numbers_start_again_after_the_contents_and_leave_out_the_appendix_wherever_its_text_goes`,
  `page_numbers_are_added_at_the_bottom_from_their_menu_and_written_in_roman_as_typed`, and in
  `numbers.rs` `a_number_is_read_in_figures_or_roman_numerals`, `numbering_holds_from_the_page_it_is_set_on_until_the_next_change`,
  `roman_numerals_are_written_small`. Check Word with `sample_docx` (its contents are roman, its
  story from 1, its last chapter without).

### R27 — Opened without a file, Caprice welcomes the writer; opening and saving remember the story

`main()` puts the welcome page up (`Start::open`) only when no file was named on the command line;
`App::new` and so `Harness` leave it down. While it is up, `frame` draws it **in place of the desk**
and returns: no toolbar, no page, no keys reach the story (only Ctrl+O, as its "Open another…").
It begins a new story of a `Kind` (`App::begin`), shows the file dialog, or opens a story last
opened through `open_path`. Escape begins a blank page, as Caprice used to start. Giving up on the
file dialog, or a story that cannot be read, leaves the page up.

- **Leaving it is a flight, not a cut** (`Start::flight`, `welcome_leaving`, drawn last in `frame`
  on a foreground layer): the desk is drawn as usual underneath, the welcome fades from over it,
  and the sheet chosen moves from its place on the page to `App::last_page_rect`, fading out over
  the page itself at the end. Until it lands (`Start::in_air`) `frame` draws no page at all, so
  the sheet becomes the page rather than a second one: never paint something over the page to hide
  it, for the desk is shaded, and a magic theme draws on it (a flat patch of `pal().desk` showed as
  a block). The story takes typing from the first frame of it. Anything that
  takes the welcome away calls `Start::leave` (or sets a flight), so it never just vanishes.
- **A `Kind` is the starting state of a story, not a setting**: its paper, margins, headers and
  footers, drop caps (`Kind::setup`), the look of its text (`Kind::body`: font from the first of
  its fonts offered, size, alignment, spacing) and its first chapter title (`Kind::title`). `begin`
  builds the flow from them and goes through `adopt`, like opening a file, then starts a new story
  id (R22), sets `saved_hash` so nothing counts as unsaved until written, and puts the caret in the
  paragraph after the title. A new kind: add it to `Kind::ALL` and each match; its sheet comes from
  its setup. Nothing about a kind is stored: the story just has those settings (R9).
- **The emblem beside the name** (`emblem.rs`) draws from when the page first showed
  (`Start::shown_at`): a pencil draws the icon's loop, it rests, fades and is drawn again
  (`ROUND`). While the page shows it asks for every frame; it stops with the page. It reaches
  `HEADROOM` above its tile, which the header leaves room for. Tests: in `emblem.rs`
  `the_pencil_draws_the_loop_then_it_rests_fades_and_starts_again`, and
  `the_welcome_shows_the_emblem_beside_the_name_its_pencil_drawing`.
- **The sheets are drawn from the page setup** (`Sheet::of`: shape, margins, title, drop cap,
  justified, spacing, header and footer looks), for the kinds and for stories last opened alike,
  so a new page setting that changes how a page looks can be shown there too.
- **`open_path` and `save_to` call `Start::remember`**, and `open_path` calls `Start::leave`. A new
  way to open or save a story goes through them, or it is missing from the list. Paths are kept
  canonical, so a story named on the command line is the same entry as from the dialog. Autosave
  saves through `save_to` every half minute: `remember` writes the list only when the story is not
  already on top, so it does not write every time.
- The list (`recent.json` in the data folder, beside `progress.json`) holds paths and when each
  was last opened (`Keeper::now`, so tests set the clock), newest first, at most `REMEMBER`. What
  the page tells about each (words, chapters, post-its, paper, first words, when changed) is read
  from the files once, when it first shows (`Start::stories`), never each frame. A file moved or
  deleted since is left out of what is shown, not out of the list. **With a shelf (in the browser)
  the page shows every story kept there** (`Start::listed`, "Your stories"): the ones last opened
  first, then the rest, last changed first, said to be opened only if they were on the list. A
  browser has no other place to find a story.
- **Coming back to it closes the story** (`close_story`, `Leaving::Home`): File › Close story
  with a shelf, and in the browser the back button (a phone's too). It asks about changes as
  opening another does, then keeps a version, puts a blank page under the welcome (`begin`, so
  there is no file: the story is out of its live room, R34, and nothing of it is open to `plan`,
  R33) and reads the list again. The back button is `platform::story_shown` / `went_back`: while
  a story shows the page's history has one entry for it, pushed once; closing from the menu leaves
  it there, as taking it away (`history.back()`) would arrive late and could close the next story.
  Tests press it with `platform::press_back`; `web/smoke.mjs` presses the real one.
- **A row of the list can go** (`Row`, `Start::asking`): its button asks in the row itself, then
  with a shelf deletes the story (`delete_story`: from the shelf, the list and online, R33), and
  without one only takes it off the list (`Start::forget`): on the desktop the file is the
  writer's own. "Open a story" on the shelf offers the same for every story but the open one. A
  story deleted is kept among the versions with no file (`keep_deleted`), so the Versions window
  lists it as a story never saved, to bring back.
- **Tests never write it behind your back**: `Harness::new` sets no file and an empty list; the
  tests about it use `Start::kept_in` with a folder of their own. What the page paints is read
  with `Harness::painted_texts` / `painted_at`; the kinds and stories are buttons for `click_widget`.
- Tests: `opened_without_a_file_caprice_welcomes_with_new_stories_and_the_ones_last_opened`,
  `the_sheet_chosen_flies_to_the_desk_while_the_welcome_fades`,
  `nothing_lies_on_the_desk_where_the_sheet_chosen_lands_until_it_lands`,
  `a_new_novel_begins_on_book_pages_at_chapter_one_and_a_journal_at_today`,
  `escape_on_the_welcome_page_begins_a_blank_story`,
  `in_the_browser_back_and_close_story_go_back_to_the_welcome_page`,
  `in_the_browser_open_a_story_offers_to_delete_every_story_but_the_open_one`,
  `in_the_browser_the_welcome_page_shows_every_story_kept_here_after_the_ones_last_opened`.

### R28 — The cast peeks out: a puppet in parts, behind the page, only while the writer rests

Someone of the cast with a peek drawn (`Look::peek`) comes out from behind the side of the page
now and then, holds its edge, looks at the line that names them, blinks, glances at the writer and
ducks away (`peek.rs`). Claude draws the peek once per look, from its model sheet (`puppet_prompt`,
through the same job as model sheets, `SheetJob::peek`).

- **A puppet is its parts** (`PARTS`): groups with those ids in the drawing, rendered one by one
  (`cut`), each a texture placed where it lies in the drawing. Only a drawing with all of them
  (the body may be missing) is kept: `draw_sheet` cuts it before taking it. A new part needs its id
  in the prompt and in `PARTS`.
- **Behind the page, then the hand in front**: `draw_peek` is called in the settled page's branch
  of `frame`, before `paper` (body, head, irises, lids: the page hides what is behind its edge) and
  after the post-its (the hand, its fingers over the edge). It is drawn only on a settled page; a
  page turned ends it. Nothing of it is printed or exported: it is not on the page.
- **Its course is a function of time** (`pose`): rising with an overshoot, looking at the line,
  blinks at set times, a glance at the writer, ducking. Typing, clicking or scrolling makes it duck
  at once (`Peek::duck`). It is placed from the page's rect each frame (`SIZE` of its height), so
  zoom and window size need nothing more. It is worked out as if on the right side and mirrored
  about the page's middle on the left, so **it leans out, away from the page, on either side**: the
  whole figure turns on its hips (`LEAN`), its top outwards, and the head tilts only a little back
  towards the line, on its neck (it once leaned in, towards the page).
- **Who, when, where** (`update_peeks`): every `EVERY` seconds at random (the first after
  `FIRST`), once the writer has left keys and mouse alone for `QUIET`; someone named on the page,
  else in its chapter, else anyone with a peek; looking at where their name first stands on the
  page, else the caret, from the side nearer it. "Peek now" in the Cast window asks for someone at
  once. While a peek is due it asks for a frame then (`request_repaint_after`), and every frame while
  one is under way. "Cast peeks out" in the Scene menu turns it off (`Doc::peeks`, saved); it
  shows only once someone has a peek drawn, so the first peek is a surprise and the way to stop it
  is there from then on.
- Tests: in `peek.rs` `a_puppet_is_cut_into_its_parts_each_where_it_lies`,
  `a_peek_rises_looks_blinks_glances_and_ducks`; and
  `someone_named_on_the_page_peeks_out_looks_at_their_name_blinks_and_ducks_away`,
  `a_peek_leans_out_away_from_the_page_on_either_side`,
  `a_peek_blinks_and_ducks_at_once_when_the_writer_types`,
  `the_switch_to_stop_peeking_shows_only_once_someone_can_peek`,
  `the_cast_peeks_every_minute_or_two_while_the_writer_rests_and_not_when_turned_off`,
  `only_someone_named_on_the_page_peeks_out_when_the_page_names_anyone`. The prompt with the real
  Claude: `live_peek` (ignored).

### R29 — Themes colour everything around the page, never the page

A theme (`theme::Theme`, picked under Page → Theme) is a `Palette`: the desk, docks, buttons, text,
accent, caret and the scrollbar's thumb. The page is the story: `PAPER` and `INK` are constants, the
same in every theme, so what is printed (R20) and exported (R10) never depends on the theme.

- **Chrome asks `theme::pal()` each time it draws**, never a colour literal, or that part stays in
  the first theme's colours when the writer picks another. Text on the accent uses `on_accent`
  (a light theme's accent can need white text); the desk is drawn with `theme::paint_desk`.
- **The palette in use is per thread** (a `thread_local`): egui draws on one thread, and each test on
  its own, so a test that picks a theme changes no other. Code drawing off the UI thread must take
  its colours with it, not call `pal()`.
- **`Themes::show` runs at the start of each frame** and is the only place the palette changes
  (`apply_palette`): a new theme fades in over `FADE`, egui's own visuals with it (light or dark
  widgets switch halfway).
- **Google's sign-in button keeps Google's colours** (`cloud::google_button`: white, grey edge, the
  four-coloured G), in every theme, as Google asks and as people know it; like the pictures, it
  is a deliberate exception to `pal()`.
- **Light themes must stay readable**: `text` on `dock` at 4.5:1, `text_dim` at 3:1. A new theme
  goes into `Theme::ALL`, so the gallery and the test cover it; a magic one (hiding things to
  find, R30) also into `Theme::magic`, as the Page menu's Theme gallery shows the classic themes
  or the magic ones or the writer's own as a switch at its top says, first those of the theme in use
  (`the_theme_menu_shows_the_classic_themes_or_the_magic_ones_as_its_switch_says`,
  `the_theme_gallery_shows_one_s_own_themes_to_choose_export_and_delete`).
- The choice is kept in the data folder (`theme.json`), not in the story; `Harness` keeps it
  nowhere (`Themes::kept_in(None)`). It is a theme's name, as it always was, or `{"own": id}` for
  one of the writer's own (R35), read either way (`theme::Kept`, untagged); one no longer on the
  shelf leaves the usual theme.
- **A theme of one's own is `Theme::Own`, never in `ALL`**: which one is `Themes::own_id`, and its
  palette, name and gallery are the shelf's (`Themes::palette`, `Themes::name`, the gallery's
  "Yours"). Anything that asks a theme its palette asks `Themes::palette`, not `Theme::palette`,
  which gives `Own` the usual one. Its palette is made readable by `own::palette_of` whatever its
  picture, so `the_docks_stay_readable_in_every_theme` covers one too.
- Tests: `a_theme_picked_in_the_page_menu_washes_over_the_desk_and_is_kept_for_next_time`,
  `the_docks_stay_readable_in_every_theme`.

### R30 — Hidden gems: on the desk, or over the page on the screen only; never moving anything

The magic themes hide things for the writer to find (`eggs.rs`): the Magic Garden (`Theme::Garden`)
a garden and a hot-air balloon, the Magic City (`Theme::City`) houses and a dragon, the Magic Sea
(`Theme::Sea`) ships, the Magic Forest (`Theme::Forest`) lanterns, the Magic Reef (`Theme::Reef`) corals, the Magic
Snow (`Theme::Snow`) the frozen lake and its igloos, the Magic Vineyard (`Theme::Vineyard`) vines, grapes and starlings. Their blurbs give nothing away. What a theme hides is its own: picking another theme is how to stop it. A theme of the writer's own (R35), and one that is a program (R36), lies on the desk the same way, and everything here holds for it too.

- **The garden lies on the desk, behind every page**, in the Magic Garden only. `draw_garden` is
  called in `frame` right after `page_rect` and before any page is drawn, so the pages hide what is
  behind them, in either look and while turning. A plant a chapter (one for a story without
  chapters), taking turns on the left and right of the page, as tall as its chapter's pages
  (`height_of`), flowering once the next chapter has begun: every plant flowers but the newest
  (the last of `Doc::garden`, not of `Doc::chapters`). The chapters come from `Doc::chapters`
  and `page_of`, so the garden follows whatever changes them, with nothing to keep in step.
  Every plant is also as much taller as the page bar hides when it lies along the bottom
  (`bar_rect`): a chapter of a page grew a plant shorter than the bar, all of it out of sight.
  Anything else growing up from the desk's bottom edge must clear the bar the same way. Test:
  `a_chapter_added_to_a_story_opened_again_grows_a_plant_in_sight_above_the_page_bar`.
- **It never shrinks while the story is open, and starts from the story as it is when opened**
  (`Doc::garden`): the most pages each plant has had, a plant a chapter in order. `grow_garden`
  raises it, in every theme, whenever the story's version or its number of pages changed, so
  every magic theme shows all the story's growth. Never while a font of the story is still to come
  (`FontBook::settled`, R21): egui's own font gives more pages, and a story opened in the browser,
  where a font's files take a while, kept that count, so the pages written next added nothing to
  see. Taking text away, a chapter out, undo: none of it lowers a plant while the story is open,
  and a chapter taken out keeps its plant. It is not saved: a story opened is counted from its
  chapters and pages as they are, the same in every browser (it was once kept in the file, and a
  story whose chapters had been taken out kept their igloos, and grew nothing new until it had
  more chapters than before; and each browser could hold a different count for the same text,
  which, being no change, was never sent online). The first count of a story opened makes every
  theme show it as it stands at once (`Eggs::regrown`): in the browser it comes a few frames after
  opening, and the themes grew it all again from nothing. A page loaded again after the GPU was
  taken away (`again.rs`) carries the garden over, as the story was never closed. Tests:
  `a_story_opened_again_shows_its_chapters_and_pages_as_they_are_not_as_they_once_were`,
  `a_story_opened_shows_what_it_has_grown_at_once_rather_than_growing_it_again`,
  `chapters_taken_out_on_one_device_leave_the_other_s_igloos_until_it_opens_the_story_again`
  (open in two browsers, each keeps what it has shown until it opens the story again),
  `a_story_never_saved_comes_back_unsaved_when_the_page_loads_again`. Nothing that counts "the chapter being written"
  for a plant or a ship asks `Doc::chapters`, which shrinks, but the length of `Doc::garden`,
  which does not: a title taken away took the flower off a plant, and brought a ship back from the
  bay to be built again while the half-built one sailed out finished. Guarded by
  `a_chapter_title_taken_away_leaves_every_plant_as_it_was_in_flower_or_not` and
  `a_chapter_title_taken_away_leaves_every_ship_where_it_was`. Whatever puts
  another `Doc` in place of the open one goes through `adopt`, which keeps the bigger garden when
  it is the same story still open (`keep_garden`: a version, its file again, dropping the changes
  since saving, a newer copy from another browser, following it live); anything else that
  replaces `doc` must do the same. Closing a story begins a blank one under the welcome page, so
  opening it later counts it afresh. Plants are matched to chapters by
  order, so a chapter put in between takes the next plant (decided: tying plants to their titles
  like notes, R7, costs more than it is worth here).
- **The city stands on the desk the same way**, in the Magic City only: `draw_city` is called right
  after `draw_garden`, so it is behind every page too. A floor for each page each chapter has had,
  the sum of the same `Doc::garden`, so the city never comes down either and needs nothing kept of
  its own. The street is fixed beside the page, whatever the window (`street`): house `k` stands
  on the left if `k` is even, from the page's edge outwards on its side, never overlapping, so
  the window only shows more or fewer of them. Each floor is given a house when it is built
  (`deal`, kept in `Eggs::order` while the story is shown): the first house in sight not yet at
  its `most`; once all are, the one in sight furthest below the rest, nearest the page first, so
  they grow ever higher, past the top of the window if the story is long enough. A floor given
  stays on its house: a window made narrower cuts the houses beyond it and moves no floor (once
  they were all dealt again, every house jumping to another height); a story opened, or the
  theme picked, deals afresh for the window it has (`city`, `placed`). A new floor is built in sight
  (`Eggs::built`, like `grown` below, `BUILD` seconds a floor, quicker when many wait): a light
  flows out of a window of the top floor (a lit one if it has one; for a house's first floor, a
  window of its neighbour nearer the page) for the first `FLOW` of it, then spreads into the floor,
  lifting the roof, and cools into the wall. Which windows are lit, and which of those flicker
  (a few, and only slightly: the rest burn steadily), come from `story_id` and the window, so a
  test can look at any moment. How a house looks (its roof, a little tower, a statue, balconies,
  arched windows) comes from its seed too, drawn after where it stands and how high it gets, so a
  new look never moves a floor. Whatever is added to a house stays within 4 of its walls: houses
  stand 8 apart, so nothing reaches the next one. The city and the sea are the size the desk's
  height gives (`Eggs::unit`), followed over a moment (`toward`), so a window made taller or
  shorter never resizes them at a stroke. Flickering needs frames: in the
  Magic City the app asks for one every 80 ms even when idle, and in the Magic Sea every 50 ms, for
  the ships rocking and the waves drifting; nothing else in the app does.
- **The sea lies on the desk the same way**, in the Magic Sea only: `draw_sea` is called right after
  `draw_city`. A ship for each chapter, from the same `Doc::garden`, so it needs nothing kept of its
  own either. The newest chapter (the last of `Doc::garden`, so a ship once launched never comes back) has its ship
  in the yard on the left of the page, a part for each page its chapter has had, up to `PARTS`
  (keel and ribs, planks below, planks above, rail and castle, stripe and portholes, main mast,
  its sail, foremast, its sail, bowsprit and jib, topsails, flags); each part comes in sight out of
  a little light (`Eggs::rigged`, `BUILD` seconds a part like a floor). Every other chapter's ship
  is whole and lies in the bay (`Bay::places`: on the right of the page and on the left beyond the
  yard, from the page outwards, taking turns; once a row is full, the next lies further out,
  smaller and nearer the horizon). The places reach `REACH` out from the page whatever the
  window, and a ship is as long whatever its width, so the window only shows more or fewer of
  them. Each ship is given its place when it first lies in the bay (`Eggs::berths`, by chapter,
  kept while the story is shown): the first free one in sight (`Bay::in_view`). It keeps it when
  the window changes; once ships changed size with the room left of the page and moved to other
  places as rows held more or fewer. When the chapter in the yard changes to a later one, its ship is
  launched (`Eggs::launch`, `Bay::launching`, `LAUNCH` seconds): the parts it lacks come in at
  once, it slides down the slipway into the water and sails out to its place, turning about if it
  lies the other way, and only then is the next keel laid. The dock (`Bay::dock`: the quay, the
  slipway and its posts) stands at the front: ships further back (`Bay::behind`) are drawn before
  it, the rest after, so a ship lying beside the yard is behind it. A ship bound for the left
  would cross it: it backs out first, turning about, sails past behind it, and comes forward to
  its place if that is at the front, growing and shrinking with how far back it is
  (`Bay::len_at`, as the rows of the bay do). Once it sailed straight there, in front of the dock. Ships are matched to chapters by order,
  like plants. A story opened comes with its fleet in the bay and its ship built. What each ship
  looks like (its stripe, sails and flags) and how it rocks come from `story_id` and its chapter.
  The water in front of a hull below its waterline is drawn over it (`Bay::cover`), farther ships
  first, so ships sit in the sea rather than on it. That water must not hide the dock: the slipway
  in the water and its posts (`Bay::piles`) are drawn after the ships at the front, all in the order
  `Bay::scene` gives; once a ship just launched hid the slipway beside its hull and left its end
  showing below the water (test `the_water_around_a_ship_just_launched_leaves_the_slipway_in_the_water_in_sight`). The Magic Sea has no `Carrier` yet: nothing
  brings a title there.
- **The forest stands on the desk the same way**, in the Magic Forest only (`eggs/forest.rs`):
  `draw_forest` is called right after `draw_sea`. The forest itself is there from the start and
  never changes (`woods`): great trunks with roots, two rows of thinner ones behind fading into
  mist, leaves along the top of the window; the trunks stand at distances from the page that come
  from `story_id`, whatever the window (the nearest, `near`, each with `BRANCHES` branches one
  above the other, are what the big lanterns hang from, so the forest and the lanterns take them
  from one place). The rest is counted from the same `Doc::garden`, so nothing new is kept and
  nothing goes out. A lantern for each page the chapter being written has had (the last of
  `Doc::garden`, not of `Doc::chapters`: a title taken away must not bring a lantern back down out
  of the trees), each rising from the forest floor into the swarm over `RISE` (`Eggs::born`, when
  each was lit). The swarm is a function of the time (`swarm`, `lantern_at`): over the desk beside
  the page, mostly on one side or the other and crossing behind the page now and then; each
  lantern goes round its middle its own way and a little behind it, so the swarm trails. When the
  chapter being written changes to a later one (`Eggs::flowed`), the swarm of the one before flies
  to a branch and becomes one big lantern hanging there over `MERGE` (`Eggs::merge`), and the next
  swarm begins. Every chapter but the one being written has its big lantern, on the `k`th of
  `spots`: the branches nearest the page first, taking turns left and right, never one whose
  lantern would hang behind the page, under the window's top or low among the trunks, nor within
  reach of one before it; chapters past them all hang on the last. Its warm light falls on the
  trunks about it and its own tree's bark (`warm_light`). Light is drawn as one mesh fading to
  nothing (`radial`): circles laid over each other showed rings. Nothing brings a title in this
  theme. A story opened or the theme picked comes with all of it there (`forest_for`, forgotten
  in another theme). In the Magic Forest the app asks for a frame every 33 ms even when idle, the
  most of any theme.
- **The reef lies on the desk the same way**, in the Magic Reef only (`eggs/reef.rs`): `draw_reef`
  is called right after `draw_forest`. The sea itself is there from the start (`water`: rays of
  sun swaying from the surface, the reef far behind, seagrass, sand and rocks; `bubbles`), at
  distances from the page that come from `story_id`. A coral for each chapter of `Doc::garden`
  standing in a place of the reef (`places`: row by row from the front back, each further up the
  sand, smaller and paler in the water (`ROWS`, `Coral::hazed`), every other row shifted by half
  a place; in each, taking turns left and right of the page from its edge outwards). The places
  reach as far from the page whatever the window; each coral is given the first free one in
  sight when it is first drawn (`bed`) and keeps it while the story is shown (`Eggs::beds`,
  cleared with `reef_for`), so a window made narrower moves no coral, as the sea's ships keep
  their places in the bay. The rows further back are drawn first, each coral with its fish, so
  the nearer ones come in front; then the rocks and seagrass (`front`), in front of every coral
  and fish, nearest the writer; the eggs rise in front of the whole reef. Each coral is of a
  kind and colour from `story_id` and its number (`Coral::of`; the kinds take turns), with a branch for each page its chapter has had, up to `BRANCHES`. All of a
  coral's branches are laid out whatever how many are shown, so a new one only adds at the tips;
  it grows out of a glimmer over `GROW` (`Eggs::branched`, the newest coral's only). When the
  chapter being written changes to a later one (`Eggs::reef_for`), the coral of the one before
  spawns over `SPAWN` (`Eggs::spawn`): eggs rise from its tips, its polyps open, and its shoal
  (`fish_at`, a function of the time) swims in from beyond the desk on its side; every coral
  but the newest keeps its polyps open and its fish. As in the forest and the sea, the newest
  coral is the last of `Doc::garden`, not of `Doc::chapters`, and a story opened or the theme
  picked comes with all of it there, nothing spawning. Each fish faces the way it swims across,
  as a number from 1 to -1 rather than a side: at each end of its way round it turns through
  facing the writer, seen narrower, over a second or two (a side flipped it in one frame); one
  arriving turns from the way it came into its way round as it slows. An octopus brings a new
  chapter title (`Carrier::Octopus`, below).
- **The hillside lies on the desk the same way**, in the Magic Snow only (`eggs/snow.rs`):
  `draw_snow` is called right after `draw_reef`. The hillside itself (`hillside`: stars, the moon
  to one side, hills with pines, the snow; `snowfall`, drawn in front of everything) is there
  from the start, from `story_id`. The frozen lake (`Lake`) lies along the foot of the desk under
  the page and either side, and the whole of it shows: it is laid out from the window at the
  fitting zoom, a little in from its edges and above the page bar, its shores meeting in a curve
  at its ends (it first reached beyond the window and under the bar, and did not read as a lake;
  `the_whole_lake_shows_along_the_foot_of_the_window_either_side_of_the_page`). It keeps what the reef keeps, in one `Tended` (`Eggs::ice`;
  `Tended::tend`): the story and the chapter growing, how far it has grown, the one before
  finishing, and each chapter's place, the first free one in sight when first drawn
  (`Tended::place`), kept while the story is shown and forgotten in another theme
  (`Tended::forget`), so a window made narrower moves none and the theme picked again shows it all
  as it stands. As in the reef and the forest, the newest is the last of `Doc::garden`, not of
  `Doc::chapters`. Each chapter has an igloo (`Igloo`), a brick for each page up to `BRICKS`,
  laid in `COURSES` from the bottom up, the last its cap (`Igloo::brick`: bands of a half oval,
  narrower towards the sides as the dome turns away, the outer ones bowed to its round; together
  they fill the dome). Each brick is a block cut out of the lake below its place
  (`Lake::cut_at`) over `HANG` (`Igloo::coming`): a glint traces round it, it rises out of the
  dark water, floats up over an arc to its brick (`Igloo::slot`), fading into the brick as it
  lands, and the hole freezes over. The lake's part of it is drawn in front of the igloos, the
  block last, before the snow. When the next chapter begins (over `FINISH`) the igloo before is
  finished: the bricks it lacks drop in one after another, its tunnel rises on the page's side,
  and a warm light comes on inside, flickering a little: the doorway glows, the joints between the
  bricks (each brick is drawn over its own outline in the joint's colour, a little within it)
  turn warm, and light falls on the snow before it.
  Zooming brings the whole hillside nearer, as through a camera: everything is zoomed by the zoom
  over the fitting one (`App::fitting_zoom`, in `view.rs`) about its ground (14 units above the foot
  of the desk, the page bar's top when it lies along it) under the page's middle.
  The hillside is laid out as at the fitting zoom, from that same point rather than from the
  window's edges (`hillside`: only its plain fills reach the edges of what shows, and hills of
  their own seeds beyond `REACH` when the page is pushed to one side), then
  transformed; the ice's units are zoomed, and its places are measured from the page's edges,
  which zoom about the same point. The lake is zoomed with the hillside, so zoomed in it reaches
  beyond the window like the rest; where a block is cut is found on the lake as laid out, then
  zoomed with it. Laid out from the window alone, the hillside first stood still
  while the trees slid out with the page's edges, then, laid out from what showed, reshuffled as
  the zoom changed. The snowfall is the window's and is never zoomed; a flake's way down is a share
  of the window's height, never the time multiplied by a size, which made every flake leap as the
  size changed. Anything that moves with the time must not multiply it by a size that can change
  (`zooming_in_on_the_magic_snow_brings_the_whole_hillside_nearer_not_the_igloos_alone`,
  `the_hillside_is_laid_out_the_same_whatever_part_of_it_shows`,
  `the_hills_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page`,
  `the_snow_falls_gently_whatever_the_size_of_things`). The places (`snow::places`) lie beyond the
  lake, spread about rather than in rows: a grid at `DEPTHS` depths up the hill, each place a
  little off it (none almost straight behind one nearer down the hill, where its igloo would hide:
  a story in ten had one so until 2026-11-15), in an order from `story_id` that takes either side of the page in turn and comes
  nearer the page first but at every depth; further up, smaller and bluer in the dusk; drawn from
  the furthest up. Nothing brings a chapter title in this theme. **It has a twin**: its port to a
  theme program (`themes/snow/theme.rs`, R36) draws the same shapes, so a change to anything it
  draws is made in both (`the_magic_snow_as_a_program_draws_what_the_built_in_one_draws_frame_by_frame`).
- **The country** (`eggs/country.rs`, `country`) is what the Magic Vineyard lies in, kept apart so
  another theme of the countryside can share it (the Magic Wheat Field did): the sky, the low sun, the clouds, the hills, cypresses and a farmhouse, laid out
  from the land (`Land`: its middle, its ground and its unit). What lies in it gives the lines along the nearer hills (rows
  of vines), the colour at the foot of the nearest, and what flies over the hills (`company`: the
  starlings), and draws its own slope in front.
- **The vineyard lies on the desk the same way**, in the Magic Vineyard only (`eggs/vineyard.rs`):
  `draw_vineyard` is called right after `draw_snow`. Its own `Tended` (`Eggs::vines`): the newest
  chapter's stretch of a row grows a bunch a page up to `BUNCHES`, each swelling green and ripening
  purple over `RIPEN` (`bunch`); the stretch before turns over `TURN` once the next chapter begins:
  its leaves go gold and red (`vines` takes how far each place along the row has turned, `turned`),
  each leaf slowly and in its own moment (`turned_leaf`), from its green through gold to one of the
  clear `TURNED` colours (`turning`: a straight mix of green and red, or a turned colour darkened
  towards a dark green, made them brown; `shade` darkens a colour in its own hue), and the vine's
  depths go a warm rust, never all but black. A leaf's colour changes only by how far it has turned:
  every seed it takes is taken whether it has turned or not, or the leaves after it would jump. Most
  leaves stay, so the stretch is never bare: only a few of them, `FALLING`, drift down one after another to lie in the grass in front
  (`falling`, each leaf by its number), and its grapes darken to the ripest purple (`bunch`'s
  `last`). The grapes stay: every chapter's stretch keeps its count. Unlike the snow, everything is
  laid out as at the fitting zoom, beside the page as it would be then, and the whole of it
  transformed by the zoom at once (`draw_vineyard`: `unzoom`), so nothing in it can zoom
  differently from the rest; `places` and `Place::in_view` take the unzoomed page and desk. The
  country is laid out from the land's middle as far as `REACH`, never from what shows, and anything
  chosen by where it stands (a leaf's autumn colour, `autumn`) takes its place from that middle, never
  from the screen: the middle is the page's, which moves as the zoom changes, and a turned leaf
  measured from the screen changed colour while zooming; where more
  shows than that (the page pushed to one side and zoomed out, R32), its hills and the rows go on to
  the edges on the same points, from seeds of their own, so what lies within `REACH` is the same
  whatever shows (`the_hills_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page`, in
  `eggs/country.rs` and `eggs/snow.rs`; `the_rows_go_on_to_the_edges_of_whatever_shows_however_far_from_the_page`); a cloud
  drifts by a share of the way across, never the time multiplied by a size (as the snowfall).
  A row is built as a vine is (`vines`): trunks up to the wire, a woody cordon along it, shoots
  rising from it, and real leaves along each shoot on short stalks (`vine_leaf`: five pointed
  lobes, rounded bays, a notch at the stalk, lighter on the sun's side, veins on the nearest row),
  of every size and way, some further in and darker, over the vine's dark depths. Its foot is ragged:
  leaves hang over the wire and below it along every stretch, the depths fade out at a wavy hem, and
  the wood along the wire bends (drawn straight, they ruled a clean line under every row). The leaves, the
  wood and the starlings are meshes, the clouds `puff`, a lighter `radial`, and the far rows have
  fewer leaves. Anything added to it should keep the
  frame cheap: count its shapes against the snow's. The rows (`Rows`: `DEPTHS` of them on the slope, each waving gently)
  are drawn from the furthest, each piece seeded by where it stands along the row; the stretches
  (`Place`, `places`) are on the `PLOTS` nearest rows, the first free one in sight beside the page.
  A starling murmuration (`starlings`, drawn as one mesh, `flock`) flies over the hills for `FLOCK`
  seconds in every `FLOCK_EVERY`, its shape and course functions of the time since it came, so no
  bird jumps; it gathers as it comes and thins as it goes. It flew above the hills first, where a
  desk with the pages fitted shows little sky, under the toolbar. People picking were tried and not
  liked (§7). Nothing brings a chapter title in this theme. **It has a twin**, as the Snow has: its
  port to a theme program (`themes/vineyard/theme.rs`, R36, the country with it) draws the same
  shapes, so a change to anything it or the country draws is made in both
  (`the_magic_vineyard_as_a_program_grows_and_turns_as_the_built_in_one_frame_by_frame` and its
  sibling for zooms and windows). It draws
  more than any other theme (136 000 points at the fitting zoom on a computer's desk, mostly
  leaves), which is what program themes may draw at most was set by.
- **It grows in sight.** How far each plant is drawn grown (`Eggs::grown`, by `story_id`) only goes
  up, a tenth of a second's growth a frame at most; a story opened comes with its garden grown.
  So does a theme picked: each forgets how far it had shown things (`grown_for`, `built_for`,
  `sailed_for`) while another theme is shown, or it caught up in sight with all that grew meanwhile.
  Its shapes come from a seed of `story_id` (saved, R22) and the plant's number, and the stem's
  course at a height does not depend on how tall it is, so growing only adds at the top.
- **A new chapter title is brought in**, every time (`Eggs::odds` keeps it away for tests), when
  the writer makes a paragraph with words in it a chapter title with the Chapter button (`maybe_lift_title`, from
  `apply_tools`): not on opening a file, undo, or a title made any other way. Which theme brings it
  how is `Carrier::of`: the balloon floats, and the dragon flies, in from the side at the title's
  height, each from the other side than the last, and away past the other. The octopus
  (`reef::octopus_at`, its own way rather than `rig`) swims down from beyond the top of the desk
  instead, in front of the page, drifting in a little from the other side than the last, the
  title hanging below it held at its top by two arms; it lowers it into its place, lets go and
  jets back up out of sight, along the same `pose`. Like the balloon, it holds the title up only
  as high as there is room for below the top of the page, and is smaller there. A title is often high
  on its page, so either hangs only as high as there is room for below the top of the page, and is
  smaller there (`TALL`, `DRAGON_TALL`: their heights above the title, wings up). Both follow one
  course in time (`pose`: come, carry over the place, lower, settle, let go, leave), each along its
  own way (`rig`, the same path for both, differing in size and rope). It comes in from beyond
  the desk's edge and leaves past the other, far enough that none of it shows when it appears or
  is gone: `rig` takes the desk (`Eggs::desk`, set each frame from the window's area) and goes
  past its edge by the carrier's reach (the dragon's tail, head and fire; the title it carries
  in). Once only page widths, a wide window saw the dragon vanish in mid air. Anything new
  drawn with a carrier must stay within its reach.
- **Shapes that turn must not fold flat.** egui pushes a filled shape's soft edge out along each
  corner, the further the sharper, without limit, and joins a line's turns the same way: a tooth
  seen edge on, a hull of points nearly in a line, or a line doubling back drew spikes right
  across the window, flickering as the dragon flew. So every filled shape of the dragon goes
  through `solid` (points on top of each other merged, sharp corners cut by a hair, slivers left
  out), and its lines are drawn as separate segments, never one `Shape::line` with turns. Anything
  drawn from moving, turning geometry needs the same. The dragon goes it slower
  (`Carrier::pace`, through `pose_of`): `Pose::t` stays the seconds as they are, so its wings,
  fire and tail keep their own time; anything that times a lift (`update_lift`, `lifting`, the
  tests' frames) goes through `pose_of` or the pace. The dragon holds the title
  by its top in its feet (`dragon_at`), beats its wings (`FLAP`) and breathes fire (`BREATH`), and
  turns its head as `Pose::look` says: back at the city while it comes, at the writer over the
  title's place, down at the page while it lowers it, ahead once it lets go or hurries away.
  Its head is drawn in three dimensions (`Turn`, `dragon_head`): each part is the convex hull of
  points round it, turned by a yaw and a pitch and drawn from the furthest to the nearest, so in
  profile the far eye and horn go behind it, and turning from the city to the page it turns
  through facing the writer. Its body is one tapering spine from the head to the tail (`ribbon`),
  with belly plates, spikes and a spade; its wings have an arm and four fingers, spread fuller on
  the down beat, beating twice and then gliding (`wingbeat`). It flies by its wings, not the
  balloon's smooth way: each down beat pushes it along its course and it slows as they come up
  (`surge`, in `pose_of`), and it rises on each down beat and sinks as they come up, the title
  in its feet with it (`heave`, in `rig`), less and less as it lowers the title, so the title
  still lands exactly in its place. `surge`, `heave` and `wingbeat` must keep the same beat. A
  slow wave runs down it from the neck to the tail (`wave`, `GLIDE`), the head riding it a
  little behind. Its jaw opens with its breath of fire. Looking back over itself, its fire is only
  a puff, so it does not cross its own back. The beat, the fire, the smoke and the tail come from
  `Pose::t`, so a test can look at any moment. Typing, a press or scrolling sets the title down at once
  and the carrier hurries away (`update_lift`); a page turned ends it. A press, never a letting go:
  on a touch screen eframe lets go twice, the second a frame after the tap on Chapter that called
  it, and on a phone it was gone before it was seen. A new carrier is a `Carrier`
  variant, its way in `rig` (or its own, through `offset`) and its drawing in `draw_carrier`.
- **The title being brought is only drawn elsewhere.** `editor_surface` moves the title's pieces of
  the page's layout by `eggs::offset` while painting them and puts them back after. Its place is
  its galleys' own rects (`galley.rect` at `x`, `y`), not their sizes from `x`: a centered title is
  placed by its middle, and taking `x` for its left edge once hung it half its width beside the
  ropes. The caret, hit-testing, pagination and print see the title in its place all the while
  (R3, R20). The
  carrier is drawn in the settled branch in front of the page, after the peek's hand.
- **None of it is printed or saved**: it is in none of `underlay`, `ink` or `furniture` (R20), and
  nothing of it is in `Doc`. Its colours are the garden's, the houses', the ships', the sea's, the
  forest's, the reef's, the hillside's, its lake's and its igloos', the balloon's, the dragon's and the octopus's own, like the emblem's, not the palette's: they are pictures, not chrome.
- Tests: in `eggs.rs` `the_title_is_brought_in_carried_over_its_place_lowered_and_let_go`,
  `the_balloon_and_the_dragon_bring_the_title_in_from_the_side_and_go_away_past_the_other`,
  `the_dragon_beats_its_wings_and_breathes_fire_from_its_mouth`,
  `the_dragon_looks_at_the_city_then_at_the_writer_then_at_the_page`,
  `what_brings_a_title_comes_from_beyond_the_desk_and_leaves_until_none_of_it_shows`,
  `nothing_of_the_dragon_or_the_balloon_streaks_across_the_screen_as_it_flies`,
  `the_dragon_takes_its_time_and_flies_by_its_wings_carrying_the_title_up_and_down_with_them`,
  `the_title_is_set_down_at_once_when_the_writer_goes_on`,
  `a_plant_is_the_same_for_the_same_story_and_grows_only_at_its_top`,
  `a_new_floor_is_a_light_that_flows_out_of_a_window_and_spreads_into_the_floor`,
  `the_houses_take_turns_on_either_side_of_the_page_as_many_as_there_is_room_for`,
  `once_the_desk_is_full_the_houses_grow_higher_a_floor_each_in_turn`,
  `some_windows_are_lit_a_few_flickering_slightly_the_same_ones_for_the_same_story`,
  `some_houses_have_a_little_tower_or_a_statue_and_none_reaches_the_next_house`,
  `a_ship_gets_a_part_for_each_page_until_it_is_whole`,
  `the_ships_lie_in_the_bay_beside_the_page_each_in_a_place_of_its_own`,
  `a_ship_launched_is_finished_at_once_slides_into_the_water_and_sails_to_its_place`,
  `a_ship_launched_to_the_left_sails_behind_the_dock_and_none_lies_in_front_of_it`,
  `a_floor_once_built_stays_on_its_house_whatever_the_window`,
  `the_size_of_things_follows_a_new_window_height_over_a_moment`,
  `the_forest_is_there_from_the_start_the_same_for_the_same_story`,
  `the_lanterns_fly_together_in_a_swarm_which_drifts_from_one_side_of_the_page_to_the_other`,
  `the_trees_nearest_the_page_have_branches_enough_for_many_big_lanterns_close_to_it`
  (in `eggs/forest.rs`); and
  `in_the_magic_forest_each_page_lights_a_lantern_in_the_swarm`,
  `in_the_magic_forest_a_chapter_s_swarm_becomes_a_big_lantern_in_the_trees_near_the_page`,
  `the_sea_is_there_from_the_start_the_same_for_the_same_story`,
  `a_coral_grows_a_branch_a_page_only_at_its_tips`,
  `the_corals_fill_the_front_row_beside_the_page_then_rows_further_back_smaller`,
  `the_fish_swim_round_their_coral_gently`,
  `the_octopus_swims_down_from_above_sets_the_title_in_its_place_and_jets_back_up` (in `eggs/reef.rs`),
  `the_hillside_is_there_from_the_start_the_same_for_the_same_story`,
  `the_whole_lake_shows_along_the_foot_of_the_window_either_side_of_the_page`,
  `a_block_is_cut_out_of_the_lake_rises_and_floats_onto_the_igloo_as_its_next_brick`,
  `an_igloo_gets_a_brick_a_page_course_by_course_from_the_bottom_up`,
  `an_igloo_is_finished_when_its_chapter_is_and_a_warm_light_comes_on_inside`,
  `the_places_are_spread_about_beside_the_page_near_and_far_up_the_hill` (in `eggs/snow.rs`),
  `in_the_magic_snow_each_page_cuts_a_block_out_of_the_lake_for_the_chapter_s_igloo`,
  `in_the_magic_snow_a_chapter_s_igloo_is_finished_and_lit_when_the_next_begins`,
  `a_long_story_s_igloos_are_spread_beside_the_page_and_a_narrower_window_moves_none`,
  `the_clouds_drift_slowly_whatever_the_size_of_things` (in `eggs/country.rs`),
  `the_landscape_is_laid_out_the_same_whatever_part_of_it_shows`,
  `the_vineyard_is_there_from_the_start_the_same_for_the_same_story`,
  `a_bunch_swells_green_then_ripens_purple_and_darkens_at_the_last`,
  `a_turned_stretch_has_red_and_gold_leaves_not_brown_and_keeps_most_of_them`,
  `a_leaf_turns_slowly_through_gold_never_brown`,
  `a_turned_leaf_keeps_its_colour_wherever_the_page_lies`,
  `a_vine_leaf_has_five_lobes_and_lies_flat_when_fallen`,
  `the_vines_hang_over_the_wire_raggedly_never_ending_in_a_straight_line`,
  `the_leaves_drift_down_one_after_another_and_lie_in_the_grass`,
  `now_and_then_a_murmuration_of_starlings_swoops_across_the_sky`,
  `the_stretches_lie_beside_the_page_nearer_first_on_the_nearest_rows` (in `eggs/vineyard.rs`),
  `in_the_magic_vineyard_each_page_grows_a_bunch_of_grapes_on_the_chapter_s_vines`,
  `in_the_magic_vineyard_the_stretch_before_turns_and_drops_its_leaves_when_a_chapter_begins`,
  `a_long_story_s_vines_are_beside_the_page_and_zooming_brings_the_whole_vineyard_nearer`,
  `print_shows_nothing_the_magic_themes_hide_nor_what_brings_a_title`,
  `a_magic_theme_picked_again_shows_at_once_what_the_story_grew_meanwhile`,
  `a_story_opened_before_its_font_has_come_grows_its_garden_with_every_page_added`.

### R31 — The desktop and the browser differ only in `platform.rs`

Caprice Web builds for the browser (`wasm32-unknown-unknown`, with threads) and natively, where
the tests run. Much of `std` compiles for the browser and panics or fails there only when run:
threads, the system clock, the process id, files, programs. So nothing outside `platform.rs` calls
them; it asks `platform` instead, whose desktop half does what Caprice always did.

- **Background work** goes to `platform::spawn`: a thread on the desktop, a Web Worker sharing the
  app's memory in the browser (`platform/worker.rs`: a worker a piece of work, from `worker.js`,
  which starts the app's WebAssembly on the shared memory and runs the boxed work by its
  pointer; the app's script URL is read from a stack, not evaluated). The work gets owned data and answers on a channel, which the frame
  reads with `try_recv`. It shares no lock with the main thread: in Chromium a page's main thread
  that waits on a lock is stopped dead ("Atomics.wait cannot be called in this context").
- **It asks for a frame with a `platform::Waker`**, never egui's `Context`: with threads on, egui
  makes `Context` impossible to send to a worker at all. On the desktop `wake` asks egui for a
  repaint, as before; in the browser it leaves word, and `platform::keep_awake` (each frame, in
  `App::frame`) asks for a frame at once when there is word, and every 50 ms while any work runs.
- **Files** are `platform::fs`, with the calls of `std::fs` the app uses, and `modified`, `made`
  (a file's dates) and `canonicalize`. On the desktop they are `std::fs`. In the browser the files
  are whole byte strings by path in memory (`fs::Memory`), all read from IndexedDB (database
  `caprice`, store `files`) before the app starts (`fs::load`, in `main`), so nothing waits for a
  file; each write and removal is then copied to IndexedDB behind. Folders are not kept: one is
  there while a file is in it. Only the main thread has them (a `thread_local`): a worker sees no
  files rather than share a lock. So versions (R22), the theme, the word goal and the stories
  last opened work the same, by path, under `platform::data_dir` (`/caprice` in the browser).
- **Fonts** are installed on the desktop, and come with the app in the browser, fetched with
  `platform::fetch_each` from `platform::font_url` when first wanted (R21).
- **The time now** is `platform::since_epoch`, the time zone `platform::utc_offset` (`date` on the
  desktop, the page's `Date` in the browser), a run's id `platform::run_id`.
- **File dialogs** are described, not shown, by the code that wants a path: a `platform::Dialog`
  (the kinds of file, a name to suggest) given to `App::ask_path` with what it is for
  (`DialogFor`). Every answer is a path, acted on in `poll_dialog`, whichever way it came:
  - on the desktop, `rfd`'s dialog on a background thread, as always;
  - with a shelf (`App::shelf.dir`: always in the browser, never on the desktop), saving and
    opening a story ask in Caprice's own windows (`shelf.rs`): a name, and the story becomes
    `<shelf>/<name>.caprice`; or a story from the shelf's list, or one uploaded onto it. With a
  shelf the File menu also has New (R22). The shelf's window (`shelf_window`) is drawn wherever
  its question can be asked, the welcome page too ("Open another…", Ctrl+O): a question asked
  where no window shows it waits unseen, and the button seems dead. **Another story is never
  saved over one of the same name**: only the story's own file is; any other goes beside it under
  `free_name` ("<name> 2"), and the window says so before saving. Replacing it lost the other's
  words here, and online too, where its record (found by path) had the new story uploaded over it.
  Test: `another_story_saved_under_a_name_kept_already_goes_beside_it_and_replaces_nothing`. So a
    story in the browser has a path like any other, and autosave (R22) and the stories last
    opened (R27; the welcome says "In this browser") work unchanged;
  - in the browser, a file to read (a picture) comes from the page's picker into `/uploads`,
    kept in memory only; a file to write (Word, PDF, the scene picture) gets a path in
    `/downloads`, and writing there downloads it. "Download a copy (.caprice)" in the File menu
    is the browser's way to take a story to the computer.
  Printing in the browser is R20's PDF, downloaded or opened in a tab.
  So a writer of files writes through `platform::fs` (never `image::save_buffer` and the like,
  which go to the disk themselves): the scene picture's PNG is made in memory first.
- **The page's ways where Caprice has its own** (`platform::browser`, at its start): Caprice's
  Ctrl shortcuts are kept from the browser (Ctrl+R aligns right rather than reloading, Ctrl+F
  finds in the story, Ctrl+= zooms the pages, not the page), all but copy, cut and paste. A new
  Ctrl shortcut in `editor.rs` or `App::frame` goes into that list too. Leaving with changes not
  saved, the browser asks (R22). Hidden, the page draws no frame, so what must happen then is in
  `App::logic`; the page asks for one on `visibilitychange` (`frame_on_visibility_change`).
- **Claude cannot be asked from the browser itself** (it needs a server, `WEB_PLAN.md` step 6):
  `platform::CLAUDE` is false there, and so `App::claude`. The Claude menu is then shown greyed
  ("Comes to the browser version later"), and Ctrl+Shift+P takes up no pen. Scenes are the
  exception: with Caprice's server (`App::cloud.server`) the Scene menu is there, and Caprice's
  server paints them for writers signed in (R14, R33); signed out, painting is off and scenes
  already in a story still show. Without a server (tests) both menus are greyed. The cast's model
  sheets are still drawn only by Claude Code. When step 6 comes, `CLAUDE` turns true there.
  Tests: `where_claude_cannot_be_asked_its_menus_and_the_pen_are_off`,
  `in_the_browser_signed_out_no_scene_is_painted_and_nothing_is_asked`.
- **Nothing is evaluated from a string in the browser**, so the Content-Security-Policy forbids
  it: `script-src 'self' 'wasm-unsafe-eval'` and the hashes of the page's inline scripts and
  style, which `web/csp.mjs` works out after each build and writes into `dist/_headers` (they
  change with every build: Trunk's starting script names the hashed files); `worker-src 'self'`.
  `wasm_thread` evaluated code to start each worker, so it was replaced by `platform/worker.rs`.
  When touching it: a library that evaluates code, or a new inline script, is refused by the
  browser; `web/serve.mjs` serves `_headers` as Cloudflare does, so the smoke test sees it.
  `smoke.mjs` checks that a stranger's inline script is refused, and fails on any refusal of ours.
- **The page itself is never kept; a build's own files are kept for good only if named by
  their contents.** Trunk names the script and the WebAssembly by their contents, so a new build
  has new names and they can be kept a year (`_headers`). The page names them, so it is
  `no-store`: kept, a tab restored or gone back to used the page of the build before, asked for
  files the new one had taken away (Cloudflare answers 403, `serve.mjs` 404), and stayed on
  "opening" until the browser's history was cleared, here and when deployed. `worker.js` and the
  snippets keep their names across builds (the snippets' folder is named for the crate, not their
  contents), so they are asked about each time (`no-cache`): kept a year, a snippet's old copy
  failed the integrity the new page asks of it, and kept failing however often it was reloaded.
  Should a file of it still not load, the page fetches every file it loads afresh past the
  browser's copies (`cache: "reload"`; only the one that failed, and the next stale one failed
  after it) and reloads itself once (`index.html`, remembered in `sessionStorage` until it has
  opened), and says which file if that does not help, forgetting it tried so that a reload tries
  again: a file failing to load is never an error on `window`, so without this it waited 20
  seconds to say only that it was still opening. Tried for real in a browser that keeps what it
  is sent: with old snippets kept a year, the new build stayed on "opening" without this, and
  opened with it.
  Anything new served beside the page under a name that stays across builds needs a rule there
  too. Checked by `smoke.mjs`: "the page itself is never kept", "the snippets, whose names stay,
  are asked about each time", "a file of it gone at first is fetched again, and Caprice opens",
  "an old copy of a file failing its integrity is fetched afresh, and Caprice opens".
- **"Follow my writing" is not offered in the browser** (the Scene menu hides its checkbox), for
  now. It is the only way into `Mode::Writing`, which is never saved.

When touching it: anything new in the background takes a `Waker`, not a `Context`, and moves no
lock or `App` state into the work. Anything new that needs the system goes through `platform`.

Tests: `clippy.toml` disallows `std::thread::spawn`, `SystemTime::now`, `std::process::id`, the
`std::fs` calls and `Path::exists` outside `platform` (test modules are let off: they run only
natively). `the_browser_s_files_are_read_back_as_written_and_folders_hold_what_is_in_them` and
`a_folder_of_the_browser_s_files_is_gone_with_its_last_file` test `fs::Memory`;
`in_the_browser_saving_asks_for_a_name_and_the_story_is_kept_under_it`,
`in_the_browser_opening_lists_the_stories_kept_and_opens_the_one_chosen` and
`in_the_browser_escape_saves_nothing` and
`in_the_browser_open_another_on_the_welcome_page_shows_the_stories_kept_here` test the shelf, given a folder as the browser has. The browser build
is checked by `web/build.sh`, and running it by `web/smoke.mjs` (headless Chromium: the welcome, a
blank story, typing, a picture uploaded, the PDF downloaded and opened to print, a version kept in IndexedDB
and listed again after a reload, a story saved by name, downloaded as `.caprice` and Word,
uploaded back, a story with a scene drawn behind the page in a worker, Ctrl+R and Ctrl+F kept
from the browser, leaving asking only with changes not saved, saving when the page is hidden, and
the stories listed on the welcome, and a page that cannot open saying why; it reports any error on the page).
The native tests cover the desktop half.

- **A page that cannot open says why.** `web/index.html` shows "Caprice is opening…" until the
  WebAssembly has started; an error before then (the download, compiling, instantiating), or
  eframe failing to start (`main`), takes its place in words, and after 20 s with nothing said it
  names what it may be (a slow connection, or a browser without shared memory: another app's own
  browser). A phone has no console to look in, and "opening" stayed for good.
- **A page that lost its drawing loads again, back where it was** (`again.rs`). A phone's browser
  takes the GPU from a page in the background (Chrome on Android: its WebGL context or WebGPU
  device is lost), and eframe cannot make a new one: back in the browser, the page stayed blank.
  The page notes the loss (`platform::drawing_lost`: `webglcontextlost`, or the device's `lost`,
  watched by wrapping `GPUAdapter.requestDevice` before eframe asks). Once it is shown again
  (`App::logic`), the app saves and keeps a version as on leaving, writes where the writer is
  (`again::Place`: the whole story, saved or not, its path and what it knew of its saving, the
  caret and selection, its device id in the story's room) to `again.json`, and loads the page
  again (`platform::reload`, without the leaving question, once IndexedDB holds every write). The
  new page reads that file once, takes it away, and opens the story without the welcome
  (`App::come_back`). Undo starts afresh. **The device id goes with it**: a new one would find
  the room keeping the pen for the old one (`LIVE_FREE_AFTER_MS`, R34) and lock the writer out
  of their own story. Something new that the writer would miss after a reload (a story's state
  kept only in `App`) goes into `Place`. Tests:
  `a_page_that_lost_its_drawing_loads_again_back_at_the_story_and_the_caret`,
  `a_story_never_saved_comes_back_unsaved_when_the_page_loads_again`.

### R32 — On a phone the page fills the width, scrolls, and keeps the caret in sight

`page_rect` is the camera. In a view narrower than `view::NARROW` (a phone held upright), the whole
page would be too small to read (about half size), so fitting fills the width with it, without
the room for post-its, and the page scrolls up and down while it stays fitted; `centered` keeps its
top no lower than the view's top and its foot no higher than the view's foot. On a phone on its
side (`ui::Held::Sideways`) the whole page was a third of its size: it fills the width too, but no
larger than `view::SIDEWAYS_MOST` (100%), as larger fewer lines showed over the keyboard and they
grew too long. Wider views fit the whole page, centred, as always.

- **Narrow is the width alone**, never "taller than wide": a phone's keyboard takes half the height,
  and the page shrank to the whole-page fit when it came up.
- **A phone is known by its screen, not its window** (`ui::held`): a window at least `NARROW` wide
  is a phone on its side when the screen's short side (`platform::screen_short_side`: the
  browser's `screen`, the desktop's monitor) is under `NARROW`. The screen does not change as the
  keyboard comes up or the phone turns round; a tablet's is wider, and lays out as a desk. Tests
  give a phone's screen with `Harness::monitor`.
- **Zooming out stops at half the fitting zoom** (`view::LEAST`, not below `MIN_ZOOM`), and a
  window grown while zoomed out brings the zoom up to it. The magic themes that zoom with the page
  (R30: the snow, the vineyard) come no further than that; below it the page shrank alone over a
  scene standing still, and pushed to one side it showed the land's end. Anything drawn around the
  page that follows the zoom can count on no less than half the fit
  (`zooming_out_stops_at_half_the_fitting_zoom_even_when_the_window_grows`).
- **The view follows the caret** (`follow_caret`) when the caret or the text changed, or the view
  got shorter, moving the page up or down as little as keeps the caret `CARET_ROOM` inside the
  view, and leaving it wherever it was scrolled to otherwise. The caret is where the editor drew
  it the frame before (`App::caret_seen`, in page points, written only for the page that is
  editable). What it was drawn for (caret, `Doc::version`, view height) is compared, never its
  rectangle: that wobbles by a rounding as the page moves, and a page moved by hand snapped back.
- **Two fingers on a touch screen pan as they pinch** (`multi_touch().translation_delta`, only over
  the pages as in R16); fitted on a phone, a page taller than the view only up and down, staying
  fitted. **A fitted page with no room to scroll goes with the fingers and is no longer fitted**
  (as Ctrl+drag): all of it in sight (a phone with the keyboard put away), the fit put it back in
  the middle each frame, and it zipped back from under the finger. Not in a frame that zooms: a
  pinch may have just stopped at the fitting size
  (`on_a_phone_a_fitted_page_shorter_than_the_screen_moves_with_the_finger_and_stays`). egui starts such a gesture only with a
  pointer position, which the browser gives with the first finger: a test sends one too.
- **One finger on the text shows what it is for before the caret moves** (`editor::Finger`): lifted
  still, a tap puts the caret there; held still `FINGER_HOLD`, it selects as a mouse drags; moved
  sideways past `FINGER_STILL`, it turns one page a swipe (`FINGER_TURN`, left goes forward, like
  the trackpad's `swipe_pages`), leaving the caret; up or down, it moves the page as two fingers do
  (`App::finger_pan`, read by `page_rect`). A second finger ends it: a pinch moves no caret.
- **A press by a finger is known by the press itself** (`platform::pressed_by_finger`, noted by a
  capturing `pointerdown` listener on the window, before eframe's). eframe runs a frame on the
  `pointerdown` alone, before the `touchstart`, so that frame has no touches; it presses again with
  the touch, where the finger is, and for every other finger, the second where it lands. Taken for
  a mouse, a pinch put the caret under a finger and dragged a selection to the second finger: in a
  margin, past the text's foot or top, that turned the page forward or back. An echo where the
  finger is is ignored; a press elsewhere, or with two touches, is a second finger. The test
  helpers `finger_down` and `pinch` send exactly what eframe sends (seen in Chromium with touch
  events); a new touch test uses them.
- **Windows and dialogs show whole on a phone.** Their widths go through `ui::window_width` (an
  `egui::Window`'s `default_width`) or `ui::fit_width` (a fixed width inside a frame of its own, as
  the question before leaving): no wider than the screen less a gutter either side. egui keeps a
  window's place on the screen, not its width; the question before leaving, 440 wide, lost its
  buttons off both sides. A new window or dialog with a width of its own uses them. The test helper
  `tap_widget` taps as eframe tells a finger's tap.
- **A finger closes a window by tapping near its cross** (`ui::finger_close`, first in the window's
  contents). egui's cross answers for itself alone, an icon's width, and a window that cannot be
  moved takes every click on itself: a finger a few points off closed nothing, and the gallery
  closed only after several taps. While `platform::pressed_by_finger`, the title bar's corner
  around the cross, and a little beyond the window's edge, closes it too; with a mouse the cross is
  left as egui draws it. A tap there closes; a drag from there still moves a window that moves.
  Every `egui::Window` with a cross has it, its `closed` taken from `open` after `show`; a new one
  does too. `a_finger_tapping_beside_the_gallery_s_cross_closes_it` (one that cannot move),
  `a_finger_tapping_beside_a_window_s_cross_closes_it_whether_the_window_moves_or_not`.
- **A row with a name and a button in a narrow column puts the button first** (`egui::Sides` with
  `shrink_left().truncate()`): a label followed by a right-to-left layout gave the button no room of
  its own, and a long author's name ran under it and out of the window.
  `a_long_name_under_a_gallery_theme_is_cut_short_before_its_button`.
- **Zooming stops at the fitting size only on the way through it**, never whenever near it: a slow
  pinch changes the zoom by a hair each frame, and stayed fitted.
- **The keyboard shrinks the page in the browser**: `interactive-widget=resizes-content` in
  `web/index.html`'s viewport, so Android gives the canvas the height above the keyboard (Safari
  is not supported, §7), and the caret is followed up into it.
- **eframe's hidden text field shows nothing.** In the browser, typing goes through an `<input>`
  eframe puts beside the canvas and moves to the caret (the editor's `IMEOutput`). It is 1 px with
  a transparent caret, but its padding leaves 5×3 px, and it keeps what was typed (only another
  kind of input clears it), so the end of the text showed there as a short line after the last
  letter. `web/index.html` makes its text transparent (`canvas ~ input`). When touching it: an
  eframe update that styles the field differently, or a page that moves the canvas into another
  element, is checked by `smoke.mjs`'s "the hidden text field at the caret shows nothing of what
  was typed".
- **The keyboard stays up while the writer writes** (`hold_keyboard`, at the end of `frame`). The
  story asks for text input only while the editable page draws its caret, and eframe hides a
  phone's keyboard in any frame without that ask, so a page sliding in or turning dropped it and
  brought it back. The story's last ask is repeated in frames without one; never while a field in
  a window has the keys (`egui_wants_keyboard_input`), whose own ask is not kept. Anything new that
  draws the pages without the editable one is covered by it.
- **On a touch screen the keyboard comes with a tap on the page and stays away once put away.** The
  story asks only while `App::writing`: from a tap on the page (the editor's `clicked`) or letters
  typed, until the window is taller again by much at the same width (the keyboard put away, seen
  through `interactive-widget`; a phone turned round changes the width and keeps it). Asked all the
  time, eframe gave its text field the focus again at the end of every touch (for iOS), so a
  button or a pinch brought the keyboard back however often it was put away. Without a touch
  screen nothing changes. A new way to begin writing on the page sets `writing` too.
- `App::origin` starts far down, so a page taller than the view first rests with its top in sight.
- Headless Chromium cannot show this at a phone's pixel ratio: its emulation leaves
  `devicePixelContentBoxSize` in CSS pixels, so eframe draws everything 2.6 times too big. Look at
  phone layouts at ratio 1 (412×839): egui lays out by points, so that is what a phone shows.
- Tests: `on_a_phone_on_its_side_the_page_fills_the_width_and_the_docks_go_away_while_writing`,
  `on_a_phone_the_page_fills_the_width_scrolls_and_keeps_the_caret_in_sight` (fitted width,
  the top first, the caret followed down the page and above a keyboard, two fingers panning),
  `on_a_phone_the_keyboard_stays_up_while_a_new_page_comes_in_and_a_menu_is_open`,
  `on_a_touch_screen_the_keyboard_comes_with_a_tap_on_the_page_and_stays_away_once_put_away`,
  `on_a_touch_screen_a_pinch_zooms_without_turning_a_page_or_selecting_whichever_way_it_goes`,
  `on_a_touch_screen_one_finger_swiped_sideways_turns_one_page_and_leaves_the_caret_and_keyboard`,
  `on_a_touch_screen_one_finger_taps_the_caret_in_held_selects_and_moves_the_page_up_and_down`,
  `on_a_phone_the_question_before_leaving_and_the_windows_show_whole`.

---

### R33 — Stories online and scenes: who comes only from the session, and nothing reaches R2 or Claude the limits forbid

Signing in and keeping stories online are the Worker's (`web/api/worker.js`); every other path is
a static file, served without it (`run_worker_first: ["/api/*"]`). Plan and reasons: `CLOUD_PLAN.md`.

- **Who is asking comes only from the session cookie**, an HMAC of what it holds with the Worker
  secret `SESSION_SECRET`; without a long enough secret nothing is signed or trusted (500, never a
  guess). A user is `sha256("<provider>:<their id>")`, cut to 32 hex: the provider's id is not kept.
- **Where a story is comes only from that user and the story's id**, which must match
  `^[0-9a-z-]{1,64}$`: `ledger::key` is the only place an R2 key is made. Anything new kept in R2
  is keyed under `users/<user>/` the same way, and listed and deleted with the user's stories.
- **Only the app may ask:** every story request has `X-Caprice: 1`, every change an `Origin` equal
  to our own; no CORS headers, ever. Sign-in's `state` and PKCE verifier are in a signed cookie
  that lasts ten minutes. Sign-in asks for the public profile only, never the email.
- **The Ledger says yes before R2 is touched.** R2 bills past its free amounts; Workers and
  Durable Objects on the free plan stop instead. So every write and read of R2 is first allowed by
  the Ledger (`begin` before `put`, then `commit`; `current` before `get`), which counts it
  against the limits in `wrangler.jsonc` (`vars`). A new kind of R2 request needs its own count
  there. Deleting is free and is never refused. The Ledger is one object for the whole site, so
  its counts and the current versions are consistent; it keeps no story's text.
- **No copy is overwritten unseen:** a save names the version it replaces (`If-Match`) or says it
  is new (`If-None-Match: *`); otherwise 412 or 428. Every save is a new object; the Ledger keeps
  the version before and older ones `HISTORY_APART_HOURS` apart, `HISTORY` in all, and hands the
  Worker the rest to delete. A writer short of room loses their oldest history first.
- **Only real stories are kept**: a body must begin like a `.caprice` file (`{` then `"version"`),
  and no story is readable by anyone but its owner (no sharing links).
- **The tests run in the Workers runtime** (Miniflare, with the date in `wrangler.jsonc`), signing
  in against stand-in providers; each limit refuses, and the ones on the bill leave R2 alone.

In the app (`cloud.rs`) **the shelf is where a story is; online is a copy, and no words are lost.**

- **Nothing is sent unless the writer signed in here**: at start the app asks `/api/me` only if
  `cloud.json` says a sign-in was begun, or who was signed in. The session cookie is HttpOnly, so
  the page cannot see it; `cloud.json` is how it knows to ask. Signing out forgets `cloud.json`.
- **Signed in stays signed in when the server cannot be reached; online backup is off, and
  says so.** With no network, a limit (the day's requests used up, R34) or a fault of the
  server's, the writer still sees their name and writes and begins stories as ever; only nothing
  goes up or comes down. Both sides keep to it:
  - The Worker signs the writer in even when the Ledger cannot be reached (`callback`): the
    session then says the sign-up is still to be recorded (`k`), and the next request records it
    first (`ledger.signIn`, so the day's limit on new accounts still holds) and renews the cookie
    without it. `/api/me` needs no Ledger, so the app learns the name at once.
  - The app keeps the name and provider of who signed in last in `cloud.json` (`Kept`), and when
    `/api/me` gets no answer it takes them from there. `Cloud::pause` notes why (`why_off`: no
    connection, the server's own words for a limit, or a fault) and asks nothing until it lifts,
    `/api/me` included (asking it again at once asked in every frame with no network).
    `Cloud::backup_off` is that note while signed in: the Stories online window says "Online
    backup is off for now", why, and what it means (`BACKUP_OFF`); the welcome page's line says it
    instead of "kept online too"; the status line says when it goes off and when it is back.
  - Tests: `with_no_connection_the_writer_stays_signed_in_writes_on_and_is_told_backup_is_off`,
    `a_limit_pauses_stories_online_and_they_go_up_once_it_lifts`; in `api.test.mjs`, "with the
    ledger out of reach the writer still signs in and is named, and is recorded once it is back",
    "a session still to be recorded cannot pass the day's limit on new accounts" (`ledger-down.mjs`,
    a Ledger the tests can make fail as Cloudflare's does).
- **Saving never waits for the network.** `save_to` calls `Cloud::saved`, and the story goes up
  `UP_AFTER` later; going out of sight or coming back looks at once; otherwise every `LOOK_EVERY`.
- **What to do is `decide`**, from the shelf's file (its fingerprint, and whether it is the open
  story with changes not saved), the record of when the two sides last agreed, and the version
  online. **There is one story, the same in every browser.** Changed on both sides, or never
  agreed and different, it is settled (`settle`): the newer is the story, here and online, and the
  other's words are kept among its backups for a week (R22), out of sight. Newer is by when each
  was saved: online, the server's `modified` (in the list, `Online::modified`); here, when it was
  last saved on the shelf (`Kept::changed`, by `Cloud::saved`), or now for words being typed (which
  are saved at once when they win). If the backup cannot be written nothing changes, and the next
  round tries again: no word is ever replaced unkept. The writer is told in the status line. It
  once kept both, the online copy beside it as "<name> (from another browser)", a story of its own:
  every clash left another story in every browser's list. A story only on one
  side goes to the other, even if it was deleted there, **unless it was deleted on purpose**: a
  story deleted here (`delete_story`) is named in `cloud.json` (`deleting`, with the version it
  agreed with) until `DELETE` has gone, and is neither sent up nor brought down meanwhile. The
  Ledger then lists it as gone (`GET /api/stories?gone=1`, `GONE_DAYS`), with that version, and
  another device whose copy is that version, unchanged since, deletes its own (`Do::Delete`); one
  written in since puts it back, and an open one waits until it is closed. A story changed online
  before the delete arrives (412) is kept, and comes down again. Something new that writes stories on the
  shelf needs nothing more: a changed file is found by its fingerprint (`fingerprint`, stable in
  every browser), not by being told.
- **Each story says how it stands online**, while signed in (`Cloud::mark_of`, `cloud::Mark`): a
  drawn mark and its word after its name on the welcome page, and the word in the shelf's Open
  window. Online (a cloud with a tick: the record's fingerprint is the file's), going online (a
  cloud with an arrow: saved since, and it goes up by itself), or only in this browser (a screen, in
  the accent: online backup is off, or the story is too big; the hover says why and that it goes
  up by itself). The mark is worked out from what the app knows, never by asking the
  server, so it shows offline too; the welcome page takes the fingerprint from its list
  (`Story::hash`) rather than reading every file each frame. Going online stays automatic; the
  Stories online window offers "Try again now" while backup is off and "Back up now" while a save
  waits (`Cloud::try_now`). Anything new that changes whether a story is online (a new `Do`, a new
  refusal) must leave `mark_of` right. Tests:
  `each_story_on_the_welcome_page_says_whether_it_is_online_or_only_in_this_browser`,
  `a_story_changed_here_and_in_another_browser_is_kept_once_as_the_newer_and_the_other_is_a_backup`.
- **A story opened is brought up to date before it can be written in** (`Cloud::checking`, in
  `App::can_edit`). Opened (any way `App::path` changes to a story with a record), it asks online
  at once, and editing is off, with "Looking for a newer copy online…" (`live_bar`), until the list
  is planned and a newer copy, if any, has come down, at most `CHECK_FOR` seconds, and not at all
  while backup is off. Opened from a browser's older copy and written in at once, it was changed
  in two places, and the newer, the few words typed, replaced the story written elsewhere. Test:
  `a_story_opened_with_a_newer_copy_online_is_brought_up_to_date_before_it_can_be_written_in`.
- **Copies an older Caprice kept beside a story are folded back** (`fold_copies`, each `plan`):
  "<name> (from another browser)" or "(written here)", perhaps numbered, whose story is on the
  shelf, becomes a backup of it and is deleted here and online (`Cloud::deleted`), so other
  browsers drop it too. One open, or whose story is gone, stays. Test:
  `a_copy_an_older_caprice_kept_beside_a_story_becomes_a_backup_of_it`.
- **A save whose answer was lost is this browser's own.** A page frozen or gone in the background
  (a phone) may never hear the answer to a `PUT` that the server took: the record still names the
  version before, the version online is newer, and the shelf's file may have changed since, which
  `decide` reads as changed on both sides. So what goes up is written down first (`Kept::sending`,
  in `cloud.json`, its fingerprint by key), and a story compared (`Task::Compare`) whose copy online
  is what this browser sent is agreed on, not settled; what was written since goes up over it.
  Test: `a_story_saved_online_whose_answer_was_lost_is_not_kept_twice`.
- **Two copies are compared as stories, not as files** (`same_story`): what each browser keeps
  for itself in the file (the scene being described and whether pictures show, R34; the garden
  older files held, R30) is left out, and when only that differs, this browser's file is agreed on
  as it is (its own fingerprint recorded, so it is not sent up and fetched back by the other).
  Compared byte for byte, a story open in two browsers was settled without a word differing.
  Anything new a file holds that is a browser's own goes into `same_story` too. Test:
  `a_story_that_differs_in_another_browser_only_in_what_each_keeps_for_itself_is_kept_once`.
- **The open story is never written over** while it has changes not saved: the copy from online
  goes beside it. Without such changes it is written and shown again (`Cloud::reload`), the caret
  kept, its page shown with `go_to_page` (R5). **Its scenes stay on screen**: `adopt` takes the
  drawings already rendered (`Backdrop::take_drawings`, by a fingerprint of the SVG, not the
  scene's id) and gives them to the story shown next (`give_drawings`). Rendered again, a scene
  took seconds on a phone, with the page blank behind the text meanwhile. Something new kept
  rendered per scene (in `Backdrop`) goes through them too
  (`a_story_shown_again_from_online_keeps_its_scene_on_screen`).
- **A story online is named by its id and its file's name together** (`cloud::key`): "Save as"
  keeps the id, so two stories on a shelf may share one. The record keeps the path, so a story
  brought down under another name (`free_name`) keeps its name online.
- **A story that comes down new to this browser is among the stories last opened**
  (`Start::remember`, the welcome's list read again): a new browser is otherwise an empty welcome
  page, and the writer thinks their stories are lost.
  The welcome greets whoever is signed in by their first name. Signed out, it offers Google's
  button under the greeting, which goes straight to Google (`App::sign_in`, saving first, as the
  window's does) and asks the server nothing on the way.
- **Requests go through `cloud::Server`**: `platform::send` (the page's `fetch`) in the browser, a
  stand-in (`cloud::stand_in`) in tests; tests never reach a server, as they never reach printers.

**Scenes are painted for the browser by the Worker, and the day's scenes bound the bill**
(`/api/scene`, `scene` in `worker.js`). Claude bills each painting, so this is the one request
that costs money as it is asked:

- **The Ledger counts a scene before Claude is asked** (`Ledger::paint`): `SITE_SCENES_PER_DAY`
  (10 for now in `wrangler.jsonc`, 50 later) for the whole site together, however many a single writer takes, and one at a time for each
  writer (a second click would cost another). A painting counts whether or not a picture comes of
  it (declined, cut off, no SVG): Claude bills the asking, so the count is a count of what is
  billed. The Ledger is one object, so two writers asking for the last scene at once cannot both
  have it. A new request that has Claude do something needs its own count there, before the call.
- **Painting goes on without the Ledger, and the day's scenes still bound it** (`tally.js`). Once
  Cloudflare's daily Durable Object requests are used up (R34) the Ledger answers nothing, but a
  scene needs only Claude. So each scene the Ledger counts is copied to one small object in R2
  (`noted`), and while the Ledger throws `OVER_LIMIT` the scene is counted there alone (`tally`),
  from where the Ledger's count stopped, with R2's conditional writes (`etagMatches`, or
  `etagDoesNotMatch: "*"` for the first) in place of the Ledger's one call at a time: a count
  changed meanwhile is read again. Nothing else is asked of the Ledger then: not the one painting
  at a time for each writer, not `keepPainted` (the picture comes back at once, unkept), not the
  hashed counts. A session still to be recorded (`k`, signed in without the Ledger) paints too,
  and is recorded later. Anything new in `scene` that asks the Ledger goes through
  `unlessOverLimit`, or the picture already paid for is lost to a 503. Test: "with the ledger out
  of reach scenes are still painted, and the day's scenes still bound them".
- **The most one scene costs is the Worker's, never the app's**: `SCENE_PROMPT_BYTES` refuses a
  bigger request before anything is counted, and `SCENE_TOKENS` is `max_tokens` (thinking counts
  in it). With Opus 5.5 ($4 and $20 a million tokens in and out) that is at most about $1.30 a
  scene and $64 a day; the Anthropic Console's spending limit is the net under a bug.
- **The key stays in the Worker** (`ANTHROPIC_API_KEY`, a secret), and only the SVG goes back, so
  it is of no use but for painting. The prompt is the app's (`scene_prompt`), the system prompt the
  Worker's own (`PAINTING`, kept the same as `backdrop::SYSTEM`). The picture says in its
  `<metadata>` that an AI made it, and the panel says what goes to Claude before it goes.
- **A picture comes back able only to draw** (`clean`): scripts, HTML inside it
  (`foreignObject`), `on...` handlers, `javascript:`, `@import` and links outside the picture
  are taken out, again until nothing changes. resvg runs none of them, but a scene saved as
  `.svg` and opened in a browser would, and the story's text in the prompt could ask for them.
- **Claude's answer comes in one piece, not streamed**: waiting costs the Worker no CPU time (10 ms
  a request on the free plan), reading thousands of streamed events would.
- **The prepaid credit used up pauses painting** ("Painting is paused for now", 503): Anthropic's
  answer (402, or "credit balance") is told apart from Claude being busy or failing, and is
  logged with its reason. The day's limit is 10 while the credit is small; 50 later.
- **Who painted how many is kept by hashed id only** (`scene_days`, `SCENE_DAYS` days), for the
  site's owner to see whether one account uses up the scenes: `GET /api/admin/scenes` with
  `Authorization: Bearer <ADMIN_TOKEN>` (a secret of 32 characters or more; without it there is no
  such request). It names no one: no name or email is kept anywhere. Deleting a writer's stories
  online deletes their counts too, but not the day's total.
- **A finished picture waits in the Ledger until the app has it** (`keepPainted`, `painted`):
  `PAINTED_HOURS` (48) at most, `PAINTED_PER_USER` (20), the oldest first, with only the story's
  id and the scene's char, no text of the story. `GET /api/scenes?story=` lists them for the
  writer, `DELETE /api/scenes/<id>` forgets one; deleting the stories online deletes them too.
- `SCENES_OFF = "1"` stops painting at once; without a key nothing is painted. Neither touches
  stories online, nor `SYNC_OFF` painting.
- The app reads `X-Caprice-Scenes-Left` (`platform::READ` names it, or the page cannot see it) and
  the Scene menu says how many are left today.

**Theme programs are written by Claude through the Worker, a round at a time, and never as a way to
ask Claude anything else** (`/api/theme-program`, `web/api/theme.js`, R36, the plan's step 5):

- **The Worker writes the whole prompt** (`SYSTEM`): the guide written for Claude
  (`theme-sdk/GUIDE.md`: what a theme is, R30's rules in plain words, laying out, drawing, the
  budget of points, how to answer), the SDK's source, the WIT world, and the fireflies, the Snow
  and the Vineyard as examples, imported as text from where they are written (wrangler's `rules`;
  the tests' Miniflare loads them from the checkout's root). So **a change to the SDK, its world
  or a ported theme changes the prompt too**: keep `GUIDE.md` true to it.
- **A round is one of a few kinds, each capped in size and number**: begun (the writer's request,
  `THEME_REQUEST_BYTES`), `errors` (the compiler's words, `THEME_ERRORS_BYTES`), `checks` (what
  Caprice's checks found of a program that built, in Caprice's words, as large; with `errors` at
  most `THEME_ERRORS_ROUNDS` a theme, a fix being a fix whichever kind), `frames` (the preview's PNGs as bytes, checked to be PNGs of a
  desk's size, never decoded, made base64 here, with their moments and points; `THEME_FRAMES_BYTES`,
  `THEME_FRAMES_ROUNDS`), `change` (the writer's words, `THEME_CHANGE_ROUNDS`). The writer's words
  are one field in a sentence the Worker wrote. Anything else is refused before Claude is asked.
- **The conversation is the Ledger's** (`theme_talks`, `theme_turns`, `themeRound`,
  `themeAnswered`): each answer kept whole, so the browser cannot write Claude's side; pictures kept
  as words only; one round of a theme at a time; forgotten after `THEME_DAYS`. A theme is the
  writer's who began it: anyone else's round is 404.
- **The Ledger counts every round before Claude is asked**: `SITE_THEME_ROUNDS_PER_DAY` for
  everyone, `USER_THEMES_PER_DAY` begun and `USER_THEME_ROUNDS_PER_DAY` for one writer. A round
  counts whether or not a theme comes of it. `THEME_TOKENS` is `max_tokens`; the prompt (about
  40 000 tokens) is cached, and the conversation with it (`cache_control`).
- **What each theme cost is kept, and whether it was kept** (`theme_costs`, `cost`, `themeKept`,
  the owner's call of 2026-11-19: free at first, the numbers gathered for a price later). Every
  round Claude answered adds its tokens and their price in millionths of a dollar (`PRICES`, the
  writer's: change it with the model) to its theme, a theme or not; `POST
  /api/theme-program/<id>/kept`, from the app when the writer keeps it, marks it once, for its
  writer only, asking nothing of Claude. These outlive the conversation (`THEME_DAYS`); deleting a
  writer's stories online takes their name off them, not the numbers. The owner reads them a day at
  a time with `GET /api/admin/themes` and `ADMIN_TOKEN`, as `/api/admin/scenes`: begun, kept,
  rounds, tokens, dollars, those of themes kept, and the cost of all per theme kept. Billing will
  hang on the themes kept.
- **Only the program's source and its palette go back** (`read`): the last ```rust block (it
  must name `theme!(`), and from the ```json block the name, a few words and the colours by the
  palette's names as `#rrggbb`; never Claude's own words. A reply without a program is 502.
- The model is named once (`WRITER`, `claude-opus-5-5`); thinking adaptive at `THEME_EFFORT`
  (`medium` in `wrangler.jsonc`, measured to fit `THEME_TOKENS`; `high` unless set); a model's refusal goes on with the platform's fallback
  (`server-side-fallback-2026-07-01`, `fallbacks: "default"`). Not streamed, as a scene is not.
  `THEMES_OFF = "1"` stops it; without a key there is none.
- **A round not answered within a beat is kept alive** (`keptAlive`, `BEAT_MS` 15 s,
  `THEME_BEAT_MS` in tests): answered at once with 200, a space sent every beat while Claude writes,
  then the one JSON object (JSON may begin with spaces): the theme, or what went wrong as
  `{ error, message, retry }`, since the status was sent before it was known. A round answered
  within the beat keeps its status. The window counts the spaces as word that it is still at work
  (`making::LOST`: a minute without one is a connection lost), so **a change to the beat changes
  `QUIET` and `LOST` in `making.rs` too**. The answer is `Cache-Control: no-store, no-transform`
  and `text/event-stream`, which `worker.js`'s `guarded` keeps, so Cloudflare does not hold the
  spaces back to compress them. **What a writer may read** (the messages here and the Ledger's
  `themeRound`) never names Claude: it says what happened and what to do.

Tests: "a theme is begun from the writer's words, and only its source and palette come back", "what
each theme costs is kept with it, and the owner sees it a day at a time with how many were kept", "the
next round tells claude the compiler's words, with the whole conversation kept by the worker",
"what caprice's checks found of a program that built goes to claude as a fix, counted with the
compiler's", "pictures go to claude as images, and are kept in the conversation as words only", "what is not one
of its kinds, too big, or not pictures is refused before claude is asked", "rounds of each kind run
out for a theme, and themes for a writer, and claude is not asked again", "the site's rounds for the
day run out for everyone, and a round that fails still counts", "without a key, or switched off, no
theme is made", "a round that takes longer than a beat is answered at once, a space every beat, then
the theme or why not".

The gallery of theme programs (`web/api/gallery.js`, R36, the plan's step 6) is for writers signed in:

- **A theme sent is checked as far as is cheap, and queued for the owner**: its file (a zip, at most
  `GALLERY_FILE_BYTES`) is unpacked in the Worker (`theme-file.js`, which imports no text so that
  `smoke.mjs` can run it on what a real tab sends: stored or deflated, each part capped);
  its manifest's world, name, toolchain and colours, its module (`wasm-check.js`, R36's check
  ported: only `host` imported, the world's exports, sizes bounded), its source (UTF-8, naming
  `theme!(`) and its picture (a PNG 320 wide) must hold, or 422 with a sentence. The Worker works
  out the module's and source's SHA-256 itself; nothing the writer's browser says of them is kept.
  `USER_PUBLISHED_PER_DAY` a writer, `GALLERY_WAITING` in the queue, `GALLERY_BYTES` in all; the
  Ledger says yes and counts two writes before R2 is touched (`galleryPublish`, then
  `galleryCommit`), as for stories. Its files are under `gallery/<id>/`, the one R2 key outside
  `users/`: a theme published is the gallery's, not its writer's, and stays when they delete their
  stories online; one still waiting, or refused, goes with them (`removeAll`).
- **The owner is the session's user named in `GALLERY_OWNERS`** (a dashboard var, hashed ids,
  never in the repo); `/api/me` says `owner` to them only, and `/api/gallery/waiting` is 404 to
  anyone else, as is a waiting theme's file.
- **Only the owner's yes signs, over what the Worker worked out**: the decision names the module's
  SHA-256 the owner's build gave, which must be the one kept (409 otherwise); the Worker signs
  `Signed::message` with the secret `GALLERY_SIGNING_KEY` (an Ed25519 seed, base64), whose public
  half is `own::GALLERY_KEY`. Without the secret nothing is signed (503). A refusal says why to the
  writer (`/api/gallery/mine`) and its files are deleted.
- **Only the owner takes a published theme out again** (`DELETE /api/gallery/<id>`, 404 to anyone
  else, 404 for a theme not in the gallery): the Ledger marks it `removed` first (`galleryRemove`:
  listed and read no more, its room given back, its writer told "Taken out of the gallery" under
  `/api/gallery/mine`), then its files are deleted, so it is never listed with its files gone.
  Deleting its files in R2 by hand leaves it listed, its "Use" broken: the list is the Ledger's
  table, not R2's (seen live, 2026-11-20). A writer who took it keeps it: the signature on their
  shelf stays good. Sent again, it waits for the owner as a new theme.
- **Reading the gallery is counted**: the list, and each file, against `SITE_READS_PER_DAY`
  (`galleryRead`); a theme published never changes, so its files go back `private, max-age=31536000,
  immutable` (the one Cache-Control `guarded` keeps) and the browser asks once.
- `GALLERY_OFF = "1"` stops sending.

Tests: "a theme sent is queued for the owner and seen by no one else", "what is not a theme the
gallery can take is refused before anything is kept", "the owner's yes is signed by the worker over
what it worked out, and the theme is everyone's", "a theme the owner refuses says why to its writer,
and its files go", "the owner takes a published theme out of the gallery: listed and read no more,
its files gone, its writer told", "a writer sends a few themes a day, the queue is bounded, and without a key
nothing is signed", "deleting a writer's stories online takes their themes still waiting, and
leaves the gallery's".

Tests (`node --test web/api/api.test.mjs`): "without a session, or with a forged one,
nothing is answered", "one writer cannot reach another's stories", "ids that could reach elsewhere
are refused", "requests from another site are refused", "a sign-in not started here is refused",
"a save must say which copy it replaces, and a stale one is refused", "the site's writes for the
day pause everyone, and leave r2 alone", "the site's room is shared by everyone", "a writer's room
drops their oldest history first, then refuses", "only caprice stories, within the size limit, are
kept", "a story deleted is listed as gone for the writer's other devices, until it is saved
again", "the off switch stops stories online at once", "a scene is painted for a writer signed in, and only
the picture comes back", "the site's scenes for the day run out for everyone, and claude is not
asked again", "a painting that fails or is declined still counts, since claude was asked", "a
writer paints one scene at a time", "too much to paint is refused before anything is counted",
"without a key, or switched off, no scene is painted", "when the credit is used up, painting is
paused, and claude being down is said so", "the site's owner sees the scenes of each day by hashed
id, and nobody else does", "a picture comes back without anything that could run or reach outside
it", "a picture painted waits for the app until it has it, and only for its writer", "pictures
waiting are few for each writer, and go with the stories online". In the app: the `cloud::tests` (`decide`, names,
keys), `in_the_browser_nothing_is_asked_of_the_server_until_the_writer_signs_in`,
`a_story_saved_goes_online_a_little_later`, `a_story_newer_online_comes_down_onto_the_shelf_and_opens`,
`the_open_story_changed_in_another_browser_is_shown_as_it_is_there`,
`a_story_changed_here_and_in_another_browser_is_kept_twice_and_loses_no_word`,
`words_typed_and_not_yet_saved_are_never_written_over_by_a_story_from_online`,
`a_limit_pauses_stories_online_and_they_go_up_once_it_lifts`,
`signing_out_keeps_the_stories_in_this_browser_and_sends_nothing_more`,
`a_story_deleted_on_the_welcome_page_goes_here_online_and_on_the_writer_s_other_devices`,
`a_story_deleted_on_another_device_and_written_in_here_meanwhile_is_kept`,
`deleting_the_stories_online_signs_out_and_leaves_them_here`,
`in_a_new_browser_the_stories_from_online_are_on_the_welcome_page`,
`signed_in_the_welcome_greets_the_writer_by_their_first_name`,
`the_welcome_s_google_button_goes_straight_to_google_s_sign_in`,
`in_the_browser_a_scene_described_is_painted_by_caprice_s_server`,
`in_the_browser_signed_out_no_scene_is_painted_and_nothing_is_asked`,
`when_the_day_s_scenes_are_used_up_the_writer_is_told_and_nothing_is_painted`; and in `web/smoke.mjs`, that nothing
is asked signed out, and the app's header on what is.

---

### R34 — One pen per story: one device writes, the others follow, nothing is merged

A story on the shelf, open on several devices of a writer signed in, meets in its room
(`web/api/room.js`, `StoryRoom`, named from the session's user and the story's key online, so only
that writer's devices meet there). One device holds the pen; the others follow and cannot edit.
Plan and reasons: `LIVE_PLAN.md`.

- **Every change of the text goes through `Doc::apply_raw`** (typed, grouped, undone, redone), so
  that is where the holder gathers what to send (`Doc::outbox`). It sends a batch at most every
  `BATCH_EVERY` (a second), numbered, with `App::live_hash` after it; the room passes it on and
  refuses one from a follower or out of order.
- **A follower applies a batch only as it was made** (`Doc::apply_live`: the old text must be
  there, nothing touches the final mark), with no undo, the pages worked out from the edit (R3),
  its caret and selection moved with the text. Anything else (a gap, an edit that does not fit,
  a hash that differs) and it asks for the whole story: the holder saves, it goes up (R33), and
  the follower fetches that version (`take_whole`). So **what changes the story outside
  `Doc::apply` needs nothing more**: page settings, notes, boxes, the cast and scenes reach the
  followers that way. `live_hash` is the document only (`DocFile::from_doc`, no garden): what
  is one device's own (the scene being described, Show pictures) must stay out of it, or the
  devices would never agree.
- **Following, the story is the holder's**: editing is off (`App::can_edit`), and nothing is
  saved, kept as a version or sent online from here (`keep_safe`, `save_before_leaving`,
  `page_hidden`, and `Cloud::skip` in `plan`). Its `saved_hash` follows each batch, so it is
  never "unsaved" and leaving asks nothing. Editing is off in three layers:
  - every function that changes the story returns at once (`replace_raw`, `apply_edit`, `paste`,
    `apply_style`, `set_para`, `set_setup`, `step_history`, the pictures', `add_note`,
    `insert_text_box`, `set_band`, replacing, going back to a version);
  - what reaches them is greyed or held back: `process_input` keeps only events that read
    (`reads_only`), `apply_tools` only finding, turning, printing and other stories, the toolbar's
    groups, the Claude menu and `can_paint` are off, notes, the scratchpad and the cast are read
    only, text boxes take no pointer, the scene list only goes to a scene;
  - and a net: after a frame with a click or a key, a story that changed all the same is replaced
    by the holder's (`live_check`).
  **Anything new that changes the story must return early without `can_edit`**, in the
  function itself rather than only in the button that calls it.
- **Transfer here** (`take`): the holder sends its last batch, then gives the pen (`given`), or
  loses it after `LIVE_HAND_OVER_MS`. A device that takes the pen of a story it followed clears
  its undo and first learns the version online (`take_claim`, `Cloud::agree`), so its saving
  goes up over that version, not settled as a story changed in two places (R33). It writes at
  its own caret, where its reader was.
- **A device gone** keeps the pen for `LIVE_FREE_AFTER_MS` (30 s), then the first device there
  has it. A device that comes in to follow asks for the whole story at once: its shelf's copy is
  older than the holder's. A follower that hears nothing back asks again after 30 s.
  **A device that left frees it at once**: its socket closed by the page, 1000 when the story is
  closed (`live_close`), 1001 or 1000 when the page goes (`pagehide`; browsers close a page's
  sockets with 1001). Only out of reach (1006, no close at all) does the room wait. Every tab
  is a new device, so a tab opened just after one was closed followed the closed one for half
  a minute, "Open for writing on another device" with no other device. The page that loads
  again to come straight back (`again.rs`, R31) keeps its device id and closes with 4003
  (`live_back_soon`), so its pen waits for it. As the pen passes the moment a story is left,
  the holder sends what it typed in the last second before leaving (`live_leaving`, from
  `save_before_leaving`). Tests: "a writer whose page or story closed frees the pen at once;
  one loading again keeps it",
  `a_story_closed_where_it_was_written_goes_on_on_the_other_device_with_its_last_words`.
- **Out of reach, a device stays as it was** (a tab in the background, a phone asleep, the
  network): its socket closing is not leaving the room. Following, it stays locked
  (`LOCKED_WHILE_LOST`, as long as the room keeps a pen) with "Connecting again…"; holding, it
  writes on, its edits waiting in the outbox for when it is back. It tries again at once, then
  every `AGAIN_AFTER`. Unlocked at once, as it first was, it was written in while it heard
  nothing, and coming back the holder's story replaced those words. **Words the room never had
  are kept among the story's backups** (`keep_unsent`, `keep_backup`, a week, R22, R33) before a
  device follows again; they were once a story beside it, "… (written here)".
  Going out of sight, the holder sends what it typed in the last second (`live_hidden`, from
  `App::logic`), as no frame may come for a long time.
- **The room costs requests, out of 100,000 a day for the whole site on the free plan** (stories
  online included): each message in is one, pings are answered without waking it, and it writes
  almost nothing (the batch number goes with the holder's socket, the day's count twenty at a
  time). `SITE_LIVE_PER_DAY` and `ROOM_MESSAGES_PER_DAY` bound it; `LIVE_OFF = "1"` stops it.
  **Live editing has its own share of the day** (`SITE_LIVE_MESSAGES_PER_DAY`, 40,000): a room
  passes on only messages the Ledger gave it (`Ledger::liveMessages`, `LIVE_MESSAGES_AT_ONCE` at a
  time, `StoryRoom::allowed`), so the rest stays for stories online. Out of share, or the Ledger
  out of reach, the room closes its sockets (4002). Asking the Ledger lets the next message in
  meanwhile, so messages are handled one after another (`webSocketMessage` chains `message`),
  or a writer's batches could pass each other. Test: "live editing has its share of the site's
  requests, and stories online keep the rest".
  **Its alarm is a request too**, and counts against neither: a room wakes only to hand the pen
  to a device there (`settle`), never in an empty room, and never for a time already past. An
  empty room with its holder gone once set an alarm in the past for ever, and two such rooms
  used up the day's 100,000 in a morning.
- **Once the day's 100,000 are used up, the writer is told so** (`web/api/failure.js`). Cloudflare
  then refuses every Durable Object request until midnight UTC, and the Worker's catch-all answered
  "Something went wrong", which looked like a broken sign-in. Its error (`OVER_LIMIT`) now answers
  503 saying what happened and when it lifts, with `Retry-After` until midnight UTC, so the app
  pauses stories online that long and shows the words (online backup is off, R33). Signing in and
  painting scenes still work then (R33, `tally.js`), so the words name only stories online; the
  status line wraps them to the window and shows a long message longer (`update_title_and_toast`,
  test `a_long_message_is_shown_whole_in_a_narrow_window_and_stays_long_enough_to_read`). `/api/me` needs no Durable Object and
  still answers. Any other fault still says only "Something went wrong". Test:
  "cloudflare's daily limit is said plainly, with when it lifts, and other faults say only that".
- **Only the writer's own devices**: the WebSocket cannot carry `X-Caprice`, so `connect` checks
  its `Origin`, then the session; the room hears who from the Worker, never from the device, and
  refuses a socket whose session has run out.
- The browser's side is `platform::live` (a WebSocket per room, `PageSocket`) and
  `platform::device_name`; tests use `cloud::stand_in`'s rooms, with no time in them.

Tests: in `api.test.mjs`, "the first device holds the pen, the others follow, and every edit
reaches them", "a follower cannot write, and a batch out of order is refused", "only the writer's
own devices meet in a story's room", "transfer here: the writer sends its last batch, gives the
pen, and everyone is told", "a writer that does not answer loses the pen after the hand-over
time", "a writer gone for a while frees the pen; back sooner, it keeps it", "a follower that lost
its way has the writer save, and is told what to fetch", "a story's messages for the day run out,
and too big a message is not taken", "the site's live connections for the day run out, and the
off switch stops them", "a room everyone left sleeps until a device comes back, however long the
pen was gone". In the app:
`a_story_open_on_two_devices_is_written_on_one_and_followed_live_on_the_other`,
`a_change_that_is_no_edit_reaches_the_following_device_as_the_whole_story`,
`a_following_device_changes_nothing_with_its_tools_notes_or_scratchpad`,
`a_following_device_out_of_reach_for_a_moment_stays_locked_and_follows_again`,
`words_written_where_the_pen_was_lost_are_kept_as_a_backup`,
`transfer_here_moves_the_pen_and_the_other_device_follows_what_is_written_here`.

### R35 — A theme of one's own is one drawing, checked when it comes in, played like a magic theme

A writer can make a theme (`own.rs`): one SVG with a group `desk` (the picture the pages lie on,
which must move), optionally `piece` (or `piece-1`, `piece-2`, …: what grows a stage a page beside
the pages, a piece a chapter, with its `done`) and `carrier` (what brings a new chapter title, at its
`hook`), and its settings as JSON in `<metadata id="caprice">` (`Meta`). Caprice plays it; nothing
in it is code. The guide in the app (`own::guide`, `MOTIONS`) and the comment at the top of
`assets/theme-template.svg` say all of it to the people (and AI chats) who make one.

- **A drawing is made readable once, when it comes in** (`normalize`): a role named by an id, by
  Illustrator's escaped id (`move_x2D_drift_x2D_1`), by Figma's `data-name` or by Inkscape's layer
  name (`inkscape:label`) becomes that id (`plain_name`, `role`); a moving part's name used twice
  gets a number of its own (or one part would move twice); every `href` leading out of the file is
  taken away, so nothing of a theme reaches outside it (a path on this machine, a server). What is
  kept on the shelf is that drawing, so everything after reads ids only. Tests:
  `names_from_inkscape_illustrator_and_figma_are_read_alike`,
  `links_out_of_the_file_are_taken_away_and_text_is_noted`.
- **The desk must move, and a refusal says why** (`check`): fewer than `LEAST_MOVING` parts that
  move is refused, as is a drawing without a desk, one usvg cannot read, or one too large
  (`MOST_BYTES`, `MOST_ELEMENTS`). What would spoil it without refusing it (text, which has no fonts
  here; parts marked to move that stay still; all the movement behind the page; a tall drawing) is
  noted. Both go in the import's window in plain words (`Report`). Tests:
  `a_drawing_whose_desk_does_not_move_is_refused_saying_why`,
  `a_drawing_whose_desk_does_not_move_is_not_added_and_a_window_says_why`,
  `the_template_is_a_theme_with_everything_in_it` (no notes at all: the template must stay clean).
- **Parts are cut out by hiding the rest, never by taking it away or `display="none"`** (`cut`,
  `hide`): a group becomes `<defs>`, so the gradients, filters and clips inside it that other parts
  use stay there; Inkscape's layers carry `style="display:inline"`, which beats a `display`
  attribute. Each stage of a piece is rendered alone in the frame of the whole piece (so stages lie
  where drawn on each other), the carrier without its hook. Test:
  `a_drawing_with_a_desk_that_moves_a_piece_and_a_carrier_is_read_into_its_parts`.
- **Its palette comes from its picture, once, and is kept with it** (`palette_of`, on import; the
  shelf's `Entry::colours`): the desk's own colours at its foot and top, docks of its light or dark,
  the accent its most vivid hue; the colours its maker chose first (and what is made from them,
  such as buttons from a dock), then all of it made readable as R29 asks. Test:
  `the_palette_comes_from_the_picture_and_stays_readable_whatever_it_is`.
- **The shelf is the `themes` folder beside `theme.json`** (`Shelf`: `index.json`, each drawing as
  `<id>.svg` and the gallery's picture as `<id>.png`; in the browser, IndexedDB like any file, R31).
  `Themes::kept_in(None)` keeps it in memory, so `Harness` writes nothing. Deleting asks first, and a
  theme deleted while in use leaves the usual one. Tests:
  `a_theme_put_on_the_shelf_is_there_the_next_time`,
  `a_theme_of_one_s_own_is_imported_chosen_and_kept_for_next_time`.
- **Its picture covers the whole desk, never stretched** (`eggs::own::cover`): as large as needed to
  cover it, cut to fit, held at the edge `Meta::anchor` names (its foot unless said otherwise), and
  a little nearer as the writer zooms in (`NEARER`), about the foot of the desk under the page, so
  it still covers. The pieces stand on its ground (`Growth::ground`, in its units) and scale with
  it, and never below the page bar along the bottom (as R30's plants). Test:
  `a_theme_of_one_s_own_covers_the_whole_desk_whatever_the_window_s_shape`.
- **Nothing of it is read or rendered on the frame** (R31): the drawing is read into its parts
  (`Pack::read`) and each part rendered (`layered`) with `platform::spawn`, at the size it shows
  but no more than `MOST_PIXELS` either way (a browser's GPU takes no larger texture), and again
  only when that size changes by a quarter or halves, at most every 0.4 s while a window is
  resized. Until the desk's picture comes it is the palette's desk, and it comes in over `COME_IN`.
  The browser's check: `smoke.mjs`, "a theme of one's own covers the desk with its picture".
- **The pieces are tended as the Magic Snow's igloos** (a `Tended`, R30): a stage a page of
  `Doc::garden`, each coming as `Growth::arrive` says (out of a little light, falling from above
  and bouncing, or rising from its foot; `piece_shapes`); when the next chapter begins the piece is
  finished at once, the stages it had no pages for coming in with its `done` (as a ship is), so no
  finished piece is half built. Their places come from `Growth::layout` (in rows out from the page,
  or scattered), given once and kept. Test:
  `in_a_theme_of_one_s_own_each_page_grows_a_stage_and_the_next_chapter_finishes_the_piece`.
- **Or a piece stays where it is drawn** (`Layout::Drawn`): one for the whole story, on the
  picture where its maker drew it (`Piece::at`, through the same cover as the desk), a stage for
  every page of the story whatever its chapters (the sum of `Doc::garden`, or the story's pages if
  more: the text before the first title is in no chapter; tended as one, so it never shrinks), its
  `done` with its last stage. This is what a maker means by a coconut on the beach for every page;
  drawn spread out with the beside-the-page layouts it was a strip far wider than tall, scaled to a
  quarter of the desk and off it, so `check` notes a piece far from square there, and a stage
  repeating the one before (`repeated_stage`). Tests:
  `in_a_theme_drawn_in_place_a_stage_falls_where_it_is_drawn_for_every_page_of_the_story`,
  `a_piece_drawn_spread_out_or_with_stages_repeating_the_ones_before_is_noted`.
- **Or each chapter's piece stays where it is drawn** (`Each::Chapter`): the chapter's piece
  (in turn, or at random with `Order::Random`, never the one before's, the same for the same
  story: `piece_for`) is tended as the beside-the-page ones are; when the next chapter begins it
  shows its `done`, or with `Finish::Fall` its stages fall one after another to `ground` and lie
  there for good, about its foot (`fallen`, seeded by chapter and stage). A piece a later chapter
  grows on again is that chapter's: an older one finished with its `done` is not drawn under it.
  What is drawn in place on a moving part of the desk moves with it (`Drawn::holding`,
  `Drawn::moved`): the smallest moving part holding the piece's middle, whether the piece is drawn
  inside that part's group (a piece may lie in the desk) or over it; asked for: coconuts on a
  swaying palm stood still while its crown swayed. Tests:
  `coconuts_grow_on_a_chapter_s_palm_sway_with_it_and_fall_to_the_sand_when_the_next_begins`,
  `what_lies_on_a_moving_part_finds_it_and_moves_with_it`.
- **A point in an id may be in the drawing's units or the part's group's** (`motion::layered`):
  whichever of them lies on the part (in it, for a spin), the drawing's first, so what Claude
  writes for a scene reads as before. A hand-made bird moved into place by a `translate` had its
  wing's shoulder written as 0,0 in it, and the wing stayed still; parts of the piece and the
  carrier that stay still are noted as the desk's are. Test:
  `a_point_may_be_written_in_the_units_of_the_group_its_part_is_drawn_in`.
- **Its carrier flies the balloon's way** (`Carrier::Own`, `rig`), mirrored to face the way it flies
  (`CarrierLook::faces`), and is called only once its picture is rendered
  (`Shown::carrier_ready`). Test: `a_theme_of_one_s_own_brings_a_new_chapter_title_with_its_carrier`.
- **None of it is printed or saved with a story** (R20, R30): it is drawn in none of `App::underlay`,
  `PageLayout::ink` or `App::furniture`, and its pictures keep their own colours (R29). Test:
  `print_shows_nothing_of_a_theme_of_one_s_own`.
- **A theme of one's own may instead be a program** (`Entry::program`), kept on the same shelf
  and chosen the same way, drawn by `eggs/program.rs` rather than `eggs/own.rs` (R36).
- **To add a setting**: a field in `Meta` (or `Growth`, `CarrierLook`) with a default, the guide's
  line, the template's comment. **A new kind of motion**: as in §5, and its line in `own::MOTIONS`
  and the template's comment.

### R36 — A theme that is a program draws only shapes, in a sandbox, a frame behind; Caprice decides everything else

A theme of the writer's own may be a program rather than a drawing (`Entry::program`; the module
kept on the shelf as `<id>.wasm` beside the drawings): one WebAssembly module, built from one Rust
file against `theme-sdk/` (`themes/fireflies/theme.rs` is one), that draws the desk as the magic
themes do. Its interface is the WIT world `caprice:theme@1.0.0` (`theme-sdk/wit/theme.wit`): what
Caprice tells it (`events`: `fresh`, `grown`, `begun`, `clicked`), what it may ask (`host`: the
story's seed, the garden, the page count, whether it carries titles), and the frame it draws
(`frame`: shapes on the desk behind the pages, in the window in front of the desk, and, for its
carrier, over the page). `themes/snow` is the Magic Snow ported to it, shape for shape. The plan behind it is
`THEME_PROGRAMS_PLAN.md`; the decisions measured there are in §7.

- **The world is published once and kept working for good.** A change to `theme.wit` is a new
  version (`@1.1.0`, `@2.0.0`), never an edit of `1.0.0`: a theme built today must run as long as
  Caprice does. The SDK carries the bindings made ahead of time (`theme-sdk/src/bindings.rs`, with
  its own little runtime in `rt.rs` in place of wit-bindgen's: a theme's build is the SDK and one
  file, no proc macro, no other crate). Test: `the_bindings_carried_are_made_again_the_same_from_the_wit`
  (in `theme-sdk/tests/`).
- **Every theme can still become a component.** The SDK's bindings put the world's type into each
  module, in its two `component-type` sections, so `wit-component` wraps any theme into a component
  of `caprice:theme@1.0.0` from the module alone, as `wasm-tools component new` does, with the same
  bytes each time: what a host on Wasmtime, or a browser that runs components itself, would take.
  The gallery signs the module, and a theme is never built again, so a change to the SDK's bindings
  or to the flags (`themes/build.sh`, `toolchain/pack.mjs`) must keep those sections: `-C
  strip=debuginfo` does, stripping every custom section would not. Test:
  `the_sdk_s_themes_wrap_into_components_of_the_world_as_they_are_built` (every theme in `themes/`).
- **Caprice writes and reads the canonical ABI itself; in the tests no module is run.** It writes
  the frame's input (`input_bytes`, 144 bytes) and the events (`event_bytes`, 12 bytes each) in
  `eggs/program.rs`; in the browser `web/theme-worker.js` reads the program's `frame-output` (40
  bytes: the desk's, the window's and the carrier's shapes, 28 bytes each; `next`, the title's
  offset, the ground) out of its memory into flat words (`HEAD`, 8 of them, then the shapes), read
  into egui's shapes by `eggs::program::read`. Under `cargo test` nothing runs WebAssembly (wasmi
  ran the modules until 2026-11-15: thirty times slower than the theme natively, it made the
  Vineyard's comparison take minutes and tested a host that never ships): each fixture module
  stands for its theme's source built for the machine (`themes/played`, a crate optimized in the
  test profile; `theme!` gives each a `played()` there, and the SDK's `host` answers from
  `theme_sdk::native::answering`), and the tests' player (`platform::program::played`) reads the
  input and the events out of Caprice's bytes as the world lays them out and makes the theme's
  output into the same words; a module a test writes by hand stands for what the test says
  (`stand_in`: words, a frame that fails, one that never comes). So the browser's host, the player
  and the reader must agree on every layout. Tests:
  `a_theme_is_told_every_field_of_its_input_and_each_event_as_written` (through `themes/echo`,
  which draws back what it is told), `the_browser_s_host_is_given_the_same_words_to_make_as_the_tests_host`
  (it keeps `tests/fixtures/echo.words.json`) and `web/theme-worker.test.mjs`: the real `echo.wasm`,
  run by the browser's host on the same bytes, must make the same words, so Caprice's bytes mean to
  the module what the player reads in them, and its output comes back as the player makes it.
  The words wasmi had made were kept unchanged when the player took its place. What the player
  cannot see is the module itself (its maths library, how it was compiled): that is `smoke.mjs`'s,
  the node test's and the fixtures' check in CI (below).
- **It is checked every time it is loaded, never trusted** (`eggs::program::check`, wasmparser):
  at most 1 MB (`MOST_BYTES`); it imports only `caprice:theme/host@1.0.0`'s four functions with
  their signatures, exports exactly the world's (`events`, `frame`, `cabi_post_frame`,
  `cabi_realloc`, its `memory`) and nothing else; one memory of its own, not shared, declaring at
  most 32 MB (`MOST_PAGES`: the browser's engine holds it to what it declares); and it uses no
  WebAssembly beyond a fixed set, WebAssembly 2.0 without SIMD (`features`), which every engine
  runs, so a theme built today runs everywhere for good. A later toolchain that turns on more is a
  new version of the world. Refusals say why in words. Tests:
  `a_module_that_asks_for_anything_but_caprice_s_host_is_refused_saying_why`,
  `a_module_using_webassembly_beyond_the_fixed_set_is_refused`,
  `a_module_that_may_take_too_much_memory_is_refused`,
  `the_sdk_s_themes_pass_the_check_as_they_are_built`.
- **It runs apart from the app, and Caprice never waits for it** (`platform::Program`, R31). In the
  browser, a worker of its own (`web/theme-worker.js`, from 'self', nothing evaluated) runs it in
  the browser's own engine; the frame posts the input and draws the newest words that have come
  back (transferred, never JavaScript objects: those cost more to receive than the whole frame), so
  **a program's shapes are a frame behind its input**: what must stay in step travels in one output
  (the shapes and the carried title's offset), and the zoom is Caprice's, so zooming is never late.
  A worker that does not load in `LOAD_SECONDS` or answer a frame in `ANSWER_SECONDS` is ended
  (`terminate`), as is one whose program traps. **Only time Caprice was there to hear counts**
  (`platform::program::Waiting`): of a gap between two looks (a frame of Caprice's own that took
  long, the page hidden) at most `BUSY` (a quarter second) is the program's. Counting all of it, a
  frame that held the page for over a second (the Vineyard's 156 000 vertices in headless
  Chromium's software drawing) left a worker's answer, made in 12 ms, unread, and the theme was
  stopped for not answering; a worker that truly never answers is still stopped, on the slowest
  device too (`a_program_is_given_its_time_only_while_caprice_is_there_to_hear_it`,
  `a_program_that_never_answers_is_stopped_on_the_slowest_device_too`); on the desktop it is never run, and the usual desk
  shows with a word why; in the tests a stand-in that fails or never answers is stopped the same
  way (`Stand::Fails`, `Stand::Silent`, after `ANSWER_SECONDS`). Leaving the theme ends its worker
  (`program::Shown::forget`). Tests: `a_frame_that_never_comes_or_fails_stops_the_theme_saying_why`,
  `a_program_theme_that_fails_or_draws_more_than_it_may_is_stopped_with_a_word_why`; in the browser
  `smoke.mjs`, "a program theme plays in a worker of its own and draws the desk" and "a program
  theme that never answers is ended, and the usual desk shows".
- **A frame may draw at most 450 000 points** (`MOST_POINTS`: a polygon's or a path's points and a
  mesh's vertices, a circle or a line one): a little more than the most a built-in theme draws, so
  a program may cost Caprice's frame what its own themes cost (the owner's call, 2026-11-15). The
  Magic Vineyard draws 63 000 on a phone's desk, 136 000 to 168 000 on a computer's, 309 000 zoomed
  out in a large window and about 420 000 in an ultrawide one, nearly all of it leaves as mesh
  vertices; read and tessellated at a quarter of this machine's speed, 5, 11 to 14 and 26 ms (the
  built-in pays the same tessellating, and its own drawing besides). It was 40 000, twice the
  Snow's (7 ms): the Vineyard's port was over it everywhere. Not a limit by the window's area: a
  theme's points grow with the land it shows (the width over the size of things and the zoom), so
  a small window zoomed out costs as much as a large one. The limit stops a theme that runs away;
  keeping a theme light is for where it is made (the plan's step 5: its points on a phone's desk
  and a computer's, told to Claude). Over it the frame is not drawn (the last one
  stays); `OVER_FRAMES` running stop the theme. What cannot be drawn (a number that is not finite,
  a mesh's index beyond it, a polygon of two points, a path of one) is left out. egui tessellates the shapes as
  they come: a mesh of Caprice's own per layer was measured slower. Tests:
  `a_frame_over_the_points_a_theme_may_draw_is_not_read`,
  `what_cannot_be_drawn_is_left_out_and_words_cut_short_are_refused`.
- **What it is told comes from what Caprice already keeps** (`Shown::tell`): `fresh` when it is
  picked, the story is another or opened again, or the garden is counted afresh (`Eggs::regrown`,
  R30); `begun` for each new chapter of `Doc::garden` and `grown` for each chapter with more pages
  than before, so it grows only as the magic themes do and never shrinks; `clicked` for a click on
  the desk below the toolbar, beside the page and the bar, with no window over it. `host` answers
  from a copy sent with each frame: the story's seed (`story_seed`, an FNV-1a hash of `story_id`,
  never the id, the same in every browser), the garden, the page count. The SDK's `Seeds` and
  `Tended` are Caprice's own (`eggs::Seeds`, `eggs::Tended`), and **the built-in magic themes are
  seeded from that seed too** (`eggs::Seeds::of`: `story_seed`, then splitmix64), so a program and a
  built-in theme draw from the same numbers. They were seeded from `story_id` by the standard
  library's hasher, which differs between 32- and 64-bit and may change with any Rust (the browser
  build's nightly among them); changed on 2026-11-15, which moved every story's layout once. The
  numbers are fixed for good: changing them moves every writer's garden, houses and igloos. Tests:
  `a_story_s_magic_themes_are_laid_out_the_same_everywhere_for_good` (fixed numbers),
  `a_program_theme_hears_each_chapter_grow_and_begin_and_a_click_on_the_desk`,
  `a_program_theme_picked_shows_the_story_as_it_stands_at_once`,
  `a_program_theme_draws_the_desk_and_lights_a_light_in_the_chapter_s_jar_for_each_page`,
  `the_sdk_seeds_and_tends_as_caprice_s_own_themes_do`.
- **It lays out as at the fitting zoom; Caprice zooms it about the ground the theme names** (as
  `draw_snow` and `draw_vineyard` zoom about theirs): `frame-output.ground` says how many units above
  the foot of the desk (the page bar's top when it lies along it) its ground is, and the point under
  the page's middle at that height (`desk.ground`) is what Caprice zooms about, so what stands on the
  ground stays where it is on the screen and everything else comes nearer. **The desk
  (`desk.area`) and the bar are given as they lie at the fitting zoom**, under the page's middle
  wherever the page has moved (`view_area`'s middle is where a fitted page's is), never what shows
  at the zoom: the page, the carried title's place and a click are given unzoomed about that point,
  and all it draws on the desk and over the page is zoomed back by `Shown::zoom`, the title's
  offset too. What shows (`Input::shown`, for fills that reach the window's edges) the SDK works
  out from `desk.window` zoomed back about the ground, so the world's interface did not change for
  it. Until 2026-11-19 `desk.area` was what shows, as the world's comment said, while the SDK
  and the guide said "as at the fitting zoom": the gallery's Magic Castle, placed a little in from
  the desk's right, stayed by the window's edge as it grew, and slid about the mountains behind it
  as the page moved with the pointer (the built-in themes' ports placed nothing from it and never
  showed it). Its meaning was changed within `caprice:theme@1.0.0`, its comment with it, as the
  only themes made were the owner's: a deliberate exception to the rule below, not to repeat once
  others make themes. The off-screen
  preview tells a theme the same desk (`preview::desk_of`). Test:
  `a_program_theme_zoomed_about_the_pointer_beside_the_page_keeps_its_world_in_proportion` (a
  stand-in placing a castle from `area` and a hill on its ground).
  The ground of the newest output is used for the next input and for drawing it, so a theme's very
  first frame is zoomed about the foot. What it puts in the window (`frame-output.window`: snow
  falling, not the hillside) is laid out on `desk.window`, the desk as it lies on the screen, and
  never zoomed; it is drawn after the desk's shapes, behind the pages. Zoomed about the foot
  instead, the Snow's port stood 4.6 points off the built-in at the first zoom tried: the
  comparison below fails so. **Zoomed out, a frame is drawn at the zoom it was asked at** until
  the next comes (`Shown::made_at`): it covers only the desk as it was then, so drawn at the zoom
  of the moment its sky stopped short of the window's edges while the writer zoomed out quickly
  (seen live, 2026-11-16, and never in the built-in themes, which draw in Caprice's own frame).
  Zoomed in, it covers more than the window, and follows the view at once, **zoomed about the
  ground it was asked with, put where the ground is now** (`made_at` keeps that point too): zooming
  about the pointer moves the page's middle, and the ground under it, a long way each frame, and
  drawn about the ground of the moment the world was that far off times the zoom (870 points at four
  times), and shook as frames came (seen live with the gallery's Magic Castle, 2026-11-20). Zoomed
  out, the held frame is still drawn about its own ground, so it covers the window. Tests:
  `a_program_theme_zoomed_out_quickly_never_leaves_the_edges_of_the_window_bare`,
  `a_program_theme_zoomed_in_about_the_pointer_stays_under_the_page_between_its_frames`.
- **The Magic Snow and the Magic Vineyard draw the same as their ports, frame for frame**
  (`themes/snow/theme.rs` and `themes/vineyard/theme.rs`, built into `tests/fixtures/`): two
  `Harness`es given the same story, window and keys (`Twins`), one in the built-in theme, one in its
  port; each frame of the built-in is compared with the port's next (a frame behind), shape for
  shape (kind, points, triangles, colours, widths; a rectangle as the polygon of its corners, a
  mesh with no triangles as nothing), zoomed in and out, in three windows and with the bar at the
  foot and at the sides; for the Snow while blocks are cut and float up and an igloo is finished
  and lit (at most 0.0005 points apart, the colours the same), for the Vineyard while bunches
  ripen a page at a time, the stretch before turns and drops its leaves, and the starlings fly (the
  same to the bit). Both are seeded alike (`story_seed`, above), and `Eggs::painted` keeps what
  `draw_snow`, `draw_vineyard` or `draw_program` painted. The ports are played natively (above), so
  this compares the sources; the module's own maths is the browser's to show. So
  **`eggs/snow.rs` and `themes/snow` change together, and `eggs/vineyard.rs` and `eggs/country.rs`
  with `themes/vineyard`** (the port carries the country, and `eggs.rs`'s `flame` and `ribbon`,
  as its own), as do the SDK's helpers and what they mirror (`Layer::solid`, `oval`, `radial`,
  `Color::see_through` and `lerp`, `Point::rot90` are `eggs.rs`'s `solid`, `oval`, `radial`, `glow`
  and egui's `lerp_to_gamma` and `rot90`, to the rounding): change one, change the other, run
  `themes/build.sh`. While the size of things follows a window resized, the port's shapes are
  zoomed about where the ground is a frame later than they were laid out for (a fraction of a point):
  not compared. A port that is wrong is caught: the Vineyard's ground a unit off is a point apart,
  a leaf's green two shades off, the starlings, the turning or the ripening a little early, each
  fails. Tests: `the_magic_snow_as_a_program_draws_what_the_built_in_one_draws_frame_by_frame`,
  `the_magic_vineyard_as_a_program_grows_and_turns_as_the_built_in_one_frame_by_frame`,
  `the_magic_vineyard_as_a_program_draws_what_the_built_in_one_draws_zoomed_and_in_other_windows`
  (two, run side by side: a Vineyard frame is ten times a Snow frame to draw and compare);
  in the browser `smoke.mjs`, "the Magic Snow as a program draws its hillside", "and its snow
  falls" and "the Magic Vineyard as a program draws its own sky and its slope" (the only places
  their real modules draw on Caprice's desk).
- **Its carrier is flown by Caprice's course, drawn and offset by the program** (`Carrier::Program`,
  `pose_of` as the balloon's): a program that calls `ask-lift` brings new chapter titles, under the
  same rules as every carrier (`maybe_lift_title`, `update_lift`, `Eggs::odds`); while one is
  carried it is told the pose, the side and the title's place, draws the carrier over the page
  (`draw_carrier`) and says where the title hangs (`title-offset`, through `App::title_offset`, which
  `editor.rs` moves the title by). Test: `a_program_theme_brings_a_new_chapter_title_with_its_carrier`.
- **None of it is printed or saved with a story** (R20, R30): it is drawn in none of `underlay`,
  `ink` or `furniture`, and moves no text. Its colours are its own (R29), its palette the entry's.
  Test: `print_shows_nothing_of_a_program_theme`.
- **A program travels as a theme file, but never as a file a writer picks or saves** (`bundle.rs`,
  the plan's step 3; the owner's decision of 2026-11-16, §7): a `.caprice-theme`,
  a zip of `manifest.json` (`bundle::Manifest`: the world it is built for, which must be
  `caprice:theme@1.0.0`; its name, a few words, its author; its palette by `own::NAMES`; the
  toolchain), `theme.wasm`, and if they come with it `theme.rs` (kept beside the module as
  `<id>.rs`) and `preview.png` (the gallery's picture, made 320 wide). Programs are made with
  Claude (`making.rs`, below) or taken from the gallery (step 6, whose "Use" hands the shelf a
  theme file: `App::take_theme_file`). "Import a theme…" in Yours offers drawings only
  (`own::import_dialog`); a theme file (told by its first bytes, `bundle::is_bundle`) or a `.rs`
  picked anyway is refused, saying where programs come from (`own::NO_PROGRAM_FILES`), and a
  program on the shelf has no Export: what is shared goes through the gallery, which rebuilds it
  from its source and checks it first. A theme file taken is read off the frame like a drawing
  (`own::Prepared`) and reported in the same window: refused with reasons in
  words when it is not a zip, has no manifest, neither a module nor a source, names another world,
  or its module fails `eggs::program::check`; a file with a source and no module is built first
  (below); and what it lacks or holds besides (no palette, colours not read, no
  picture, other files) is worth knowing, never a refusal. Every part is read to at most its size,
  whatever the zip says (`MOST_FILE`, the module's `MOST_BYTES`). A file never carries the
  gallery's signature.
  **Its palette is the theme's**: the toolbar, the buttons, the page bar and the desk under the
  program's shapes (shown before its first frame and when it is stopped) take its colours, so a
  program that draws its own sky should carry a palette that agrees with it. A program on the shelf
  is the file it came as again (`Shelf::bundle`, `bundle::write`: no time in the zip, so the
  same parts give the same bytes), as it will go to the gallery. `tests/fixtures/snow.caprice-theme` and `vineyard.caprice-theme`
  are the ports, with their sources and the built-in themes' palettes (`themes/<theme>/manifest.json`),
  to try by hand; a change to a port, its manifest or its module means making them again
  (`CAPRICE_FIXTURES=1 cargo test`). Tests:
  `the_snow_s_and_the_vineyard_s_theme_files_are_their_manifests_modules_and_sources`,
  `a_file_that_is_not_a_theme_or_whose_program_cannot_be_played_is_refused_saying_why`,
  `what_a_theme_file_lacks_or_holds_besides_is_worth_knowing_not_a_refusal`,
  `a_theme_file_taken_is_chosen_and_played_in_its_own_colours_and_kept_as_it_came`,
  `a_theme_file_whose_program_cannot_be_played_is_not_added_and_a_window_says_why`,
  `a_program_is_never_imported_from_a_file_nor_exported_to_one`,
  `a_writer_is_offered_only_drawings_to_import_never_a_program`; in the browser `smoke.mjs`, "the
  gallery's import offers drawings only, and a theme file given it anyway is not added".
- **A gallery theme is the gallery's only while its signature holds** (`own::Signed`): the gallery
  signs (ed25519) its module's SHA-256, its source's, the toolchain and the name it is published
  under (`Signed::message`); the shelf checks it against `GALLERY_KEY` every time it is read or a
  theme added (`Shelf::check_signatures`), never trusting an earlier check, and only then shows the
  gallery's name (`Entry::shown_name`). Changed by a byte, or signed by another key, it is the
  writer's own under its own name. Tests set the stand-in server's key (`stand_in::gallery_key`),
  and none holds the real one: so where the owner keeps it, a test checks that `GALLERY_KEY` is
  the public half of the Worker's secret (a mismatch would refuse every theme taken, and nothing
  else would say so); elsewhere it says it did not check. A test of "Use" in a real tab with a
  theme the real key signed is left open (the plan's step 6): it would mean a signature made with
  the real key in this public repo, made again for each toolchain. Tests:
  `a_gallery_theme_is_shown_under_the_gallery_s_name_until_a_byte_of_it_changes`,
  `the_gallerys_key_is_the_public_half_of_the_key_the_worker_signs_with`.
- **The gallery publishes only what the owner built again, the same, and saw play** (`gallery.rs`):
  a writer sends a theme made with Claude from its "⋯" menu (once: `Entry::sent`), under the name,
  words and author they give. The owner looks at it in Caprice: the module fetched must be the one
  the Worker took in; its source is built in their tab by the toolchain it names (another toolchain
  cannot be built the same, so it is not published), and the module built must be the module sent,
  byte for byte; then it is played through the preview's story (`eggs::preview`) and its pictures
  and points shown against `gallery::BUDGET`. "Publish it" is offered only then, and sends the hash
  of what was built. "Use" fetches the file, checks the gallery's signature over its module before
  anything is kept, and adds it with that signature; one already on the shelf is chosen instead.
  The owner sees "Take down…" under each theme in the gallery, and once more asked ("Out of the
  gallery for everyone?") it is taken out (above, R33) and goes from the window.
  A theme is on the shelf once: if the writer's own program is the module taken, byte for byte (the
  one they made and sent), it is given the signature and becomes the gallery's, source and `sent`
  kept (`Shelf::sign`); a shelf read that holds both, from before, keeps the gallery's
  (`Shelf::join_doubles`). Yours shows the themes from the gallery, those made with Claude and the
  drawings apart, in that order; the themes' menu is two tiles wide whichever it shows
  (`THEMES_WIDE`: a menu never narrows again, so nothing in it may be wider). The Gallery window
  and the one that sends are no wider than the screen (`ui::window_width`, its themes one to three
  to a row), and close with the story (`Gallery::close` in `leave`): they are not drawn over the
  welcome page, and would wait there for the next story. Tests: `a_theme_made_with_claude_is_sent_built_again_by_the_owner_published_and_used_from_the_gallery`,
  `the_owner_refuses_a_theme_that_is_not_built_again_the_same_and_says_why`,
  `the_owner_takes_a_theme_down_from_the_gallery_once_asked_and_its_writer_is_told`,
  `only_the_owner_is_shown_the_themes_waiting_and_the_gallery_is_for_writers_signed_in`,
  `yours_shows_the_themes_from_the_gallery_those_made_with_claude_and_the_drawings_apart_and_sends_each_once`,
  `a_shelf_holding_a_theme_as_made_and_as_taken_from_the_gallery_keeps_the_gallery_s_alone`,
  `the_themes_menu_is_as_wide_as_its_two_tiles_whichever_themes_it_shows`,
  `the_gallery_window_fits_a_phone_s_screen_and_goes_when_the_story_is_closed`; in the browser
  `smoke.mjs` sends the theme it made, checks the file the tab wrote with the Worker's own check
  (`web/api/theme-file.js`), and as the owner builds it again in the tab and publishes it: "a theme
  sent to the gallery passes the Worker's check, its module the one built", "the owner's tab builds
  it again from its source, the same byte for byte, and publishes it", "a theme the gallery did not
  sign is fetched with Use and not taken onto the shelf".
- **A theme is built from its source in the writer's tab, by one fixed toolchain** (`web/compiler/`,
  `platform::compiler`, the plan's step 4): rustc and wasm-ld built as WebAssembly (the
  investigation's release, `1.99.1-6c798e2613f0`), rustup's std for `wasm32-unknown-unknown`
  without `test`, `getopts` and `proc_macro`, the SDK as one rlib and the flags, packed by
  `toolchain/pack.mjs` into `web/dist/toolchain/<id>/` (brotli pieces under Workers' 25 MiB a file,
  named by their hash, kept for good in `_headers`) and named in `web/dist/compiler/toolchain.json`
  (asked about each time). **The id names everything a build depends on** (the compiler's
  `RUSTC_ID`, then a hash of the std, the SDK, the flags, the decoder and the VFS): change any of
  them and it is a new toolchain, new files, the old ones untouched. **It is the same on every
  machine**: where the checkout and the crates' sources lie would be in what rustc makes (the
  working directory in the SDK rlib's metadata, a crate's path in the decoder's panic messages), so
  both are remapped (`/caprice`, `/crates`); before that, CI and this machine packed the same
  compiler under two ids. A source Claude wrote is
  built by the window that asked for it (`making.rs`, below); a theme file taken with a source and
  no module, or a source alone (`App::build_theme_source`, `bundle::from_source`), by
  `Workshop::build_theme`, which starts a compiler (52 MiB fetched the first time, then from Cache
  Storage, its pieces' hashes checked once, when fetched), a window says how far it has loaded and
  can stop it, and the module it builds is checked like any other and kept with its source and the
  toolchain's id (`Manifest::toolchain`). **The compiler is ended as soon as it has answered**
  (dropped with `Building`): it holds about a gigabyte while it lives (step 0d). Mistakes come back
  as the compiler wrote them (`Report::details`); a build over `BUILD_SECONDS` (counted only while
  Caprice hears, `Waiting`) ends it. It runs in workers of its own from 'self', so the CSP needs
  nothing new; it needs threads and shared memory, and is built for Chromium on a computer: elsewhere
  the worker says so before fetching anything (`unsupported`), and a build with no toolchain says
  that. On the desktop the compiler says themes are built in the browser for now; under `cargo test`
  a stand-in builds the sources of `themes/` to their fixtures and anything else to what the test
  says (`compiler::stand_in`). Tests:
  `a_theme_s_source_is_built_kept_with_its_source_and_toolchain_and_played`,
  `a_theme_s_source_with_mistakes_is_not_added_and_the_window_shows_what_the_compiler_said`,
  `a_theme_being_built_says_how_far_the_compiler_has_loaded_and_can_be_stopped`,
  `a_theme_file_or_a_source_with_no_module_is_taken_to_be_built_from_its_source`; in the browser
  `smoke.mjs`, "its source is built in the tab, and Claude is shown its pictures once", "and the
  compiler is ended once there is nothing more to build" and "kept, it is the very module the
  toolchain builds anywhere, with its source and a picture, and it plays" (the Snow's `theme.rs`,
  as a stand-in Claude answers, its module compared with the fixture byte for byte). **The release is pinned**: `toolchain/release.json` names
  the investigation's GitHub release (`1.99.1-6c798e2613f0`, with its licences) and each file's
  sha256; `toolchain/fetch.mjs` fetches it (CI does, before `web/build.sh`, keeping what brotli made
  in a cache), and `pack.mjs` checks every file against its pin wherever it found it, so CI, a
  deploy and this machine pack the same toolchain, the same id. `web/build.sh` fails if it cannot
  pack it (`CAPRICE_TOOLCHAIN=0` leaves it out; `smoke.mjs` then skips the build, saying so).
- **A theme made with Claude reaches the writer only once it works** (`making.rs`, the plan's step
  5; the server's side is R33's `/api/theme-program`). The window ("Have one made…" in
  Yours, for a writer signed in, saying the words go to an AI model by Anthropic) starts the compiler first, so a
  browser that cannot build says so before Claude is asked, and the toolchain loads while Claude
  writes. Each answer (only a source and a palette) is built, checked (`eggs::program::check`) and
  played off screen (`eggs::preview`). **A version is done only when it passed everything a writer
  would meet** (the owner's decisions for step 5): what goes wrong goes back to Claude, the
  compiler's words as an `errors` round, a program Caprice will not play, one stopped while playing
  the preview's story, one that draws more than `gallery::BUDGET`, and one whose own sky and ground
  reach the window's edges at the fitting zoom but stop short of them zoomed out
  (`Previewed::bare`: stretched over `Input::area` instead of `Input::shown`, as the gallery's
  Magic Castle and Lighthouse were, 2026-11-20) as a `checks` round in Caprice's words, at most `FIXES` of either (the server's `THEME_ERRORS_ROUNDS`), and **the writer
  never sees the compiler's words nor the checks**. The budget is the one the gallery's review
  shows and the guide names (`GUIDE.md`, "Points cost"): change them together. A version over it
  that plays is a near miss: kept to fall back on while there is no version within it, offered to
  keep when the fixes run out, never put in place of a lighter one. One that stops short of the
  edges is kept as any other: when the fixes run out, Claude and the writer see it zoomed out. The
  first version that plays is shown to Claude as its pictures and points (`frames`, `LOOKS` times:
  each costs about as much as writing it); then the writer sees its pictures, keeps it, starts
  again, or asks for a change (`change`). **A version that worked is never lost to one that did
  not**: a fix that never builds, or a look Claude cannot give, leaves the last good version shown,
  with a word why; with none, the window says nothing came of it, and nothing is kept. Nothing goes
  on the shelf until "Keep it" (then as a theme file would: its module, source, palette, toolchain
  and its preview's picture, `App::keep_theme`), and the server is told it was kept (R33's
  `themeKept`; nothing waits for the answer). The window says it is free for now (`FREE`), where the
  writer asks and beside "Keep it": a theme kept meanwhile stays theirs whatever comes later (§7).
  The compiler is ended once there is nothing more to build. Closing the window only hides it. Tests:
  `a_theme_made_with_claude_is_built_shown_to_claude_once_changed_and_kept_when_the_writer_says`,
  `mistakes_go_back_to_claude_unseen_and_a_theme_that_never_works_says_so_keeping_nothing`,
  `what_caprice_finds_of_a_program_that_built_goes_back_to_claude_as_checks_unseen_by_the_writer`,
  `a_theme_whose_sky_stops_short_of_the_window_zoomed_out_goes_back_to_claude_unseen_by_the_writer`,
  `a_theme_too_heavy_however_often_it_is_sent_back_is_still_offered_to_keep`,
  `a_lighter_version_is_not_lost_to_a_heavier_one_that_cannot_be_made_lighter`,
  `a_theme_is_made_only_for_a_writer_signed_in_and_what_the_server_says_is_shown` (the stand-in
  server answers the rounds, `Online::theme_answers`); in the browser `smoke.mjs` makes one with a
  stand-in Claude, built in the tab.
- **The writer is told how far a theme is, and never of Claude** (`making.rs`, the owner's call,
  2026-11-17). Rounds take minutes, so the window says how long a theme takes (`UP_TO_MINUTES`),
  counts its steps (`Making::step`: getting ready, writing, building, trying, and for each look at
  the pictures writing, building and trying again: 7 for a theme begun, 4 for a change), never
  going back (a mistake put right stays at the step it was found in), and shows the time so far.
  The bar moves within a round of writing by `TYPICAL_ROUND`, never past most of its step. A
  round is sent with `Server::send_long`, which hears the server's spaces (R33's `keptAlive`; in
  the browser `api_send` reads the answer as it comes): half a minute without them is said, a
  minute is a connection lost. **What went wrong is said plainly**: the network lost
  (`Server::online`, the browser's `navigator.onLine`: `LOST_NETWORK`), the server out of reach
  (`LOST_SERVER`), or the server's own words, also when it had begun answering (its status in the
  body). **Claude is not named in what a writer reads about themes** (the window, Yours' "Made
  for you", the gallery, the refusal of program files); `GOES_OUT` names Anthropic, where the
  words go. Claude's own features keep its name (the Claude menu). Tests:
  `a_theme_being_made_shows_its_steps_and_its_time_and_never_names_claude`,
  `a_theme_being_made_says_plainly_when_the_network_or_the_server_is_lost` (the stand-in holds
  rounds with `Online::writing`, `beat` and `written`).
- **A theme is pictured by playing it off screen** (`eggs/preview.rs`): through the same
  `platform::Program` and `program::read` as on the desk, told a short story at ten frames a second
  (begun; three chapters written, shown afresh; the fourth begun, the third finishing; a title
  carried, only if it asked to carry titles; three chapters written, zoomed out to `view::LEAST`,
  the page zoomed with the desk about the theme's ground), each frame's desk laid out as Caprice lays it out at
  the fitting zoom (`preview::layout`, from `view::fit` and the docks' sizes: change how the desk is
  laid out and check it still agrees) and unzoomed about its ground as `draw_program` does. A
  moment's last frame is drawn as the screen draws it: the desk in the palette's colours, the
  program's desk and window shapes, the page with lines of text, what it draws over the page and
  the carried title, the docks; tessellated by egui's own tessellator and filled triangle by
  triangle (`rasterize`, premultiplied, each pixel on a shared edge in one triangle only). The
  same story counts its points on a phone's desk (412×915) and a computer's (1920×1080, and zoomed
  out to `view::LEAST`): the Vineyard 63 000 and 309 000, as measured on the screen. At three
  chapters written and zoomed out it measures how much of the window's edges the program's own
  shapes cover (`edges_covered`: 160 points along the four sides, each covered when what is drawn
  over it is at least 90% opaque), for `Previewed::bare`, which counts a theme as reaching them at
  95%: a theme that draws no sky of its own leaves the desk's colours at the fitting zoom too, and
  is never told. **What Claude is asked for, how finished a theme should look, is in the guide**
  (`GUIDE.md`, "How finished it should look": depth, small parts on what grows, a little life,
  detail where it shows, to the window's edges), and the look at the pictures asks for it again
  (`theme.js`, `asked.frames`): a theme that needed a second session to be finished (the Castle's
  texture) should not, and a change to what the guide asks goes with the check that holds it, where
  there is one. A program theme
  kept without a picture (a theme file without `preview.png`, a source just built) is given one,
  three chapters written, 320 wide (`Workshop::picture_theme`, `Shelf::set_picture`); one that
  cannot be played keeps its colours. `tests/fixtures/snow.caprice-theme` and
  `vineyard.caprice-theme` carry the pictures it makes. Tests:
  `a_theme_is_shown_its_story_and_pictured_as_the_desk_looks_with_the_page_on_it`
  (`CAPRICE_PREVIEW_OUT=dir` keeps the pictures to look at),
  `a_theme_whose_sky_stops_short_of_the_window_zoomed_out_is_told_so`,
  `a_program_theme_kept_with_no_picture_is_given_one_made_from_its_program`; in the browser
  `smoke.mjs`, "kept, it is the very module the toolchain builds anywhere, with its source and a
  picture, and it plays".
- **The themes in `themes/` are built as the writer's tab builds them** (`themes/build.sh`):
  rustc 1.99.1, the SDK as one rlib, one file, the flags Caprice fixes (`--max-memory` among them),
  the output always named `theme.wasm`, and **every path as the tab has it**: the SDK's files as
  `/sdk/…` and the theme as `/theme/theme.rs`, wherever it was written (absolute paths, remapped: a
  path is in the module, where a panic would say it happened; until 2026-11-16 the paths were
  relative and the remapping never applied, so the fixtures carried `themes/vineyard//theme.rs`).
  The same rustc gives the same bytes, natively or in the tab: `toolchain/pack.mjs` builds every
  theme with exactly what it packs (the pruned std, the SDK's rlib, the manifest's flags) and packs
  nothing unless each is its fixture byte for byte, and `smoke.mjs` holds the tab's build of the
  Snow to its fixture. So a change to the flags is made in `themes/build.sh` and `pack.mjs`
  together. Their modules are the
  tests' fixtures (`tests/fixtures/*.wasm`, checked in), and stand in the tests for the same files
  built for the machine (`themes/played`, which `build.sh` passes over: it has no `theme.rs`). So
  nothing in `cargo test` sees a module that is not its source built: change a theme or the SDK,
  then run the script; CI builds them again and fails if a fixture differs. `tests/fixtures/spin.wasm`
  is `spin.wat` (test `the_theme_that_never_answers_is_the_module_its_text_says`), the module
  `smoke.mjs` ends for not answering. `themes/echo` draws back what it is told,
  `themes/fireflies` is small enough to read at once, and `themes/snow` and `themes/vineyard` are
  the Magic Snow and the Magic Vineyard (the examples a theme Claude writes learns from, plan,
  step 5: the Snow drawing on the desk's own colour and snowing in the window, the Vineyard a
  layout of its own and its own sky, all of it zoomed).

## 5. Checklists for common changes

**Something new in the background** (a rendering, a request): `platform::spawn` with owned data in,
a channel out, a `platform::Waker` to ask for a frame; nothing behind a lock shared with the frame
(R31). Then check it in the browser: `web/build.sh`, `web/smoke.mjs`.

**A new file dialog**: a `platform::Dialog` to `ask_path` with a new `DialogFor`, acted on in
`poll_dialog`; it reads or writes through `platform::fs`. Decide what it is in the browser: a pick
(from the page's picker, in `/uploads`) or a file given (`/downloads`), and add it to `smoke.mjs`.

**A new way for a scene's parts to move**: its line in `motion::PROMPT`, its id in `parse_id`
(and the check of its point, if it has one, in `layered`), a `Motion`, its movement in `pose`
(or drawn by Caprice, like `smoke` and the `particles`), and a part of it in `NIGHT` for the tests
(R14). If themes of one's own may use it: its line in `own::MOTIONS` and in the template's comment
(R35).

**A new setting for themes of one's own**: a field in `own::Meta` (or `Growth`, `CarrierLook`) with
`#[serde(default)]`, read where the theme is drawn (`eggs/own.rs`), its line in `own::guide` and in
the comment of `assets/theme-template.svg` (R35).

**A change to what theme programs are told, may ask or draw** (R36): never an edit of
`caprice:theme@1.0.0` once published: a new version of the world in `theme-sdk/wit/theme.wit`, its
bindings made again (`theme-sdk/tests/bindings.rs` says how), the browser's host
(`web/theme-worker.js`), the tests' player (`platform::program::played`, and the SDK's `native`) and
`eggs::program` (`input_bytes`, `event_bytes`, `read`, `check`) taught it, `themes/echo` echoing it,
`themes/build.sh` run, and `CAPRICE_FIXTURES=1 cargo test` to keep the words for
`web/theme-worker.test.mjs` and the theme files. A new part of a theme file goes into
`bundle::Manifest` with `#[serde(default)]`, read in `bundle::open` and written in `bundle::write`. A change to the Magic Snow (`eggs/snow.rs`) or the Magic Vineyard
(`eggs/vineyard.rs`, `eggs/country.rs`) is made in its port (`themes/snow`, `themes/vineyard`) too,
and the fixtures built again.

**A new toolchain for theme programs** (a new Rust, the SDK or the flags changed, R36): the SDK
and the flags change in `themes/build.sh` and `toolchain/pack.mjs` together; `themes/build.sh`,
then `web/build.sh` (which packs it, and fails to pack unless every fixture is its source built with
what is packed); `smoke.mjs` builds the Snow in the tab. A new Rust is a new release of the
investigation's toolchain, published as a GitHub release of its repository with the licence texts
and pinned in `toolchain/release.json` (tag, URL, each file's sha256), and a new `rustc +<version>`
in both scripts and in CI. Old themes keep their modules; old releases stay published, for
rebuilding them.

**A new paragraph attribute** (like line spacing or chapter title): `ParaAttrs` → `run_format` /
`paragraph_job` → the toolbar (`ui.rs`) → file format default (R9) → Word mapping (R10) → whether
it changes drop caps (`has_drop_cap`) or the contents (`chapters`). The brush copies it with the
rest of `ParaAttrs` (R25).

**A new kind of character formatting** (like colour): a `Style` field (in `same_char`) →
`run_format` → the toolbar and `format_box` → Word's `run_props` (R10). It prints (R20) and the
brush copies it (R25) without more work.

**A new page setting:** `PageSetup` field + default → Page menu (`ui.rs`, applied through
`set_setup`) → pagination/layout → Word section or settings (R10) → round-trip test.

**A new look for headers, footers or text boxes:** a `BandLook`/`BoxLook` variant (in `ALL`, so
the gallery shows it) → its `Ink` in `band_ink`/`box_ink`, convex fills only → its Word mapping in
`band_xml`/`text_box_xml` (R10) → check with `sample_docx` and `sample_pdf`.

**Something new drawn on a page:** size at page size (R4) → pagination (`next_span`) and layout
(`layout_page`) agree → incremental equals full (R3) → both looks (`render.rs` and `book.rs`) →
hit-testing and caret (`PageLayout`) → into `PageLayout::ink`, so it prints (R20). Lying on the
page rather than in the text: into `App::underlay` or `App::furniture`, which layer decided (R24).

**Page navigation or a new way to move between pages:** use `turn_to` / `go_to_page` (R5); test it
on the contents pages with `Harness::long_contents`.

**The file format:** `serde(default)`, a migration in `into_doc` that moves notes and scenes, an old-file test.

**A new tool in the toolbar:** into a group in `tool_group` (`ui.rs`), what it does recorded in
`Tools` and done in `apply_tools` after all the tools are drawn, since a group can be drawn in
either row. The order of `GROUPS` is the order they go into "⋯", the last first.

**A new tool in a dock:** in the toolbar, inside a group in `tool_group`, nothing in it depending on
where the group is (R: the toolbar's groups); in the page bar, room in the sums in `scrollbar`,
`standing_scrollbar` and `phone_scrollbar` and a place in the order things go. `at_any_width_the_docks_settle_and_hold_their_tools_apart` then checks
it at every width.

**A new window over the pages:** a toggle in the page bar or a menu, not the toolbar (R16); its
scrolling must not move the pages (R16); its stored text in `Doc` and `DocFile` (R9); its width
through `ui::window_width`, and `ui::finger_close` first in its contents if it has a cross (R32).

**Something new sent with a scene** (like the cast): build it in `scene_prompt` from the subject
only (R13, R17), check it in `Backdrop::asked` in a test.

**A new kind of story on the welcome page:** a `Kind` variant in `ALL` and each match (`setup`,
`body`, `fonts`, `title`, `about`); a test that begins one and checks its pages and first text (R27).

**Something new drawn on the desk around the page** (like a peek): draw it in the settled
branch of `frame`, behind the paper or in front of it as it should hide or not; end it when the
page turns; leave it out of print (R20 is for what is on the page) (R28).

**A new theme:** a `Theme` variant in `ALL`, its `name`, `about` and `palette`; run
`the_docks_stay_readable_in_every_theme` (R29). **Something new drawn around the page:** its colours
from `theme::pal()` (R29).

**A new hidden gem** (`eggs.rs`): only in its magic theme; on the desk before the pages, or over
the settled page after them; a pose as a function of time that tests can look at, ending at once
when the writer goes on; nothing moved in the layout, nothing in print or the file (R30). A theme
whose thing grows a step a page and finishes when the next chapter begins keeps its state in a
`Tended`, as the Magic Snow and the Magic Vineyard do. Lay it all out unzoomed and zoom it at once,
as the vineyard does, and keep its shapes few enough for a frame (R30).

**Something new the Worker keeps online** (a file, a setting): keyed with `ledger::key` under the
user, its R2 reads and writes counted by the Ledger first, deleted with the user's stories, and a
test that its limit refuses and leaves R2 alone (R33). Then `node --test web/api/api.test.mjs`.

**Something new the Worker asks Claude for** (like the scenes): a count in the Ledger that says yes
before the call and is never given back (and in R2 for a day without the Ledger, `tally.js`), its size and `max_tokens` fixed in the Worker, only the
result sent back, a stand-in answer in `api.test.mjs` and in `cloud::stand_in` (R33).

**Something new pinned to the text** (like notes and scenes): move it in `apply_raw` and in
`into_doc` (R7), save it (R9), decide its Word mapping (R10).

---

## 6. Tests

- `cargo test` runs everything headlessly (about 480 tests, 13 to 35 s as busy as the machine is). They drive the real `App`
  through an egui `Context`: `layout::with_ctx` for model/layout tests, `Harness` (in `main.rs`)
  for app-level ones (`frames`, `key`, `type_text`, `click_in_page`, `scrub_to`, `long_contents`,
  `shown`, and `click_widget`, which finds a button by its label or a field by its hint through
  egui's AccessKit tree; `painted` holds the last frame's shapes, to check what is drawn and
  where it is clipped).
- Prefer tests of what the user sees (page shown, caret, text) over internals, so they survive
  redesigns. Name them as sentences of the behaviour.
- For anything incremental, compare with doing it from scratch.
- Before trusting a new test, check it fails without the fix.
- No theme program's module is run in the tests (R36): each stands for its theme's source played
  natively (`themes/played`), or for what the test says (`platform::program::stand_in`);
  `node --test web/theme-worker.test.mjs` checks that the browser's host makes the same words of the
  real modules; `themes/build.sh` makes those modules, and CI checks they are what is checked in.
  No test runs the compiler either: `platform::compiler::stand_in`, and the sources of `themes/`
  build to their fixtures.
- `cargo mutants` (config in `.cargo/mutants.toml`) finds untested code. `/tmp` is a small tmpfs
  here: run it with `TMPDIR` on disk.

---

## 7. Decisions

- **2026-10-06 — Contents are a page setting, not text.** First built as a char in the flow
  ("a real page in the story"). With contents running over several pages, all of them shared that
  one char and every page/caret mapping needed special cases. Now `PageSetup::contents` and empty
  `Span`s before the story (R5, R6).
- **2026-10-07 — Scenes are part of the story, not one picture.** A new scene used to replace the
  one before, and it was saved beside the document. Now every scene is kept, pinned to the
  paragraph it was painted for like a note, and covers the story up to the next; the page
  shown shows the caret's (R14). Pinned to a char,
  not a page (pages reflow) or a chapter (too coarse, and not every story has them).
- **2026-10-07 — Turning pages leaves the caret.** Swipes, the scrollbar and Page Up/Down used to
  put the caret at the turned-to page's start. With scenes following the caret (R14), that changed
  the picture on every page turn, and it lost the writer's place. Now only a click, typing or
  moving the caret moves it (R5).
- **2026-10-08 — Brainstorming with Claude lives in the answer panel and on post-its.** Connecting
  scenes, replies and drafts are one conversation in the panel, resent in full each time rather
  than resumed (`--resume` would need Claude Code to keep sessions on disk). What is worth keeping
  is pinned as a post-it on the passage, so ideas stay with the story without a store of their
  own. What Claude should always know is the story notes, saved with the document (R13). Claude
  sees only the passage marked, never more of the document: the writer decides what it reads.
- **2026-10-09 — The same people in every scene, from SVG model sheets.** Looked at storing
  people as image embeddings: they only help find things, and an image model that keeps a face
  needs the reference picture itself (or a model trained on it), which Claude, painting in SVG,
  cannot use. Tried instead, on four passages that only name one woman: from the passage alone
  she was someone else in every picture; with her look in words her clothes held but her build
  and drawing changed; with a model sheet Claude copied its shapes and colours and changed only
  the pose. So the cast holds a look in words and a sheet in SVG, found by name (R17).
- **2026-10-09 — Someone's looks are switched by hand, not by place in the story.** A look could
  instead start at a paragraph, like a scene, so repainting chapter 1 later still finds the girl.
  Switching by hand came first as what the writer asked for, and is simpler; pinning a look to
  where it starts can come on top (a look pinned is in use from there on).
- **2026-10-14 — Review and What next read more than a marked passage.** A writer asking for a
  chapter's mistakes or for where the story could go next wants Claude to read the chapter, or the
  story so far; making them select it first would be busywork. The writer still decides: the
  button says what it sends (R13, R19). The scratchpad goes with the ideas only while ticked, as it
  is notes to oneself.
- **2026-10-15 — Printing makes its own PDF and hands it to CUPS.** The system print dialogs
  (GTK's, macOS's `NSPrintOperation`) would each need their own bindings and lay the pages out
  again by their rules. Drawing the PDF from Caprice's own page layouts prints exactly the pages
  shown, the same PDF serves "Save as PDF", and `lp` is there on both macOS and Linux. The cost: the
  window offers only what Caprice asks for (printer, pages, copies, both sides), and adding a
  printer is done in the system's settings (R20).
- **2026-10-16 — A missing font is drawn with its twin, under its own name.** Writing on a Mac
  and on Linux, Times New Roman was missing on one and Liberation Serif on the other, and each
  fell back to egui's font, so pages broke differently. Renaming the font in the document would
  have lost the writer's choice in Word; drawing it with a font of the same widths keeps both (R21).
- **2026-10-17 — Autosave, and versions outside the story's folder.** Work was lost to a crash
  or a wrong "Don't save". Now a story with a file saves itself, and versions go to the data
  folder by story id rather than beside the file: no clutter in the writer's folders, versions
  follow a renamed file, and a story never saved has them too. Leaving still asks, but whether
  to keep or drop the changes since saving by hand, as the file holds them already (R22).
- **2026-10-19 — Text boxes lie in front of the text, pinned to a char.** Word offers text that
  wraps around a box; egui cannot flow text around anything (R11), and making pagination aware of
  boxes would tie page breaks to where a box was dropped. So a box lies in front of the text
  (Word's "In front of text", which is how it is exported), and it is pinned to a char like a
  scene, not to a page, so it goes along when the text moves to another page. Headers and footers
  likewise take no room: they sit in the margins (R24). "Page numbers" became a footer.
- **2026-10-20 — Text boxes in front of the text or behind it; pictures stay in the text.** Asked
  for: a box's text before or behind a picture. Pictures are paragraphs of the story, so making
  them float would bring back what the box decision avoided (R11). Instead a box is in one of two
  layers, behind or in front of the story with its pictures, as Word's "Behind text" and "In front
  of text", and ordered among boxes by its place in the list. Transparent lets the text under a
  box in front show through (R24).
- **2026-10-21 — Page numbering is pinned to the text, not to page numbers.** Asked for: numbers
  starting again, and pages without them (the contents, an appendix). Ranges of pages ("none from
  page 120") would hide the wrong pages as soon as text before them grew or shrank, so a change
  is pinned to a char like a text box, and holds until the next one, as Word's sections do. Word
  gets sections; "no numbers" keeps counting, as Word does (R26).
- **2026-10-22 — Opened without a file, Caprice welcomes the writer.** It used to show an empty
  story at once, and going back to yesterday's story meant the file dialog every time. A small
  modal over the empty desk came first; asked for instead was a page of its own, telling more
  about each story, with ways to begin that set things up differently. So the welcome stands in
  for the desk, a new story is begun as a kind (its settings, nothing stored about the kind), and
  the sheet chosen flies to the desk so the welcome visibly turns into the page (R27).
- **2026-10-22 — The cast peeks out as puppets in parts.** Asked for: someone of the story
  looking out from behind the page now and then, as a proper animation. A model sheet is a still
  figure; cut from it, a person could only slide and tilt. So Claude draws each look once more,
  peeking, in named parts, and Caprice moves the parts: the eyes look about and blink, the head
  tilts, a hand holds the edge. It costs one more drawing per look, asked for in the Cast window,
  never by itself (R12, R28).
- **2026-10-23 — Themes change the desk and docks, not the page.** Asked for: playful, colourful
  themes. Tinting the paper would make the screen disagree with print and Word (R20), and a story's
  colours are the writer's choice per text, so a theme is the app's mood around the page, kept per
  person in the data folder rather than in the story (R29).
- **2026-10-27 — Easter eggs live in a theme of their own.** Asked for: little hidden gems, a
  garden growing with the pages, a crane lifting a new chapter into place. In a theme of their own
  they cannot surprise a writer who wants a quiet desk, the garden can be drawn for that theme's
  colours, and choosing another theme turns them off without one more setting. The garden is there
  all the time, as it grows with the story; the crane comes only now and then, so it stays a
  surprise (R30).
- **2026-10-27 — The garden is kept with the story** (replaced on 2026-11-11, above). Asked for: it
  never shrinks, even when text and pages are taken out and the file is opened again. Worked out from the text alone it could
  not remember what was taken out, so each plant's most pages are saved with the story, measured in
  every theme, and left out of what counts as unsaved (R9, R30).
- **2026-11-11 — The garden is counted from the story when it is opened.** Replaces the decision
  below. A story whose two chapters were taken out and saved kept its two statues, and grew
  nothing new until it had more chapters than before; asked for: nothing comes down while the
  story is open, but opened again it shows its actual state. Kept in the file, the count also
  differed from browser to browser for the same text (it was no change, so never sent online),
  and saving while writing kept a count the text no longer held. So it is no longer saved.
- **2026-10-27 — Each magic theme brings a new chapter its own way.** The crane went to a Magic
  City of its own and the garden, renamed Magic Garden, got a hot-air balloon: one course in time
  for both, so the title is set down the same way, and only the path and the picture differ (R30).
- **2026-10-28 — The Magic City builds itself a floor a page.** Asked for: little magic houses with
  flickering windows, a new floor each new page, made of light flowing out of a window. The floors
  are counted from the garden's saved pages per chapter rather than a field of their own, so the
  city is kept with the story and never comes down, with nothing new in the file (R9, R30). The
  houses fill the desk on both sides first, a few floors each; once there is no room for another,
  they grow higher rather than crowding closer (asked for: squeezed-in houses overlapped, and a
  story without chapters built only on the left).
- **2026-10-30 — A dragon brings the Magic City's new chapters.** Asked for: no more crane, but a
  dragon flying in like the balloon, wings beating, breathing fire, turning its head to the city,
  the writer and the page, the title in its feet. It takes the balloon's path and course in time,
  so the two carriers differ only in their picture and where the dragon looks (R30). Then asked
  for: the dragon at every new chapter, and more lifelike. The balloon stayed a rare surprise for a
  day, then was asked for at every new chapter too (2026-10-31), so neither has odds of its own.
  Then: it flew too fast, and its body should move elegantly. It got its own pace rather than slowing the
  balloon too, and its wings, fire and tail kept real time, so it does not move in slow motion.
  Then: its flight was too like the balloon's; it should go with its wings. So its course is
  pushed on by each down beat, and it rises and sinks with the beats, carrying the title.
- **2026-10-31 — The Magic Sea builds a ship a chapter.** Asked for: a ship built up in the sea
  theme, something added with each new page, and when a new chapter starts the ship before it
  finished at once and released to the sea. The parts are counted from the garden's saved pages
  per chapter, like the city's floors, so nothing new is saved (R9, R30); a ship has a fixed number
  of parts, so a long chapter's ship is simply whole. Finished ships stay in the bay for good,
  whatever happens to their chapters, as plants and floors do.
- **2026-11-05 — The Magic Forest: a lantern a page, a big lantern in the trees a chapter.** Asked for first: an
  animal of light a chapter and a tree a page; found unimaginative (a forest planted on an empty
  desk) and another animal a chapter no event. Then tried: a sleeping forest woken by lights, a
  path to a new place each chapter, fireflies writing each title; then a river of light a chapter
  with fish, and small animals walking. Kept: the forest, the moon, the swarm of lanterns, a
  lantern a page. Chosen for a chapter: the swarm becomes a big lantern in a great tree, whose
  light brightens the trunks and finds something hidden in the tree; then, rather than one tree,
  the lanterns spread over the forest's own trees, and what they find need not be of the forest
  (a pocket watch, a teapot, a ship in a bottle), each with a little life of its own. Found not to
  look well and left out: a door, a nest, a bat, a book in a hollow. Then all of them were too
  many details: only the lanterns stay, the trees nearest the page got more branches so that more
  big lanterns hang near it. An owl bringing each new title from its perch and sitting by the new
  lantern was tried and taken out again: nothing brings a title in this theme. A word from its
  page on each small lantern and the chapter's title on each big one, on round paper lanterns, was
  tried too and taken out again: the plain small lanterns were liked better. All of it is counted from
  the garden's saved pages per chapter, so nothing new is saved and nothing goes out (R9, R30).
  The swarm is a wave of the time rather than a wander kept from frame to frame, so a test can
  look at any moment.
- **2026-11-07 — The Magic Reef: a coral a chapter, a branch a page, spawning when it is done.**
  Asked for: a magic coral reef theme in the spirit of the others. Kept to what the forest's
  history taught, few details and a chapter an event: the page grows something in sight (a branch
  out of a glimmer, like a floor of light in the city), and the chapter's end is the reef's own
  event, a coral spawning, after which a shoal comes to live about it. A sunlit shallow sea rather
  than the Lagoon's deep water, so the two do not look alike. Counted from the garden's saved pages
  per chapter, so nothing new is saved and nothing goes away (R9, R30). Then: the fish turned too
  abruptly, so they turn through facing the writer; and an octopus was asked for, appearing from
  behind the page to put each new chapter title in. Behind the page rather than from beyond the
  desk, so its head is drawn before the paper and only its arm in front. Then: the reef grew
  only outwards; corals now fill rows further back as the Magic Sea's ships do, each keeping its
  place, rather than squeezing closer together and shrinking as the chapters grew. Then: the
  rocks and seagrass always in the foreground, and the octopus coming in from the top. It swims
  down from above in front of the page rather than peeking over its top edge, which lies just
  under the format bar and would have hidden it; with it went the title fading in and the
  octopus drawn behind the page.
- **2026-11-08 — The Magic Vineyard.** Asked for: a challenging magic theme, as realistic as
  possible: a hilly vineyard under soft clouds, a single person harvesting slowly in the rows in
  the first chapter, someone new with each chapter, new grapes with each page. A Magic Vineyard was
  made (a stretch of vines a chapter, a bunch a page, pickers drawn round in three dimensions, the
  newcomer harvesting the chapter before's grapes into its crate), in a golden autumn afternoon.
  Every picker staying crowded the rows after a few chapters; a cart taking the old crates away
  and only the newest picker staying was tried, and the people were still not liked. Of four
  scenes without people (an olive grove, wheat rolled into hay bales, lavender and bees, the
  vineyard alone) an olive grove was made (a tree a chapter, olives a page, shaken into a net) and
  its trees not liked; then the wheat: the land, light and clouds stayed, a stretch of wheat a
  chapter, taller a page, rolling itself up into a bale. Round bales and waving wheat are simple
  shapes, colour and movement, which this way of drawing does well, where trees and people need a
  detail it does not. Realism is sought in the light (the sun low to one side, haze and mist with
  distance, long shadows towards the writer) and in the wind. Nothing brings a title, as in the
  snow (R30). The vineyard without people was then asked back as a theme of its own beside the
  wheat: the stretch before turns gold and red and drops its leaves, its grapes darkening and
  staying, with starlings now and then; the country the two shared went into `eggs/country.rs`.
  The wheat field was then taken out, the vineyard without people kept.
- **2026-11-11 — The Magic Snow's frozen lake.** The Christmas trees and penguins were not
  liked. Of four ideas (an ice palace under the northern lights, snowmen, mountain cabins, a frozen
  lake with skaters) the lake was picked, changed by the writer: blocks of ice cut out of it, a
  page each, collecting beside it, and an ice figure made of them at the end of a chapter. The
  figures are animals carved from convex parts, so the ice reveals itself by cutting each part at
  a rising level rather than by a mask, which egui has not got. The places are taken from either
  side of the page in turn: bigger ice in a short story otherwise all fell on one side. The lake
  first reached beyond the window's edges; asked for: the whole lake in sight, so it is laid out
  from the window rather than from the hillside's middle. Then asked for: at least twenty
  figures, appearing at random, each with a light of a different colour glowing inside; there are
  twenty, dealt shuffled so a story sees every one before any comes back. The seal did not read as
  one, and the statues were taken out for igloos: a brick a page (still a block cut out of the
  lake), the igloo finished and a light lit inside when the next chapter begins. Its bricks are
  bands of a half oval, so the dome is round without a mask, and its joints glow because each
  brick lies over an outline of itself in the joint's colour.
- **2026-11-07 — The Magic Snow.** Asked for: two more magic themes, creative, quirky and
  imaginative. A Magic Cosmos (a planet a chapter, a moon a page, a flying saucer) and a Magic Hive
  (a comb a chapter, a cell of honey a page, bees) were made first and not liked; of six proposals
  then (origami, a circus, a pond, snow, a workshop, a dinosaur island) a Magic Origami (a paper
  animal folded a fold a page, coming to life, and a paper aeroplane unfolding into each title) and
  the Magic Snow were made, and the Origami taken out again. The Snow's snowmen (a snowball a
  page, dressed with a lantern lit when done) were not liked either; of six alternatives (igloos,
  gingerbread cottages, fir trees, ice sculptures, a penguin colony, a ski lodge) the cottages were
  made; then asked for instead: Christmas trees spread about rather than in a front row, each new
  one pulled in and raised by a few penguins, an ornament a page, the latest lit as the next is
  brought, and no penguin sledging in each title. The penguins pull it in from the side away from
  the page, so they never pass behind it; the tree comes lying down, its top trailing, so raising
  it on ropes reads as hauling it up. Nothing brings a title now, as in the Magic Forest. Its state
  is in a `Tended` rather than five fields, as the reef's is, so the next such theme can share it.
  Counted from the garden's saved pages per chapter, so nothing new is saved (R9, R30).
- **2026-10-31 — Caprice Web is a copy, not a mode of the desktop app.** The browser version was
  asked for with the desktop Caprice left unchanged, so it is an independent copy of it (at
  `5c87d3d`) that builds both ways, rather than code shared with the desktop app. Its background
  work runs in Web Workers (first `wasm_thread`, then our own `platform/worker.rs`, as
  `wasm_thread` evaluates code, which the Content-Security-Policy forbids): tried first in a spike, which found egui's `Context`
  cannot go to a worker and a lock the main thread waits on crashes Chromium (R31). Chromium is
  the target, Firefox nice to have, Safari not supported. "Follow my writing" is left out for now.
- **2026-11-03 — On a phone the page keeps its pages.** Asked for: Caprice friendly on a phone.
  Fitting the page to the width, scrolling it, following the caret and panning with two fingers
  was chosen over a mode that reflows the text to the screen without pages, which would be a new
  way of drawing the story (R32).
- **2026-11-03 — On a touch screen one finger swipes, a long press selects.** Asked for: a one-finger
  swipe to the page before or after. A finger dragged over the text used to select, as a mouse
  does; now a swipe turns a page or moves the page, and a selection begins with the finger held
  still, as on phones. A mouse still selects by dragging (R32).
- **2026-11-04 — Stories online: open sign-up, Google and GitHub, R2.** Asked for: signing in and
  keeping `.caprice` files online, safe and cheap, with no invite list. Cloudflare Access was the
  first choice, but its free plan stops new sign-ups at 50 users; Supabase, Firebase, Clerk and
  Auth0 run scripts from other origins (against the CSP) and hold the stories elsewhere. So the
  Worker signs in with Google and GitHub itself (redirects only), keeps files in R2 and every
  count in one Durable Object, with limits that keep R2 inside its free amounts. The browser
  stays where a story is written; online is a copy (R33).
- **2026-11-09 — Scenes in the browser: Claude through the Worker, 50 a day.** Asked for: painting
  scenes in the browser, with a hard limit of 50 pictures a day in all, any writer taking as many as
  are left. Claude's SVGs, as on the desktop, rather than an image service traced to vectors
  (`SCENES_PLAN.md`), because the prompt, the cast and the look were there. The writer must be
  signed in (Google), the count is in the Ledger before Claude is asked and counts failures too,
  and each request's cost is fixed in the Worker, so 50 scenes are a ceiling on the bill. "Follow
  my writing" stays off in the browser: it would use the day's scenes in minutes (R33).
- **2026-11-10 — Moving scenes: Claude marks the parts, Caprice moves them.** Asked for: a
  dragon's wings beating, smoke from chimneys, clouds drifting. resvg runs no SVG animation, and
  rendering the picture anew each frame (blurs, turbulence) is far too slow, in the browser above
  all. So the picture is rendered once in layers and the parts move as textures; the kinds of
  movement are few and fixed, and people and animals on the ground stay still, since walking cut
  out of a painting looks like a puppet. Tried first on a dragon over a village and two people on
  a bridge: Claude marked wings, chimneys, clouds and lights well, and left the people alone (R14).
  Then: a dragon's wings beating once a second, without end, were too distracting to write by.
  So wings beat by their size, mostly glide and turn smoothly, and the whole picture rests while
  the writer writes, starting again slowly half a minute after the last edit.
- **2026-11-11 — A page that lost its GPU loads again, rather than eframe being forked.** On a
  phone, Caprice came back from the background as a white page: Android had taken the GPU, and
  eframe 0.36 neither notices nor makes a new one. Making one in place would mean forking eframe
  to rebuild its painter and every texture egui and the app hold. A reload gets a new one for
  nothing; what it would lose (the story shown, unsaved words, the caret, the device in the
  story's room) is written down first and read back (R31). Only undo is lost.
- **2026-11-12 — One story in every browser; a clash keeps the newer, the other a week as a
  backup.** A story changed in two browsers used to be kept twice, the other as "<name> (from
  another browser)", a story of its own on every shelf. Nothing was lost, but the writer had to
  sort out copies by hand, and they piled up. Most clashes came from a browser opening its older
  copy and being written in before it looked online, so a story opened now waits for a newer copy
  first. What still clashes (written in two places while out of reach) keeps the newer, by when
  each was saved, and the other's words among the story's backups for a week, behind "⋯" in
  Versions (R22, R33). Merging the two was not tried: the room already decided nothing is merged
  (R34), and a backup is enough to take words back from.
- **2026-11-14 — Themes of one's own are one drawing that Caprice plays, not code.** Asked for:
  writers making magic themes themselves, by upload or with Claude or other tools, later shared
  in a gallery to vote on. The built-in themes are Rust drawing shapes; a writer's cannot be code
  (the browser's CSP forbids evaluating any, R31), so a theme is data: one SVG, which every drawing
  program opens and saves, any AI chat can write from the guide, and one check can judge. All seven
  magic themes have the same shape (a thing a chapter, a part a page, a change when the next
  chapter begins, something bringing the title), so that is the shape a theme of one's own takes:
  a desk that must move (a still picture was not wanted), and optionally a piece and a carrier.
  Its motion is `motion.rs`'s, with more kinds for a picture looked at for hours. The theme
  workshop (clicking parts to set their motion) and the online gallery come later; the format
  stays as it is for them (R35).
- **2026-11-14/15 — Themes made by Claude are programs, run in the browser's engine, with Caprice's
  own host; not components.** Themes of one's own (one drawing, R35) fell well short of the
  built-in themes, which are code. So a theme may be a program (R36), written by Claude and compiled
  in the writer's tab (the plan's step 4), drawing shapes as the built-in themes do. Measured first
  (`THEME_PROGRAMS_PLAN.md`, steps 0 to 0d): wasmi inside Caprice was forty times too slow, the
  browser's own engine in a worker of its own holds a frame in 2.5 ms at a quarter of the speed;
  jco's components could share one glue between themes but took 11–14 ms a frame (63–77 posting
  their objects), so the interface is WIT, shipped as core modules with Caprice's own hosts, and a
  theme's source is kept so it can be wrapped as a component later. egui tessellating the Snow's
  2 000 shapes costs 1.6–3.5 ms at a quarter speed, less than a built-in theme's own drawing on the
  frame; the limit was 40 000 points a frame (450 000 since the Vineyard's port, below). Program
  themes are the browser's only.
- **2026-11-15 — A program names its ground, and has a window layer of its own.** Porting the Magic
  Snow (the plan's step 2) showed what the first interface lacked: the Snow zooms about its ground,
  14 units above the bar, and its snow falls in the window, unzoomed. A program could have made up
  for both with sums of its own (shifting all it draws by the gap between the two points, drawing
  the snow zoomed backwards), which Claude would get wrong; so the theme names its ground in units
  above the foot of the desk (`frame-output.ground`; units, not a point, as a point would be measured
  in a layout unzoomed about the point itself), Caprice says where that is (`desk.ground`) and where
  the window is (`desk.window`), and `frame-output.window` is never zoomed. A path (a line through
  points, joined) was added too: the Snow's see-through outlines drawn as segments showed knots
  where they met. The world was not yet published, so `1.0.0` itself changed.
- **2026-11-15 — No WebAssembly in the tests; a frame may draw 450 000 points.** Porting the Magic
  Vineyard (the plan's step 2's second half) needed nothing new of the world, but two things of
  Caprice. Its frame under wasmi, the tests' engine, took 264 ms and more fuel than the tests gave:
  so wasmi was dropped (the owner's call, proposed on 2026-11-14), and each module stands in the tests
  for its theme's source built for the machine, the browser's host held to the same words through
  the echo theme (R36); a desktop that plays program themes one day brings an engine of its own.
  And it draws 136 000 to 420 000 points a frame, nearly all leaves as mesh vertices, against the
  40 000 a program could: the limit became a little more than the most a built-in theme draws, so a
  program may cost Caprice's frame what its own themes do, and keeping a theme light moved to where
  it is made (step 5). A limit by the window's area was weighed and dropped: a theme's points grow
  with the land it shows, not the window's size, so it would have stopped themes in small windows
  zoomed out; drawing fewer leaves when they are small would have made them change while zooming.
- **2026-11-16 — The toolchain is packed and served by Caprice itself, and ended after each build.**
  The plan had the investigation's release gain the theme std and the SDK; packing them in Caprice
  (`toolchain/pack.mjs`) instead keeps the SDK where it is written and lets the packing check every
  fixture against what it packs, and the toolchain's id covers all of it. It is served as Caprice's
  own static assets (free, no R2 read counted), in brotli pieces decoded in the tab, packed from the
  investigation's GitHub release (free to keep and fetch for good, unlike CI's caches and artifacts,
  which expire, or Git LFS, whose free bandwidth CI would use up in a month), and the page
  names the current toolchain in `compiler/toolchain.json`, outside `/toolchain/*`, whose rule keeps
  files for good (rules add their headers together). Its glue is the investigation's TypeScript made
  JavaScript once and kept beside `theme-worker.js`, so the web build needs no bundler. The compiler
  holds a gigabyte while it lives, so it is ended as soon as a build has answered: loading it again
  from Cache Storage costs under two seconds.
- **2026-11-16 — Theme programs never come from or go to files.** A writer imports drawings
  (`.svg`) and nothing else; a program is made with Claude in Caprice or taken from the gallery, and
  is shared only by publishing it there (R36). Theme files (`.caprice-theme`) and sources (`.rs`)
  were imported and exported until then (steps 3 and 4). A program anyone could hand anyone else
  would be played unchecked on their shelf; through the gallery each is rebuilt from its source,
  played and looked at before anyone else plays it, and a theme made with Claude is built in the
  writer's own tab from what Claude wrote. The theme file stays as the form a program travels in
  (to the gallery and back) and the ports' fixtures.
- **2026-11-16 — The gallery signs core modules, and its owner reviews in Caprice.** What the
  writer's tab builds and the reader plays is a core module (the world `caprice:theme@1.0.0` as
  wit-bindgen lays it out), so that is what is signed and taken: a component would be a second form
  to check, build and keep in step for nothing a theme needs. Each module still carries the world's
  type and wraps into a component whenever one is wanted (R36); measured on 2026-11-19, that pays
  only with a host that is a component too, linked to the theme by the engine (jco's JavaScript
  between them, or a JavaScript host given the theme's values as objects, is slower than today's). The owner's review is in the app
  (the Waiting tab), with their session, rather than a page with an admin token: it needs the
  compiler and the preview that are there already, and no token sits in a browser. The first step
  is the loop alone (send, review, sign, list, Use); votes, reports, remixes, taking a theme out
  and Claude's opinion before the owner's come after.
- **2026-11-18 — On a phone the docks are redesigned, not replaced.** The desktop's docks,
  shrunk, left a phone with File and "⋯" over a wide gap, a "+" between gaps below, and the
  formatting two taps away. A phone's own kind of bar was weighed against the docks laid out
  again: the floating docks are part of how Caprice and its themes look, and the narrow width,
  the keyboard's state and the second row were all there already. So under `view::NARROW` the
  toolbar is File, Undo, Redo, the name and "⋯"; the page bar is the count, the track and small
  buttons, and the formatting while writing, over the keyboard, where the thumbs are.
- **2026-11-18 — A phone on its side puts the docks away while the writer writes.** In Chrome on
  Android the keyboard left Caprice a strip of about 95 points, less than the two docks: no text
  showed. One bar over the keyboard still left two lines. So the docks go, a round button at the
  top left brings them back by putting the keyboard away, and the formatting is one tap further
  than upright. The page fills the width up to 100%. Fullscreen, which gives back the browser's
  own bars, is the next step, apart: it rests on what the browser allows, and is checked on a phone.
- **2026-11-19 — Themes made with Claude are free at first; what they cost is measured.** The
  owner wants momentum before charging. Payment (a provider, VAT, terms) waits until prices can be
  set from real numbers, rather than being built and left switched off: the Ledger keeps what each
  theme cost and whether it was kept (R33), the daily limits bound what free costs, and the window
  says it is free for now. Themes kept while it is free stay the writer's. Billing, when it comes,
  is a free allowance and payment beyond it, charged for a theme kept, never for rounds.
- **No `.docx` import.** Export only.
- **Contents heading is "Contents"** (English), in the app and in Word.
