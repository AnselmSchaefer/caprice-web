//! Stories kept in the browser. A page cannot save to the writer's computer, so in the browser a
//! story is saved among the browser's own files (`platform::fs`), in one folder: the shelf. Saving
//! asks for a name rather than a place, and opening lists the stories on the shelf, with a way to
//! bring one in from the computer. Each answers the waiting dialog with a path, as the system's
//! dialogs do on the desktop, so saving, autosave (R22) and the stories last opened work the same.
//!
//! The shelf is there when `dir` is: always in the browser, never on the desktop, and in tests
//! that say so.

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use eframe::egui::{self, Id, Key, pos2};

use crate::App;

pub struct Shelf {
    /// The folder the stories are kept in (None: there is no shelf, and the system's dialogs ask).
    pub dir: Option<PathBuf>,
    asking: Option<Asking>,
}

/// What the shelf is asking the writer, and where the answer goes.
enum Asking {
    /// A name to save the story under.
    Name { name: String, tx: Sender<Option<PathBuf>>, focused: bool },
    /// Which story to open; and whether to delete one, when its delete button was pressed.
    Open { stories: Vec<Kept>, tx: Sender<Option<PathBuf>>, deleting: Option<PathBuf> },
}

/// A story on the shelf.
struct Kept {
    name: String,
    path: PathBuf,
    changed: Option<u64>,
    /// Whether it is online too, while signed in (`cloud.rs`).
    online: Option<&'static str>,
}

impl Shelf {
    pub fn new() -> Self {
        let dir = if cfg!(target_arch = "wasm32") { crate::platform::data_dir().map(|d| d.join("stories")) } else { None };
        Shelf { dir, asking: None }
    }

    /// Ask for a name to save the story under, suggesting `name` (a file name). The path chosen,
    /// or None, goes to `tx`.
    pub fn ask_name(&mut self, name: Option<String>, tx: Sender<Option<PathBuf>>) {
        let name = name.as_deref().map_or("Untitled", |n| n.strip_suffix(".caprice").unwrap_or(n)).to_owned();
        self.asking = Some(Asking::Name { name, tx, focused: false });
    }

    /// Ask which story to open, each with what `mark` says of it. The path chosen, or None, goes to `tx`.
    pub fn ask_open(&mut self, tx: Sender<Option<PathBuf>>, mark: &dyn Fn(&std::path::Path) -> Option<&'static str>) {
        let Some(dir) = &self.dir else { return };
        let mut stories: Vec<Kept> = crate::platform::fs::read_dir(dir)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "caprice"))
            .map(|path| Kept {
                name: path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned()),
                changed: crate::platform::fs::modified(&path),
                online: mark(&path),
                path,
            })
            .collect();
        stories.sort_by_key(|k| std::cmp::Reverse(k.changed));
        self.asking = Some(Asking::Open { stories, tx, deleting: None });
    }
}

/// A name the writer typed, made fit for a file: no slashes, no space around it.
pub(crate) fn file_name(name: &str) -> String {
    let name: String = name.trim().chars().map(|c| if matches!(c, '/' | '\\') { '-' } else { c }).collect();
    if name.is_empty() { "Untitled".into() } else { name }
}

impl App {
    /// Delete a story on the shelf: from this browser, from the stories last opened, and online,
    /// where the writer's other devices learn of it and delete theirs (R33). The open story is
    /// never deleted. It stays in the Versions window, to bring back.
    pub fn delete_story(&mut self, path: &std::path::Path) {
        if self.path.as_deref() == Some(path) {
            return;
        }
        self.cloud.deleted(path);
        self.forget_story(path);
        self.status = "- deleted".into();
    }

    /// The story at `path` goes from the shelf and the list, kept among the versions first.
    pub(crate) fn forget_story(&mut self, path: &std::path::Path) {
        if let Ok(json) = crate::platform::fs::read_to_string(path) {
            let name = path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned());
            self.keep_deleted(&json, name);
        }
        let _ = crate::platform::fs::remove_file(path);
        self.start.forget(path);
    }

    /// The shelf's question, while it asks one.
    pub fn shelf_window(&mut self, ctx: &egui::Context) {
        let (Some(dir), Some(asking)) = (self.shelf.dir.clone(), self.shelf.asking.as_mut()) else { return };
        let screen = ctx.content_rect();
        let mut open = true;
        let mut answer: Option<Option<PathBuf>> = None;
        let mut upload = false;
        let mut delete = None;
        let shown = self.path.clone();
        let title = if matches!(asking, Asking::Name { .. }) { "Save in this browser" } else { "Open a story" };
        egui::Window::new(title)
            .id(Id::new("shelf"))
            .open(&mut open)
            .default_pos(pos2(screen.center().x - 160.0, screen.top() + 90.0))
            .default_width(crate::ui::window_width(ctx, 320.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| match asking {
                Asking::Name { name, focused, .. } => {
                    ui.label("Stories are kept in this browser, under a name of their own:");
                    ui.add_space(4.0);
                    let edit = egui::TextEdit::singleline(name).hint_text("Name of the story").desired_width(f32::INFINITY);
                    let field = ui.add(edit);
                    if !*focused {
                        field.request_focus();
                        *focused = true;
                    }
                    // Another story of that name is never replaced: this one goes beside it, under a
                    // name of its own, so that no words are lost here or online (where the other's
                    // copy would be replaced too). Only the story's own file is saved over.
                    let mut path = dir.join(format!("{}.caprice", file_name(name)));
                    if crate::platform::fs::exists(&path) && shown.as_deref() != Some(path.as_path()) {
                        path = crate::platform::free_name(&dir, &format!("{}.caprice", file_name(name)));
                        let beside = path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned());
                        ui.weak(format!("Another story is kept under that name: this one is saved beside it as \u{201c}{beside}\u{201d}."));
                    }
                    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() || entered {
                            let _ = crate::platform::fs::create_dir_all(&dir);
                            answer = Some(Some(path));
                        }
                        if ui.button("Cancel").clicked() {
                            answer = Some(None);
                        }
                    });
                }
                Asking::Open { stories, deleting, .. } => {
                    if stories.is_empty() {
                        ui.weak("No story is kept in this browser yet.");
                    }
                    let now = crate::platform::since_epoch().as_secs();
                    egui::ScrollArea::vertical().id_salt("shelf_stories").max_height(320.0).show(ui, |ui| {
                        for k in stories.iter() {
                            if deleting.as_ref() == Some(&k.path) {
                                ui.label(format!("Delete \u{201c}{}\u{201d} here, and online on every device? It stays under File \u{203a} Versions.", k.name));
                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new("Delete").color(crate::leave::WARN)).clicked() {
                                        delete = Some(k.path.clone());
                                    }
                                    if ui.button("Keep").clicked() {
                                        *deleting = None;
                                    }
                                });
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.button(&k.name).on_hover_text("Open this story").clicked() {
                                    answer = Some(Some(k.path.clone()));
                                }
                                if let Some(at) = k.changed {
                                    ui.weak(crate::versions::ago(at, now));
                                }
                                if let Some(online) = k.online {
                                    ui.weak(format!("\u{00b7} {online}"));
                                }
                                // Not the story open: it is being written in.
                                let gone = egui::Button::new(egui::RichText::new("Delete\u{2026}").weak()).frame(false);
                                if shown.as_ref() != Some(&k.path) && ui.add(gone).on_hover_text("Delete this story here and online").clicked() {
                                    *deleting = Some(k.path.clone());
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                    let tip = "A .caprice file from your computer: it is kept in this browser from then on";
                    if cfg!(target_arch = "wasm32") && ui.button("Upload a .caprice file\u{2026}").on_hover_text(tip).clicked() {
                        upload = true;
                    }
                }
            });
        if !open || ctx.input(|i| i.key_pressed(Key::Escape)) {
            answer = answer.or(Some(None));
        }
        if let Some(path) = delete {
            self.delete_story(&path);
            if let Some(Asking::Open { stories, deleting, .. }) = &mut self.shelf.asking {
                stories.retain(|k| k.path != path);
                *deleting = None;
            }
        }
        if upload {
            // The picker answers the same waiting dialog, once the file is in.
            #[cfg(target_arch = "wasm32")]
            if let Some(Asking::Open { tx, .. }) = self.shelf.asking.take() {
                let picker = crate::platform::Dialog::new().add_filter("Caprice document", &["caprice"]);
                crate::platform::upload(&picker, dir, tx, crate::platform::Waker::new(ctx));
            }
            return;
        }
        if let Some(answer) = answer
            && let Some(Asking::Name { tx, .. } | Asking::Open { tx, .. }) = self.shelf.asking.take()
        {
            let _ = tx.send(answer);
        }
    }
}
