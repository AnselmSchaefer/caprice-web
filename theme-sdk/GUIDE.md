# Writing a magic theme for Caprice

Caprice is a word processor for writing stories. Its pages lie on a desk, and a *magic theme*
fills that desk with a little world that grows as the story does: the Magic Snow builds an igloo a
chapter beside a frozen lake, a block of ice cut for each page; the Magic Vineyard plants a stretch
of vines a chapter, a bunch of grapes ripening for each page, the stretch before turning gold when
the next chapter begins. A writer looks at it for hours while writing, and now and then notices that
it has grown. You write such a theme: one Rust file, against the SDK below, compiled to WebAssembly
and played by Caprice in a sandbox. It can only draw shapes; Caprice decides everything else.

## What a theme is

- **A thing a chapter, a part a page, a change when the next chapter begins.** The story's growth
  is the *garden*: `host::garden()`, the most pages each chapter has had, never shrinking while the
  story is open (one entry for a story without chapters). The last entry is the chapter being
  written; every one before it is finished. Draw a thing for each chapter, grown by its pages, and
  finish the one before when a new chapter begins (`Event::Begun`). `Tended` (in the SDK) keeps all
  of that for you, as the built-in themes do: use it.
- **It never moves anything and is never printed.** It lies on the desk behind every page, or, for
  a carrier only, over the page on the screen. The page in the middle hides whatever is behind it,
  so put what matters beside the page: on its left and right, and above and below where the window
  is tall. The page bar (`Input::bar`) lies along the foot of the desk (or a side): anything that
  grows up from the foot starts above its top.
- **It is calm.** Slow, gentle motion: drifting snow, a lantern's glow breathing, starlings now and
  then. Nothing flashing, nothing fast, nothing that pulls the eye from the page for long. Growth is
  the event: a block floats up and settles, an igloo lights, a stretch turns gold over seconds.
- **It is the same for the same story.** Lay things out from `host::story_seed()` with `Seeds`, so
  each story has its own world and it stays the same each time the story is opened, in every
  browser. Never use the time to choose where things go.
- **`Event::Fresh` shows everything as it stands, growing nothing**: the theme was picked or the
  story opened. Then `Grown` (a chapter has more pages than ever) and `Begun` (a new chapter) are
  what the writer just did: those animate.
- **A carrier is optional.** A theme that calls `host::ask_lift()` (it answers whether Caprice will
  bring titles with it) brings each new chapter's title onto the page: while `Input::carrier` is
  `Some`, draw the carrier in `Out::over` following its `pose` (in from the side, over the title's
  place, the title lowered, let go, away) and say where the title hangs in `Out::title_offset`.
  The built-in themes use a balloon, a dragon, an octopus. Leave it out unless it fits the world.

## Laying it out

- Lay everything out for the desk as it lies at the fitting zoom: `Input::area` (the desk that
  shows), `Input::page`, `Input::bar`. Caprice zooms what you draw on the desk itself, about the
  ground you name (`Out::ground`, in units above the foot of the desk, or the bar's top when the bar
  lies along it); never zoom anything yourself. What belongs to the window rather than the world
  (snow falling, rain) goes in `Out::window`, laid out on `Input::window` and never zoomed.
- Size things by `Input::unit` (1 on a desk 900 points high), never by pixels: the same theme runs
  on a phone (a desk about 412 by 915, the page filling its width) and on a wide screen (1920 by
  1080 and more). On a phone there is little room beside the page: grow things above it and below
  it, or let them stand partly behind it.
- A theme draws its own sky if it likes (one mesh with a colour at each corner costs nothing);
  otherwise the desk shows the palette's two colours (`desk_top` at the top, `desk` at the foot).
- Keep state between frames in your theme's struct; the frame is called many times a second while
  anything moves. Set `Out::next` to the milliseconds until you next need a frame when nothing
  moves (0 while anything does).

## Drawing

- Only shapes: circles, convex polygons, lines, paths, and meshes of coloured triangles. No text,
  no pictures, no fonts. Draw everything from shapes, as the examples do.
- Polygons must be convex (egui fills only convex ones): cut anything else into convex pieces or
  triangles of a mesh. Prefer `Layer::solid` for filled shapes with corners; `oval` and `radial`
  are soft ellipses and glows.
- Colours are premultiplied sRGB (`Color::rgb`, `Color::premultiplied`, `see_through`,
  `gamma_multiply`, `lerp`). A gradient is a mesh, its colour at each vertex.
- **Points cost.** A frame may draw at most 450 000 points (a polygon's points, a path's points, a
  mesh's vertices; a circle or a line counts one), but every point is paid for in the writer's own
  frame, on their phone too. Keep a frame within about 100 000 points on a phone's desk and 200 000
  on a computer's, zoomed out included. A field of many small fills is cheaper as one mesh.
  Polygons cost the most: egui smooths every edge.

## What you may not do

- Import nothing but the SDK: the file names no other crate, uses no `unsafe`, no files, no clock,
  no randomness but `Seeds`. `std` is there; there is no WASI.
- Draw nothing that could be mistaken for Caprice's own controls, and no words or letters made of
  shapes.
- Nothing violent, gory, frightening or unkind, whatever is asked: it lies beside someone's story
  for hours.

## How you answer

Answer with exactly two fenced blocks and nothing else of length:

1. a ```json block: the theme's palette and its name, `{"name": "...", "about": "a few words",
   "colours": {...}}`, the colours by these names as `#rrggbb`: `desk_top`, `desk` (the desk's two
   colours, which show before the first frame: they should agree with the sky you draw), `dock`,
   `dock_edge` (the toolbar and page bar), `button`, `button_hover`, `field`, `text`, `text_dim`,
   `text_hot`, `accent`, `on_accent`, `caret`, `thumb`, and `light`: `"yes"` if the toolbar is light
   with dark text, else `"no"`. The text must read clearly on the dock, and `on_accent` on `accent`.
2. a ```rust block: the whole theme, one file, starting `use theme_sdk::*;` and ending with
   `theme!(YourTheme);`. Always the whole file, never a part of it or a change to it. Its comments
   are `//` ones: a `//!` after the `use` does not build.

It is built with Rust 1.99 (edition 2024): `rustc --crate-type cdylib --target
wasm32-unknown-unknown -O` against the SDK. If it does not build, you are shown the compiler's
messages and give the whole file again, fixed. If it builds, you are shown pictures of it at a few
moments of a story (begun; three chapters written; a chapter finishing; a title carried, if it
carries titles) and its points on a phone's desk and a computer's, and asked to make it better.
