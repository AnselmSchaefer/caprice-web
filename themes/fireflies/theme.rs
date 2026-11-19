//! Fireflies: a theme program, small enough to read in one go, that Caprice's tests run
//! (`tests/fixtures/fireflies.wasm`, made by `themes/build.sh`). A night sky over a dark meadow;
//! for each chapter a jar beside the page, a light in it for each page the chapter has had, the
//! newest coming in one after another; a jar whose chapter is finished glows warmer. Fireflies
//! drift about the desk, and one big firefly carries each new chapter title down into its place.

use std::f32::consts::TAU;

use theme_sdk::{Color, Event, Input, Mesh, Out, Seeds, Tended, Theme, between, ease, host, pt, theme};

/// The most lights a jar holds, seconds for one to come, and for a finished jar to warm.
const LIGHTS: usize = 12;
const STEP: f32 = 0.8;
const WARM: f32 = 1.5;

const SKY_TOP: Color = Color::rgb(8, 12, 32);
const SKY_FOOT: Color = Color::rgb(30, 36, 70);
const MEADOW: Color = Color::rgb(10, 22, 16);
const GLASS: Color = Color::premultiplied(60, 80, 90, 110);
const LIGHT: Color = Color::rgb(230, 240, 120);
const WARM_LIGHT: Color = Color::rgb(255, 190, 90);

struct Fireflies {
    jars: Tended,
}

impl Theme for Fireflies {
    fn new() -> Self {
        Fireflies { jars: Tended::default() }
    }

    fn event(&mut self, event: &Event) {
        if let Event::Fresh = event {
            self.jars.forget();
        }
    }

    fn frame(&mut self, input: &Input, out: &mut Out) {
        // The sky and the meadow reach the edges of what shows; everything else is placed on the desk
        // as it lies at the fitting zoom, so it stays where it is as the writer zooms.
        let (area, shown, page, u, t) = (input.area, input.shown, input.page, input.unit, input.time);
        let seed = host::story_seed();
        let garden = host::garden();
        // This theme brings new chapter titles.
        host::ask_lift();

        // The sky, light at its foot: one mesh.
        let mut sky = Mesh::new();
        let a = sky.vertex(shown.min, SKY_TOP);
        let b = sky.vertex(pt(shown.right(), shown.top()), SKY_TOP);
        let c = sky.vertex(shown.max, SKY_FOOT);
        let d = sky.vertex(pt(shown.left(), shown.bottom()), SKY_FOOT);
        sky.triangle(a, b, c);
        sky.triangle(a, c, d);
        out.desk.mesh(sky);
        // The meadow's edge is its ground: 30 units above the foot of the desk, where Caprice zooms
        // about, so it stays where it is on the screen.
        out.ground = 30.0;
        let ground = input.ground.y;
        out.desk.polygon(vec![pt(shown.left(), ground), pt(shown.right(), ground - 8.0 * u), shown.max, pt(shown.left(), shown.bottom())], MEADOW);

        // A jar a chapter, beside the page, taking turns left and right and out from it, where it is
        // seen when its chapter begins (and there it stays).
        self.jars.tend(seed, &garden, LIGHTS, STEP, WARM, t);
        let finishing = self.jars.finishing(t, WARM);
        let (w, h) = (46.0 * u, 70.0 * u);
        let places: Vec<f32> = (0..24).map(|j| {
            let out_by = (j / 2) as f32 * (w + 14.0 * u) + w / 2.0 + 18.0 * u;
            if j % 2 == 0 { page.left() - out_by } else { page.right() + out_by }
        }).collect();
        let n = garden.len().max(1);
        for k in 0..n {
            let at = places[self.jars.place(k, places.len(), |p| places[p] - w / 2.0 > shown.left() && places[p] + w / 2.0 < shown.right())];
            let warm = match finishing {
                Some((m, q)) if m == k => ease(q),
                _ if k + 1 >= n => 0.0,
                _ => 1.0,
            };
            let foot = pt(at, ground);
            out.desk.polygon(vec![foot + pt(-w / 2.0, 0.0), foot + pt(w / 2.0, 0.0), foot + pt(w / 2.0, -h), foot + pt(-w / 2.0, -h)], GLASS);
            out.desk.polygon(vec![foot + pt(-w * 0.4, -h), foot + pt(w * 0.4, -h), foot + pt(w * 0.4, -h - 8.0 * u), foot + pt(-w * 0.4, -h - 8.0 * u)], Color::rgb(90, 60, 40));
            let grown = self.jars.grown(&garden, k, LIGHTS);
            let mut s = Seeds::of(seed, k);
            for i in 0..grown.ceil() as usize {
                let came = (grown - i as f32).clamp(0.0, 1.0);
                let (x, y, phase) = (s.next(-0.35, 0.35) * w, s.next(0.1, 0.9) * h, s.next(0.0, TAU));
                let blink = 0.7 + 0.3 * (t as f32 * 2.0 + phase).sin();
                let color = LIGHT.lerp(WARM_LIGHT, warm).gamma_multiply(came * blink);
                out.desk.circle(foot + pt(x, -y), (2.5 + 1.5 * warm) * u, color);
            }
        }

        // Fireflies drifting about the desk, slowly round where each began.
        let mut s = Seeds::of(seed, 1000);
        for _ in 0..30 {
            let (x, y, r, phase) = (s.next(area.left(), area.right()), s.next(area.top(), ground), s.next(10.0, 40.0) * u, s.next(0.0, TAU));
            let a = t as f32 * 0.4 + phase;
            out.desk.circle(pt(x + r * a.cos(), y + r * (a * 1.3).sin()), 1.6 * u, LIGHT.gamma_multiply(0.4 + 0.4 * (a * 3.0).sin().abs()));
        }

        // The big firefly carrying a title: in from its side above the page, over the title's place,
        // down into it, then up and away.
        if let Some(c) = input.carrier {
            let p = c.pose;
            let side = if c.from_left { -1.0 } else { 1.0 };
            let title = c.title;
            let away = (1.0 - p.enter) * (area.width() * 0.6) + (1.0 - between(p.run, 0.0, 1.0)) * page.width() * 0.5;
            let held = pt(side * away, -(1.0 - p.lower) * title.height() * 3.0);
            if p.hooked {
                out.title_offset = held;
            }
            let top = pt(title.center().x, title.top()) + if p.hooked { held } else { pt(0.0, -p.hook * area.height()) };
            for k in 0..4 {
                out.over.circle(top + pt(0.0, -14.0 * u), (18.0 - 4.0 * k as f32) * u, LIGHT.gamma_multiply(0.1 + 0.2 * k as f32));
            }
            out.over.line(top + pt(0.0, -6.0 * u), top, 1.2 * u, Color::rgb(200, 200, 160));
        }
        // The fireflies never rest.
        out.next = 33;
    }
}

theme!(Fireflies);
