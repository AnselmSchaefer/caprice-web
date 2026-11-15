//! Echo: a theme that draws back what Caprice tells it, so the tests can read the canonical ABI's
//! layouts through it (`tests/fixtures/echo.wasm`, made by `themes/build.sh`). Each value comes
//! back as a circle: where it is and how large say the numbers; its colour says which.
//!
//! Behind the page: 1 the desk (at its top left, as large as its width), 2 the page (the same), 3
//! the zoom and the size of things (at those two), 4 the bar if it shows (at its top left), 5 the
//! time (r), 6 the story's seed (its low and high halves, as numbers), 7 its pages and how many
//! chapters the garden has, 8 each chapter's pages (at its number), then each event heard since
//! the last frame: 10 fresh, 11 grown (chapter, pages), 12 begun (chapter), 13 clicked (the point).
//! Over the page, while a title is carried: 20 the pose (enter, run), 21 (lower, hook), 22 (look,
//! t), 23 hooked and from the left, 24 the title (at its top left, as large as its width). The
//! carried title is offset by (5, 7), and the next frame wanted in 250 ms.

use theme_sdk::{Color, Event, Input, Out, Theme, host, pt, theme};

struct Echo {
    heard: Vec<Event>,
}

fn which(k: u8) -> Color {
    Color::rgb(k, 0, 0)
}

impl Theme for Echo {
    fn new() -> Self {
        Echo { heard: Vec::new() }
    }

    fn event(&mut self, event: &Event) {
        self.heard.push(*event);
    }

    fn frame(&mut self, input: &Input, out: &mut Out) {
        let d = &mut out.desk;
        d.circle(input.area.min, input.area.width(), which(1));
        d.circle(input.page.min, input.page.width(), which(2));
        d.circle(pt(input.zoom, input.unit), 1.0, which(3));
        if let Some(bar) = input.bar {
            d.circle(bar.min, bar.width(), which(4));
        }
        d.circle(pt(0.0, 0.0), input.time as f32, which(5));
        let seed = host::story_seed();
        d.circle(pt((seed & 0xffff_ffff) as u32 as f32, (seed >> 32) as u32 as f32), 1.0, which(6));
        let garden = host::garden();
        d.circle(pt(host::page_count() as f32, garden.len() as f32), 1.0, which(7));
        for (k, &pages) in garden.iter().enumerate() {
            d.circle(pt(k as f32, pages as f32), 1.0, which(8));
        }
        for e in self.heard.drain(..) {
            match e {
                Event::Fresh => d.circle(pt(0.0, 0.0), 1.0, which(10)),
                Event::Grown(p) => d.circle(pt(p.chapter as f32, p.pages as f32), 1.0, which(11)),
                Event::Begun(k) => d.circle(pt(k as f32, 0.0), 1.0, which(12)),
                Event::Clicked(at) => d.circle(at, 1.0, which(13)),
            }
        }
        if let Some(c) = input.carrier {
            host::ask_lift();
            let (p, o) = (c.pose, &mut out.over);
            o.circle(pt(p.enter, p.run), 1.0, which(20));
            o.circle(pt(p.lower, p.hook), 1.0, which(21));
            o.circle(pt(p.look, p.t), 1.0, which(22));
            o.circle(pt(f32::from(u8::from(p.hooked)), f32::from(u8::from(c.from_left))), 1.0, which(23));
            o.circle(c.title.min, c.title.width(), which(24));
        } else if host::ask_lift() {
            // Asked every frame, so Caprice knows it carries titles.
        }
        out.title_offset = pt(5.0, 7.0);
        out.next = 250;
    }
}

theme!(Echo);
