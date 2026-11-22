//! The gallery of theme programs (R36, the plan's step 6): themes made with Claude, sent by their
//! writers, and taken onto anyone's shelf with "Use". Everything goes through Caprice's server
//! (`web/api/gallery.js`, for writers signed in): a theme is sent as its theme file (`bundle.rs`),
//! which the Worker checks as far as it cheaply can and queues for the site's owner.
//!
//! The owner looks at each in their own browser, here (`Review`), never trusting what the writer's
//! browser sent but the source: it is built again with the same toolchain and must be the very
//! module sent, byte for byte; it is played through the preview's story, and its pictures and its
//! points shown. Only then may it be published, and the Worker signs it over the hashes it worked
//! out itself. A theme taken from the gallery goes on the shelf with that signature, which the
//! shelf checks every time it is read (`own::Signed`): changed by a byte, it is no longer the
//! gallery's. Taken, it is the writer's to keep, offline too, whatever becomes of the gallery: the
//! owner may take a theme out of it again ("Take down…"), and those who took it keep it.

use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{self, TextureHandle, TextureOptions, vec2};
use serde::Deserialize;

use crate::App;
use crate::eggs::preview::{Preview, Previewed};
use crate::platform::compiler::{Built, Compiler, State};
use crate::platform::{Answer, Request, Waker};
use crate::theme::{Theme, pal};

/// The points a frame of a theme is asked to keep within (the plan's step 5): about the Magic
/// Vineyard's, on a phone's desk and on a computer's, zoomed out included. A version made with
/// Claude that draws more goes back to be made lighter (`making.rs`), and the guide names the same
/// numbers (`theme-sdk/GUIDE.md`).
pub const BUDGET: (usize, usize) = (100_000, 320_000);
/// What a writer sending a theme is told of what happens to it.
pub const WHAT_HAPPENS: &str = "Caprice builds it again from its source, plays it and looks at it before it shows in the \
    gallery. Then every writer signed in can see it and use it, under the name you give here. Its source goes with it.";

/// A theme in the gallery, as it is listed: what the shelf needs to check its signature too.
#[derive(Clone, Deserialize, Debug)]
pub struct Listed {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub colours: BTreeMap<String, String>,
    /// The SHA-256 of its source, the toolchain that built it and the gallery's signature.
    pub source: String,
    pub toolchain: String,
    pub signature: String,
}

impl Listed {
    fn signed(&self) -> crate::own::Signed {
        crate::own::Signed { name: self.name.clone(), source: self.source.clone(), toolchain: self.toolchain.clone(), signature: self.signature.clone() }
    }
}

/// A theme the writer sent, and what became of it.
#[derive(Clone, Deserialize, Debug)]
struct Mine {
    name: String,
    state: String,
    #[serde(default)]
    why: String,
}

/// A theme waiting for the owner, with what the Worker found of it.
#[derive(Clone, Deserialize, Debug)]
struct Waiting {
    id: String,
    name: String,
    #[serde(default)]
    about: String,
    #[serde(default)]
    author: String,
    /// The SHA-256 of its module, as the Worker worked it out.
    module: String,
    toolchain: String,
    #[serde(default)]
    size: u64,
}

#[derive(Clone, Copy, PartialEq, Default)]
enum Tab {
    #[default]
    New,
    Mine,
    Waiting,
}

/// A request on its way, and what to do with its answer.
type Asked = Option<Receiver<Answer>>;
/// A theme file read off the frame.
type Reading = Receiver<Result<crate::bundle::Opened, Vec<String>>>;

/// A theme being taken onto the shelf.
enum Taking {
    Fetching(Listed, Receiver<Answer>),
    Reading(Listed, Reading),
}

/// Where the owner's look at a theme waiting is.
enum Look {
    Fetching(Receiver<Answer>),
    Reading(Reading),
    /// Built again from its source: the compiler ends with it.
    Building(Box<(Compiler, crate::bundle::Opened)>),
    Playing(Box<Preview>),
    Seen { previewed: Previewed, textures: Vec<TextureHandle> },
    Failed,
}

/// The owner's look at one theme waiting: what was found so far, whether it may be published, and
/// the decision on its way.
struct Review {
    theme: Waiting,
    look: Look,
    found: Vec<String>,
    /// Built again the same, byte for byte, and played: the SHA-256 of what was built.
    same: Option<String>,
    why: String,
    deciding: Asked,
    /// Whether the decision on its way is to publish it.
    publishing: bool,
}

/// Sending a theme on the shelf to the gallery: what it is to be called there, and the answer.
pub struct Sending {
    id: String,
    name: String,
    about: String,
    author: String,
    asked: Asked,
    said: Option<String>,
    sent: bool,
}

/// The Gallery window and what it is doing.
#[derive(Default)]
pub struct Gallery {
    pub open: bool,
    tab: Tab,
    listing: Asked,
    themes: Vec<Listed>,
    /// Why the list could not be had, or what came of taking a theme.
    said: Option<String>,
    pictures: HashMap<String, TextureHandle>,
    picturing: Vec<(String, Receiver<Answer>)>,
    taking: Option<Taking>,
    mine: Vec<Mine>,
    mine_asked: Asked,
    waiting: Vec<Waiting>,
    waiting_asked: Asked,
    review: Option<Review>,
    pub sending: Option<Sending>,
    /// The theme the owner asked to take down, waiting for a yes; then the request on its way.
    down_asked: Option<String>,
    taking_down: Option<(Listed, Receiver<Answer>)>,
}

impl Gallery {
    /// Close the window; the owner's look ends with it, and the compiler it may hold.
    pub fn close(&mut self) {
        self.open = false;
        self.review = None;
        self.sending = None;
    }

    /// Whether a theme is being taken, or looked at by the owner.
    pub fn busy(&self) -> bool {
        self.taking.is_some() || self.taking_down.is_some() || self.listing.is_some() || self.review.as_ref().is_some_and(|r| !matches!(r.look, Look::Seen { .. } | Look::Failed))
    }

    /// No theme is being looked at: the owner's decision is in.
    #[cfg(test)]
    pub fn review_is_none(&self) -> bool {
        self.review.is_none()
    }

    /// How many of the themes listed have their pictures.
    #[cfg(test)]
    pub fn pictured(&self) -> usize {
        self.pictures.len()
    }
}

/// The SHA-256 of `bytes`, in hex, as the Worker writes it.
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// What the server said, as a sentence for the writer.
fn said_of(answer: &Answer) -> String {
    let said = String::from_utf8_lossy(&answer.body).trim().to_owned();
    match answer.status {
        0 => "Caprice's server could not be reached. Check the connection and try again.".into(),
        401 => "You are signed out: sign in again (File \u{203a} Stories online).".into(),
        _ if !said.is_empty() && said.len() < 300 => format!("{}.", said.trim_end_matches('.')),
        code => format!("Caprice's server could not do it ({code})."),
    }
}

/// `bytes` read as a theme file, off the frame.
fn read_off_frame(ctx: &egui::Context, bytes: Vec<u8>, name: String) -> Reading {
    let (tx, rx) = channel();
    let wake = Waker::new(ctx);
    crate::platform::spawn(move || {
        let _ = tx.send(crate::bundle::open(&bytes, &name));
        wake.wake();
    });
    rx
}

/// What has come of a request, if it has.
fn arrived(asked: &mut Asked) -> Option<Answer> {
    let answer = asked.as_ref()?.try_recv().ok()?;
    *asked = None;
    Some(answer)
}

impl App {
    fn ask_server(&self, ctx: &egui::Context, method: &'static str, path: String, body: Option<(&'static str, Vec<u8>)>) -> Asked {
        let server = self.cloud.server.clone()?;
        let (tx, rx) = channel();
        let headers = body.as_ref().map(|(kind, _)| vec![("Content-Type", (*kind).to_owned())]).unwrap_or_default();
        server.send(Request { method, path, headers, body: body.map(|b| b.1) }, tx, Waker::new(ctx));
        Some(rx)
    }

    /// Open the Gallery window, listing what is in it now.
    pub fn open_gallery(&mut self, ctx: &egui::Context) {
        self.gallery.open = true;
        self.gallery.said = None;
        self.list_gallery(ctx);
    }

    fn list_gallery(&mut self, ctx: &egui::Context) {
        if self.cloud.who.is_none() {
            return;
        }
        self.gallery.listing = self.ask_server(ctx, "GET", "/api/gallery".into(), None);
        if self.gallery.tab == Tab::Mine {
            self.gallery.mine_asked = self.ask_server(ctx, "GET", "/api/gallery/mine".into(), None);
        }
        if self.cloud.who.as_ref().is_some_and(|w| w.owner) {
            self.gallery.waiting_asked = self.ask_server(ctx, "GET", "/api/gallery/waiting".into(), None);
        }
    }

    /// Theme `id` on the shelf, to be sent to the gallery: the window that asks what it is called.
    pub fn send_to_gallery(&mut self, id: &str) {
        let Some(e) = self.themes.own.get(id) else { return };
        let author = self.cloud.who.as_ref().and_then(|w| w.name.split_whitespace().next().map(str::to_owned)).unwrap_or_default();
        let author = if e.author.is_empty() { author } else { e.author.clone() };
        self.gallery.sending = Some(Sending { id: id.to_owned(), name: e.name.clone(), about: e.about.clone(), author, asked: None, said: None, sent: false });
    }

    fn send_now(&mut self, ctx: &egui::Context) {
        let Some(s) = &self.gallery.sending else { return };
        let Some(mut bundle) = self.themes.own.bundle(&s.id) else { return };
        let why = if bundle.source.is_none() {
            Some("It has no source, which the gallery builds it again from.")
        } else if bundle.preview.is_none() {
            Some("It has no picture yet: try again in a moment.")
        } else {
            None
        };
        let s = self.gallery.sending.as_mut().unwrap();
        if let Some(why) = why {
            s.said = Some(why.into());
            return;
        }
        let m = &mut bundle.manifest;
        (m.name, m.about, m.author) = (s.name.trim().to_owned(), s.about.trim().to_owned(), s.author.trim().to_owned());
        match crate::bundle::write(&bundle) {
            Ok(file) => {
                let asked = self.ask_server(ctx, "POST", "/api/gallery".into(), Some(("application/zip", file)));
                let s = self.gallery.sending.as_mut().unwrap();
                s.said = None;
                s.asked = asked;
            }
            Err(e) => self.gallery.sending.as_mut().unwrap().said = Some(format!("It could not be packed: {e}.")),
        }
    }

    /// Take a theme from the gallery onto the shelf: its file fetched, read, its signature checked.
    fn take(&mut self, ctx: &egui::Context, listed: Listed) {
        if let Some(e) = self.themes.own.signed(&listed.signature) {
            let id = e.id.clone();
            self.themes.choose_own(ctx, &id);
            return;
        }
        if let Some(rx) = self.ask_server(ctx, "GET", format!("/api/gallery/{}/theme", listed.id), None) {
            self.gallery.said = None;
            self.gallery.taking = Some(Taking::Fetching(listed, rx));
        }
    }

    /// The owner looks at a theme waiting: its file fetched first.
    fn review(&mut self, ctx: &egui::Context, theme: Waiting) {
        let Some(rx) = self.ask_server(ctx, "GET", format!("/api/gallery/{}/theme", theme.id), None) else { return };
        self.gallery.review = Some(Review { theme, look: Look::Fetching(rx), found: Vec::new(), same: None, why: String::new(), deciding: None, publishing: false });
    }

    /// The owner takes a published theme out of the gallery.
    fn take_down(&mut self, ctx: &egui::Context, listed: Listed) {
        self.gallery.down_asked = None;
        if let Some(rx) = self.ask_server(ctx, "DELETE", format!("/api/gallery/{}", listed.id), None) {
            self.gallery.said = None;
            self.gallery.taking_down = Some((listed, rx));
        }
    }

    fn decide(&mut self, ctx: &egui::Context, publish: bool) {
        let Some(r) = &self.gallery.review else { return };
        let said = if publish {
            serde_json::json!({ "publish": true, "module": r.same.clone().unwrap_or_default() })
        } else {
            serde_json::json!({ "publish": false, "why": r.why.trim() })
        };
        let asked = self.ask_server(ctx, "POST", format!("/api/gallery/waiting/{}", r.theme.id), Some(("application/json", said.to_string().into_bytes())));
        if let Some(r) = &mut self.gallery.review {
            (r.deciding, r.publishing) = (asked, publish);
        }
    }

    /// Once a frame: what the server answered, a theme taken, the owner's look going on.
    pub fn tend_gallery(&mut self, ctx: &egui::Context) {
        let g = &mut self.gallery;
        if let Some(answer) = arrived(&mut g.listing) {
            match answer.status {
                200 => {
                    #[derive(Deserialize)]
                    struct List {
                        themes: Vec<Listed>,
                    }
                    g.themes = serde_json::from_slice::<List>(&answer.body).map(|l| l.themes).unwrap_or_default();
                }
                _ => g.said = Some(said_of(&answer)),
            }
        }
        if let Some(answer) = arrived(&mut g.mine_asked)
            && answer.status == 200
        {
            #[derive(Deserialize)]
            struct List {
                themes: Vec<Mine>,
            }
            g.mine = serde_json::from_slice::<List>(&answer.body).map(|l| l.themes).unwrap_or_default();
        }
        if let Some(answer) = arrived(&mut g.waiting_asked)
            && answer.status == 200
        {
            #[derive(Deserialize)]
            struct List {
                themes: Vec<Waiting>,
            }
            g.waiting = serde_json::from_slice::<List>(&answer.body).map(|l| l.themes).unwrap_or_default();
        }
        // Each theme's picture, asked once.
        let wanted: Vec<String> = g.themes.iter().filter(|t| !g.pictures.contains_key(&t.id) && !g.picturing.iter().any(|p| p.0 == t.id)).map(|t| t.id.clone()).collect();
        for id in wanted {
            if let Some(rx) = self.ask_server(ctx, "GET", format!("/api/gallery/{id}/preview"), None) {
                self.gallery.picturing.push((id, rx));
            }
        }
        let g = &mut self.gallery;
        let mut got = Vec::new();
        g.picturing.retain(|(id, rx)| match rx.try_recv() {
            Ok(answer) => {
                got.push((id.clone(), answer));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        for (id, answer) in got {
            if answer.status == 200
                && let Ok(image) = image::load_from_memory_with_format(&answer.body, image::ImageFormat::Png)
            {
                let rgba = image.to_rgba8();
                let picture = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
                g.pictures.insert(id.clone(), ctx.load_texture(format!("gallery-{id}"), picture, TextureOptions::LINEAR));
            }
        }
        if let Some((listed, rx)) = &g.taking_down {
            match rx.try_recv() {
                Ok(answer) => {
                    if answer.status == 200 {
                        g.themes.retain(|t| t.id != listed.id);
                        g.pictures.remove(&listed.id);
                        g.said = Some(format!("\u{201c}{}\u{201d} is taken out of the gallery. Writers who took it keep it.", listed.name));
                    } else {
                        g.said = Some(said_of(&answer));
                    }
                    g.taking_down = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => g.taking_down = None,
            }
        }
        self.tend_taking(ctx);
        self.tend_sending();
        self.tend_review(ctx);
    }

    fn tend_taking(&mut self, ctx: &egui::Context) {
        match self.gallery.taking.take() {
            Some(Taking::Fetching(listed, rx)) => match rx.try_recv() {
                Ok(answer) if answer.status == 200 => {
                    let reading = read_off_frame(ctx, answer.body, listed.name.clone());
                    self.gallery.taking = Some(Taking::Reading(listed, reading));
                }
                Ok(answer) => self.gallery.said = Some(said_of(&answer)),
                Err(std::sync::mpsc::TryRecvError::Empty) => self.gallery.taking = Some(Taking::Fetching(listed, rx)),
                Err(_) => {}
            },
            Some(Taking::Reading(listed, rx)) => match rx.try_recv() {
                Ok(Ok(opened)) => {
                    let signed = listed.signed();
                    // Kept only as the gallery signed it: what anyone else could have changed on the way is not taken.
                    if !signed.by(&self.themes.own.gallery_key, &opened.bundle.module) {
                        self.gallery.said = Some(format!("\u{201c}{}\u{201d} is not as the gallery signed it, so it was not taken.", listed.name));
                        return;
                    }
                    // The writer's own theme, published, is not put beside itself: it becomes the gallery's.
                    let shelf = &mut self.themes.own;
                    let kept = match shelf.sign(&signed, &opened.bundle.module) {
                        Some(id) => Ok(id),
                        None => shelf.add_bundle(&opened.bundle, Some(signed)),
                    };
                    match kept {
                        Ok(id) => {
                            self.themes.choose_own(ctx, &id);
                            self.status = format!("- theme \u{201c}{}\u{201d} taken from the gallery", listed.name);
                        }
                        Err(e) => self.gallery.said = Some(format!("\u{201c}{}\u{201d} could not be taken: {e}.", listed.name)),
                    }
                }
                Ok(Err(why)) => self.gallery.said = Some(format!("\u{201c}{}\u{201d} could not be taken: {}", listed.name, why.join(" "))),
                Err(std::sync::mpsc::TryRecvError::Empty) => self.gallery.taking = Some(Taking::Reading(listed, rx)),
                Err(_) => {}
            },
            None => {}
        }
    }

    fn tend_sending(&mut self) {
        let Some(s) = &mut self.gallery.sending else { return };
        let Some(answer) = arrived(&mut s.asked) else { return };
        if answer.status == 202 {
            #[derive(Deserialize)]
            struct Sent {
                id: String,
            }
            let gallery = serde_json::from_slice::<Sent>(&answer.body).map(|s| s.id).unwrap_or_default();
            s.sent = true;
            s.said = Some("Sent. It waits to be looked at; the Gallery says what became of it, under \u{201c}Sent by you\u{201d}.".into());
            let id = s.id.clone();
            self.themes.own.set_sent(&id, &gallery);
        } else {
            s.said = Some(said_of(&answer));
        }
    }

    fn tend_review(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some(r) = &mut self.gallery.review else { return };
        if let Some(answer) = arrived(&mut r.deciding) {
            let decided = answer.status == 200;
            r.found.push(if decided { "Decided.".into() } else { said_of(&answer) });
            if decided {
                let (id, name, published) = (r.theme.id.clone(), r.theme.name.clone(), r.publishing);
                self.gallery.review = None;
                self.gallery.waiting.retain(|w| w.id != id);
                // What was decided, in its own words: a yes is not told as a no is.
                self.gallery.said = Some(if published {
                    format!("\u{201c}{name}\u{201d} is published: it is in the gallery now, under New.")
                } else {
                    format!("\u{201c}{name}\u{201d} is refused: its writer reads why under \u{201c}Sent by you\u{201d}.")
                });
                self.list_gallery(ctx);
            }
            return;
        }
        match std::mem::replace(&mut r.look, Look::Failed) {
            Look::Fetching(rx) => match rx.try_recv() {
                Ok(answer) if answer.status == 200 => r.look = Look::Reading(read_off_frame(ctx, answer.body, r.theme.name.clone())),
                Ok(answer) => r.found.push(said_of(&answer)),
                Err(std::sync::mpsc::TryRecvError::Empty) => r.look = Look::Fetching(rx),
                Err(_) => {}
            },
            Look::Reading(rx) => match rx.try_recv() {
                Ok(Ok(opened)) => {
                    if sha256(&opened.bundle.module) != r.theme.module {
                        r.found.push("Its program is not the one the server took in.".into());
                        return;
                    }
                    r.found.push(format!("Its program checks, {} KB; its source is {} KB.", opened.bundle.module.len().div_ceil(1024),
                        opened.bundle.source.as_deref().map_or(0, str::len).div_ceil(1024)));
                    let mut compiler = Compiler::start(Waker::new(ctx));
                    compiler.build(opened.bundle.source.as_deref().unwrap_or_default());
                    r.look = Look::Building(Box::new((compiler, opened)));
                }
                Ok(Err(why)) => r.found.push(format!("It cannot be read: {}", why.join(" "))),
                Err(std::sync::mpsc::TryRecvError::Empty) => r.look = Look::Reading(rx),
                Err(_) => {}
            },
            Look::Building(building) => {
                let (mut compiler, opened) = *building;
                let state = compiler.state(now);
                match (compiler.take(), state) {
                    (Some(Built::Module { module, toolchain, ms }), _) => {
                        let sent = &opened.bundle;
                        if toolchain != sent.manifest.toolchain {
                            r.found.push(format!("It was built by the toolchain {}; this Caprice builds with {toolchain}, so it cannot be built the same here.", sent.manifest.toolchain));
                        } else if module != sent.module {
                            r.found.push("Built again from its source, it is not the program sent: it must not be published.".into());
                        } else {
                            r.found.push(format!("Built again from its source in {:.1} s: the very program sent, byte for byte.", ms as f32 / 1000.0));
                            let palette = crate::own::with_colours(Theme::Midnight.palette(), &sent.manifest.colours).0;
                            r.same = Some(sha256(&module));
                            r.look = Look::Playing(Box::new(Preview::start(&module, palette, Waker::new(ctx), now)));
                        }
                    }
                    (Some(Built::Errors(said)), _) => r.found.push(format!("It does not build from its source: {}", said.lines().next().unwrap_or_default())),
                    (None, State::Failed { why, .. }) => r.found.push(format!("It could not be built here: {why}")),
                    (None, _) => {
                        r.look = Look::Building(Box::new((compiler, opened)));
                        ctx.request_repaint_after(std::time::Duration::from_millis(250));
                    }
                }
            }
            Look::Playing(mut preview) => match preview.tend(now) {
                Some(Ok(previewed)) => {
                    let (phone, computer) = (previewed.phone_points, previewed.computer_points);
                    let over = phone > BUDGET.0 || computer > BUDGET.1;
                    r.found.push(format!("It plays its story. At most {phone} points a frame on a phone's desk, {computer} on a computer's{}.",
                        if over { format!(": more than the {} and {} a theme is asked to keep within", BUDGET.0, BUDGET.1) } else { String::new() }));
                    let textures = previewed.pictures.iter().map(|p| ctx.load_texture(format!("review-{}", p.name), p.image.clone(), TextureOptions::LINEAR)).collect();
                    r.look = Look::Seen { previewed, textures };
                }
                Some(Err(why)) => {
                    r.same = None;
                    r.found.push(format!("Caprice stopped it while playing it: {why}."));
                }
                None => {
                    r.look = Look::Playing(preview);
                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                }
            },
            seen @ Look::Seen { .. } => r.look = seen,
            Look::Failed => {}
        }
        // Whatever went wrong, it may not be published.
        if matches!(r.look, Look::Failed) {
            r.same = None;
        }
    }

    /// The Gallery window, and the window that sends a theme to it.
    pub fn gallery_window(&mut self, ctx: &egui::Context) {
        self.sending_window(ctx);
        if !self.gallery.open {
            return;
        }
        let signed_in = self.cloud.who.is_some();
        let owner = self.cloud.who.as_ref().is_some_and(|w| w.owner);
        let mut open = true;
        let mut act = None;
        let g = &mut self.gallery;
        let shelf = &self.themes.own;
        // Three cards to a row, or two on a phone's screen.
        let wide = crate::ui::window_width(ctx, 660.0);
        let gap = if wide < 500.0 { 10.0 } else { 14.0 };
        let columns = (((wide + gap) / (150.0 + gap)).floor() as usize).clamp(1, 3);
        // Room is left at the right for the scroll bar, which would otherwise lie over the last card.
        let bar = ctx.global_style().spacing.scroll.allocated_width() + 2.0;
        let card = ((wide - bar - gap * (columns - 1) as f32) / columns as f32).floor();
        let high = (ctx.content_rect().height() * 0.62).clamp(240.0, 560.0);
        let id = egui::Id::new("theme_gallery_window");
        let mut closed = false;
        egui::Window::new("Gallery").id(id).open(&mut open).collapsible(false).resizable(false).default_width(wide)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            closed = crate::ui::finger_close(ui, id);
            ui.set_max_width(wide);
            if !signed_in {
                ui.add_space(6.0);
                ui.label("Themes made in Caprice for writers using it, looked at by Caprice before they show here. The gallery is for writers signed in.");
                ui.add_space(8.0);
                if ui.add(accent_button("Sign in\u{2026}")).clicked() {
                    act = Some(GalleryAct::SignIn);
                }
                ui.add_space(4.0);
                return;
            }
            let before = g.tab;
            let listed = g.themes.len();
            egui::Sides::new().show(ui, |ui| {
                ui.selectable_value(&mut g.tab, Tab::New, "New");
                ui.selectable_value(&mut g.tab, Tab::Mine, "Sent by you");
                if owner {
                    ui.selectable_value(&mut g.tab, Tab::Waiting, format!("Waiting ({})", g.waiting.len()));
                }
            }, |ui| {
                if before == Tab::New && listed > 0 {
                    ui.label(egui::RichText::new(format!("{listed} theme{}", if listed == 1 { "" } else { "s" })).size(12.0).color(pal().text_dim));
                }
            });
            if g.tab != before {
                act = Some(GalleryAct::List);
            }
            ui.add_space(2.0);
            ui.separator();
            if g.tab == Tab::New {
                ui.label(egui::RichText::new("Made by writers in Caprice, built again and looked at before they show here. \
                    Taken, a theme is yours to keep.").size(12.0).color(pal().text_dim));
                ui.add_space(4.0);
            }
            if let Some(said) = &g.said {
                told(ui, said);
            }
            match g.tab {
                Tab::New => {
                    if g.listing.is_some() && g.themes.is_empty() {
                        waiting_for(ui, "Looking in the gallery\u{2026}");
                    } else if g.themes.is_empty() {
                        nothing(ui, "Nothing in the gallery yet", "Have a theme made in Yours, and send it here.");
                    }
                    if g.taking.is_some() {
                        waiting_for(ui, "Taking the theme\u{2026}");
                    }
                    egui::ScrollArea::vertical().max_height(high).min_scrolled_height(high).auto_shrink([false, true]).show(ui, |ui| {
                        egui::Grid::new("gallery_themes").num_columns(columns).spacing(vec2(gap, gap)).show(ui, |ui| {
                            for (k, t) in g.themes.iter().enumerate() {
                                let shown = Shown {
                                    picture: g.pictures.get(&t.id).map(|p| p.id()),
                                    on_shelf: shelf.signed(&t.signature).is_some(),
                                    may_take: g.taking.is_none(),
                                    owner: owner.then_some((g.down_asked.as_deref() == Some(t.id.as_str()), g.taking_down.is_none())),
                                };
                                theme_card(ui, t, card, &shown, &mut act);
                                if k % columns == columns - 1 {
                                    ui.end_row();
                                }
                            }
                        });
                        ui.add_space(4.0);
                    });
                }
                Tab::Mine => {
                    if g.mine.is_empty() {
                        nothing(ui, "You have sent no theme", "Send one from Yours: in the menu beside its name, \u{201c}Send to the gallery\u{2026}\u{201d}.");
                    }
                    egui::ScrollArea::vertical().max_height(high).min_scrolled_height(high).auto_shrink([false, true]).show(ui, |ui| {
                        for (k, m) in g.mine.iter().enumerate() {
                            if k > 0 {
                                ui.separator();
                            }
                            sent_row(ui, m);
                        }
                    });
                }
                Tab::Waiting => waiting_tab(ui, g, &mut act),
            }
        });
        self.gallery.open &= open && !closed;
        match act {
            Some(GalleryAct::SignIn) => self.cloud.open = true,
            Some(GalleryAct::List) => self.list_gallery(ctx),
            Some(GalleryAct::Take(t)) => self.take(ctx, t),
            Some(GalleryAct::Review(w)) => self.review(ctx, w),
            Some(GalleryAct::Decide(publish)) => self.decide(ctx, publish),
            Some(GalleryAct::Ask(id)) => self.gallery.down_asked = id,
            Some(GalleryAct::TakeDown(t)) => self.take_down(ctx, t),
            None => {}
        }
        if !self.gallery.open {
            // The owner's look ends with the window: the compiler it may hold goes with it.
            self.gallery.review = None;
        }
    }

    fn sending_window(&mut self, ctx: &egui::Context) {
        let Some(s) = &mut self.gallery.sending else { return };
        let mut open = true;
        let (mut send, mut done) = (false, false);
        let wide = crate::ui::window_width(ctx, 420.0);
        let field = (wide - 140.0).clamp(100.0, 280.0);
        let id = egui::Id::new("gallery_sending");
        egui::Window::new("Send to the gallery").id(id).open(&mut open).collapsible(false).resizable(false).default_width(wide)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            done |= crate::ui::finger_close(ui, id);
            ui.set_max_width(wide);
            ui.label(WHAT_HAPPENS);
            ui.add_space(4.0);
            egui::Grid::new("gallery_sending_fields").num_columns(2).spacing(vec2(8.0, 6.0)).show(ui, |ui| {
                ui.label("Called");
                ui.add_enabled(!s.sent, egui::TextEdit::singleline(&mut s.name).char_limit(60).desired_width(field));
                ui.end_row();
                ui.label("In a few words");
                ui.add_enabled(!s.sent, egui::TextEdit::singleline(&mut s.about).char_limit(140).desired_width(field));
                ui.end_row();
                ui.label("By");
                ui.add_enabled(!s.sent, egui::TextEdit::singleline(&mut s.author).char_limit(40).desired_width(field).hint_text("A writer"));
                ui.end_row();
            });
            if let Some(said) = &s.said {
                ui.label(egui::RichText::new(said).color(if s.sent { pal().text } else { pal().accent }));
            }
            ui.horizontal(|ui| {
                if s.asked.is_some() {
                    ui.spinner();
                    ui.label("Sending\u{2026}");
                } else if s.sent {
                    done = ui.button("Done").clicked();
                } else if ui.add_enabled(!s.name.trim().is_empty(), egui::Button::new("Send it")).clicked() {
                    send = true;
                }
            });
        });
        if send {
            self.send_now(ctx);
        }
        if !open || done {
            self.gallery.sending = None;
        }
    }
}

/// What the writer or the owner did in the Gallery window.
enum GalleryAct {
    SignIn,
    List,
    Take(Listed),
    Review(Waiting),
    Decide(bool),
    /// The owner asks to take a theme down (or thinks better of it), then says yes.
    Ask(Option<String>),
    TakeDown(Listed),
}

/// What a card shows of a theme besides the theme itself: its picture, whether it is on the shelf
/// already, whether one may be taken now, and for the owner whether they were asked to take it down
/// and may.
struct Shown {
    picture: Option<egui::TextureId>,
    on_shelf: bool,
    may_take: bool,
    owner: Option<(bool, bool)>,
}

/// How high a card's picture is for its width: as the shelf keeps them, 320 by 200.
const PICTURE: f32 = 0.625;

/// A theme in the gallery as a card `w` wide: its picture, its name, who made it, its words and
/// colours, and the button that takes it. Every part keeps its room, a long name or few words
/// alike, so the cards of a row are as high as each other and their buttons in line.
fn theme_card(ui: &mut egui::Ui, t: &Listed, w: f32, shown: &Shown, act: &mut Option<GalleryAct>) {
    let palette = crate::own::with_colours(Theme::Midnight.palette(), &t.colours).0;
    let p = pal();
    let fill = if p.light { p.field } else { p.btn };
    let round = 10u8;
    // A grid lays its cells out sideways: the card goes down.
    let framed = ui.vertical(|ui| egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, p.dock_edge)).corner_radius(round).show(ui, |ui| {
        ui.set_width(w - 2.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        let inner = w - 2.0;
        let (rect, picture) = ui.allocate_exact_size(vec2(inner, (inner * PICTURE).round()), egui::Sense::click());
        picture.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &t.name));
        let corners = egui::CornerRadius { nw: round - 1, ne: round - 1, sw: 0, se: 0 };
        crate::ui::paint_tile(ui.painter(), rect, palette, shown.picture, corners);
        if picture.hovered() && shown.may_take && !shown.on_shelf {
            ui.painter().rect_filled(rect, corners, egui::Color32::from_white_alpha(14));
        }
        let picture = picture.on_hover_cursor(egui::CursorIcon::PointingHand);
        if picture.clicked() && shown.may_take {
            *act = Some(GalleryAct::Take(t.clone()));
        }
        egui::Frame::new().inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 10 }).show(ui, |ui| {
            let text = inner - 24.0;
            ui.set_width(text);
            ui.add(egui::Label::new(egui::RichText::new(&t.name).size(14.0).strong()).truncate());
            ui.add(egui::Label::new(egui::RichText::new(format!("by {}", t.author)).size(11.5).color(p.text_dim)).truncate());
            ui.add_space(4.0);
            about_lines(ui, &t.about, text);
            ui.add_space(8.0);
            take_row(ui, t, &palette, text, shown, act);
            if let Some((asked, may)) = shown.owner {
                take_down_row(ui, t, asked, may, act);
            }
        });
    })).inner;
    if framed.response.hovered() {
        ui.painter().rect_stroke(framed.response.rect, round, egui::Stroke::new(1.0, p.text_dim), egui::StrokeKind::Inside);
    }
}

/// A theme's words in two lines at most, cut short with an ellipsis and whole when pointed at. The
/// two lines keep their room when the words are fewer, or none.
fn about_lines(ui: &mut egui::Ui, about: &str, w: f32) {
    let font = egui::FontId::proportional(12.0);
    let row = ui.fonts_mut(|f| f.row_height(&font));
    let mut job = egui::text::LayoutJob::single_section(about.to_owned(), egui::TextFormat::simple(font, pal().text));
    job.wrap = egui::text::TextWrapping { max_width: w, max_rows: 2, break_anywhere: false, overflow_character: Some('\u{2026}') };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let (rect, resp) = ui.allocate_exact_size(vec2(w, row * 2.0), egui::Sense::hover());
    let cut = galley.elided;
    ui.painter().galley(rect.min, galley, pal().text);
    if cut {
        resp.on_hover_text(about);
    }
}

/// The card's foot: the theme's colours, and "Use", or word that it is on the shelf already. The
/// colours are left out where the card is too narrow for both.
fn take_row(ui: &mut egui::Ui, t: &Listed, c: &crate::theme::Palette, w: f32, shown: &Shown, act: &mut Option<GalleryAct>) {
    egui::Sides::new().height(28.0).show(ui, |ui| {
        if w >= 150.0 {
            swatches(ui, c);
        }
    }, |ui| {
        if shown.on_shelf {
            ui.label(egui::RichText::new("On your shelf").size(12.0).color(pal().text_dim));
            tick(ui);
        } else if ui.add_enabled(shown.may_take, accent_button("Use")).on_hover_text("Put it on your shelf, and use it").clicked() {
            *act = Some(GalleryAct::Take(t.clone()));
        }
    });
}

/// The owner may take a theme out of the gallery again, once more asked: it goes for everyone.
fn take_down_row(ui: &mut egui::Ui, t: &Listed, asked: bool, may: bool, act: &mut Option<GalleryAct>) {
    ui.add_space(4.0);
    if asked {
        ui.label(egui::RichText::new("Out of the gallery for everyone?").size(11.0).color(pal().accent));
        ui.horizontal(|ui| {
            if ui.add_enabled(may, egui::Button::new("Take it down").small()).clicked() {
                *act = Some(GalleryAct::TakeDown(t.clone()));
            }
            if ui.add(egui::Button::new("Keep it").small()).clicked() {
                *act = Some(GalleryAct::Ask(None));
            }
        });
    } else if ui.add_enabled(may, egui::Button::new(egui::RichText::new("Take down\u{2026}").size(11.0).color(pal().text_dim)).small().frame(false)).clicked() {
        *act = Some(GalleryAct::Ask(Some(t.id.clone())));
    }
}

/// A theme's colours as a row of dots: its desk, dock, accent and caret.
fn swatches(ui: &mut egui::Ui, c: &crate::theme::Palette) {
    let colours = [c.desk_top, c.dock, c.accent, c.caret];
    let (rect, _) = ui.allocate_exact_size(vec2(colours.len() as f32 * 14.0, 12.0), egui::Sense::hover());
    for (k, colour) in colours.into_iter().enumerate() {
        let at = egui::pos2(rect.left() + 6.0 + k as f32 * 14.0, rect.center().y);
        ui.painter().circle(at, 5.5, colour, egui::Stroke::new(1.0, pal().dock_edge));
    }
}

/// A tick, drawn: the fonts may not have one.
fn tick(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
    let s = egui::Stroke::new(1.6, pal().accent);
    let (a, b, c) = (rect.left_center() + vec2(1.0, 0.5), rect.center_bottom() + vec2(-1.5, -2.5), rect.right_top() + vec2(-1.0, 2.5));
    ui.painter().line_segment([a, b], s);
    ui.painter().line_segment([b, c], s);
}

/// A button in the accent, for what a window is mostly for.
fn accent_button(word: &str) -> egui::Button<'_> {
    egui::Button::new(egui::RichText::new(word).color(pal().on_accent).strong()).fill(pal().accent).corner_radius(8).min_size(vec2(64.0, 28.0))
}

/// What came of something the writer did, set apart from the rest.
fn told(ui: &mut egui::Ui, said: &str) {
    egui::Frame::new().fill(pal().accent.gamma_multiply(0.14)).stroke(egui::Stroke::new(1.0, pal().accent.gamma_multiply(0.45)))
        .corner_radius(8).inner_margin(egui::Margin::symmetric(10, 7)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(egui::RichText::new(said).color(pal().text));
    });
    ui.add_space(6.0);
}

/// Something on its way, in the middle with room around it.
fn waiting_for(ui: &mut egui::Ui, what: &str) {
    ui.add_space(14.0);
    ui.vertical_centered(|ui| {
        ui.spinner();
        ui.label(egui::RichText::new(what).color(pal().text_dim));
    });
    ui.add_space(14.0);
}

/// An empty tab: what is not there, and what to do about it.
fn nothing(ui: &mut egui::Ui, what: &str, then: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(what).size(15.0).strong());
        ui.add_space(2.0);
        ui.label(egui::RichText::new(then).color(pal().text_dim));
    });
    ui.add_space(24.0);
}

/// A theme the writer sent, with what became of it in a coloured word, and why if it was not taken.
fn sent_row(ui: &mut egui::Ui, m: &Mine) {
    let (state, colour) = match m.state.as_str() {
        "public" => ("In the gallery", pal().accent),
        "refused" => ("Not taken", pal().text),
        "removed" => ("Taken out of the gallery", pal().text_dim),
        _ => ("Waiting to be looked at", pal().text_dim),
    };
    ui.add_space(2.0);
    egui::Sides::new().height(24.0).shrink_left().truncate().show(ui, |ui| {
        ui.label(egui::RichText::new(&m.name).strong());
    }, |ui| {
        egui::Frame::new().fill(colour.gamma_multiply(0.16)).corner_radius(10).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
            ui.label(egui::RichText::new(state).size(12.0).color(colour));
        });
    });
    if m.state == "refused" && !m.why.is_empty() {
        ui.label(egui::RichText::new(&m.why).size(12.0).color(pal().text_dim));
    }
    ui.add_space(2.0);
}

/// The owner's tab: the themes waiting, and the one being looked at.
fn waiting_tab(ui: &mut egui::Ui, g: &mut Gallery, act: &mut Option<GalleryAct>) {
    if g.waiting.is_empty() && g.review.is_none() {
        ui.label(egui::RichText::new("No theme is waiting.").color(pal().text_dim));
    }
    for w in &g.waiting {
        let looking = g.review.as_ref().is_some_and(|r| r.theme.id == w.id);
        // The button at the row's end, where it is whatever the name.
        egui::Sides::new().height(28.0).shrink_left().truncate().show(ui, |ui| {
            ui.label(egui::RichText::new(&w.name).strong());
            ui.label(egui::RichText::new(format!("by {}, {} KB, {}", w.author, w.size.div_ceil(1024), w.toolchain)).size(11.0).color(pal().text_dim));
        }, |ui| {
            if !looking && ui.button("Look at it").clicked() {
                *act = Some(GalleryAct::Review(w.clone()));
            }
        });
        if !w.about.is_empty() {
            ui.label(egui::RichText::new(&w.about).size(12.0).color(pal().text_dim));
        }
    }
    let Some(r) = &mut g.review else { return };
    ui.separator();
    ui.label(egui::RichText::new(&r.theme.name).strong().size(15.0));
    for f in &r.found {
        ui.label(f);
    }
    let working = matches!(r.look, Look::Fetching(_) | Look::Reading(_) | Look::Building(..) | Look::Playing(_));
    if working {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(match r.look {
                Look::Building(..) => "Building it again from its source\u{2026}",
                Look::Playing(_) => "Playing its story\u{2026}",
                _ => "Fetching it\u{2026}",
            });
        });
    }
    if let Look::Seen { previewed, textures } = &r.look {
        let at = previewed.pictures.iter().position(|p| p.name == "written").unwrap_or(0);
        if let Some(t) = textures.get(at) {
            let w = ui.available_width().min(480.0);
            ui.add(egui::Image::new(t).fit_to_exact_size(vec2(w, w * 0.625)));
        }
        ui.horizontal_wrapped(|ui| {
            for (k, t) in textures.iter().enumerate() {
                if k != at {
                    ui.add(egui::Image::new(t).fit_to_exact_size(vec2(112.0, 70.0)));
                }
            }
        });
    }
    if working {
        return;
    }
    let may = r.same.is_some() && matches!(r.look, Look::Seen { .. });
    ui.horizontal_wrapped(|ui| {
        if r.deciding.is_some() {
            ui.spinner();
            return;
        }
        if ui.add_enabled(may, egui::Button::new("Publish it")).on_disabled_hover_text("Only a theme built again the same, that plays, is published").clicked() {
            *act = Some(GalleryAct::Decide(true));
        }
        ui.add(egui::TextEdit::singleline(&mut r.why).char_limit(200).desired_width(ui.available_width().min(220.0)).hint_text("Why it is not taken"));
        if ui.add_enabled(!r.why.trim().is_empty(), egui::Button::new("Refuse it")).clicked() {
            *act = Some(GalleryAct::Decide(false));
        }
    });
}
