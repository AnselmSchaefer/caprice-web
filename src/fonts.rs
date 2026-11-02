use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use eframe::egui::{self, FontFamily};

/// Fonts egui knows about right now (by UI name, plus "<name>:bold", "<name>:italic" and
/// "<name>:bold:italic" for those faces).
pub static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The name a face of `font` is loaded under.
fn face_key(font: &str, bold: bool, italic: bool) -> String {
    format!("{font}{}{}", if bold { ":bold" } else { "" }, if italic { ":italic" } else { "" })
}

/// The family to draw `font` with, and whether egui must slant it itself, for want of an italic
/// face. A missing bold face is drawn regular.
pub fn family_for(font: &str, bold: bool, italic: bool) -> (FontFamily, bool) {
    let loaded = LOADED.lock().unwrap();
    if !loaded.iter().any(|f| f == font) {
        return (FontFamily::Proportional, italic);
    }
    let has = |b: bool, i: bool| loaded.contains(&face_key(font, b, i));
    let bold = bold && (has(true, italic) || has(true, false));
    let italic_face = italic && has(bold, true);
    (FontFamily::Name(face_key(font, bold, italic_face).into()), italic && !italic_face)
}

/// Common document fonts, most popular first; the first ten that are installed get offered.
const COMMON_FONTS: [&str; 24] = [
    "Times New Roman", "Arial", "Calibri", "Helvetica", "Georgia", "Verdana", "Courier New", "Cambria", "Garamond",
    "Palatino Linotype", "Liberation Serif", "Liberation Sans", "Liberation Mono", "Noto Serif", "Noto Sans",
    "DejaVu Serif", "DejaVu Sans", "Nimbus Roman", "Nimbus Sans", "P052", "Nimbus Mono PS", "Tinos", "Arimo",
    "Cousine",
];

/// Fonts made to take the same room, char for char, as each other: a missing one is drawn with an
/// installed twin, so a story written on a Mac with Times New Roman breaks into the same pages on
/// Linux with Liberation Serif. Its name stays the one written with, in the file and in Word.
const TWINS: [&[&str]; 5] = [
    &["Times New Roman", "Liberation Serif", "Tinos"],
    &["Arial", "Liberation Sans", "Arimo"],
    &["Courier New", "Liberation Mono", "Cousine"],
    &["Calibri", "Carlito"],
    &["Cambria", "Caladea"],
];

/// The installed family that draws `font`: itself, else a twin of it, else none.
pub fn drawn_with(font: &str, installed: &BTreeSet<String>) -> Option<String> {
    if installed.contains(font) {
        return Some(font.to_owned());
    }
    let twins = TWINS.iter().find(|t| t.contains(&font))?;
    twins.iter().find(|t| installed.contains(**t)).map(|t| (*t).to_owned())
}

/// The fonts that come with Caprice in the browser, which has none installed: the twins of Times
/// New Roman, Arial and Courier New, and a book face. Each is fetched from `fonts/` (`web/fonts`)
/// the first time it is wanted, its four faces as `<file>-Regular.ttf` and so on.
#[cfg(target_arch = "wasm32")]
const SHIPPED: [(&str, &str); 4] = [
    ("Liberation Serif", "LiberationSerif"),
    ("Liberation Sans", "LiberationSans"),
    ("Liberation Mono", "LiberationMono"),
    ("Noto Serif", "NotoSerif"),
];

pub struct FontBook {
    db: fontdb::Database,
    /// Every installed family (with an upright face).
    installed: BTreeSet<String>,
    /// (name shown in the UI and stored in files, installed family that renders it)
    pub families: Vec<(String, String)>,
    defs: egui::FontDefinitions,
    requested: BTreeSet<String>,
    pending: Vec<String>,
    /// In the browser: fonts wanted whose family is still on its way (stored name, and the family).
    #[cfg(target_arch = "wasm32")]
    waiting: Vec<(String, String)>,
    /// Families fetched, with the files that came (none if none could be), not yet given to `db`.
    #[cfg(target_arch = "wasm32")]
    fetched: std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<Vec<u8>>)>>>,
    #[cfg(target_arch = "wasm32")]
    fetching: BTreeSet<String>,
}

impl FontBook {
    pub fn new() -> Self {
        // The browser has no fonts installed: they come later, fetched (`SHIPPED`).
        #[cfg(not(target_arch = "wasm32"))]
        let db = {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        };
        #[cfg(target_arch = "wasm32")]
        let db = fontdb::Database::new();
        let installed: BTreeSet<String> = db
            .faces()
            .filter(|f| f.style == fontdb::Style::Normal)
            .filter_map(|f| f.families.first().map(|(n, _)| n.clone()))
            .collect();
        // In the browser the shipped fonts count as installed: they are there for the fetching.
        #[cfg(target_arch = "wasm32")]
        let installed: BTreeSet<String> = installed.into_iter().chain(SHIPPED.iter().map(|(f, _)| (*f).to_owned())).collect();
        let mut families: Vec<(String, String)> = COMMON_FONTS
            .iter()
            .filter(|f| installed.contains(**f))
            .take(10)
            .map(|f| ((*f).to_owned(), (*f).to_owned()))
            .collect();
        if families.is_empty() {
            families.push(("Default".into(), "Default".into()));
        }
        Self {
            db,
            installed,
            families,
            defs: egui::FontDefinitions::default(),
            requested: BTreeSet::new(),
            pending: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            waiting: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            fetched: Default::default(),
            #[cfg(target_arch = "wasm32")]
            fetching: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    pub fn is_installed(&self, font: &str) -> bool {
        self.installed.contains(font)
    }

    pub fn preferred_default(&self) -> String {
        self.families[0].0.clone()
    }

    /// Register `font` with egui. It becomes usable on the next frame.
    pub fn ensure(&mut self, ctx: &egui::Context, font: &str, immediate: bool) {
        if font == "Default" || !self.requested.insert(font.to_owned()) {
            return;
        }
        // A font in the document but not offered (not common, or a twin's name) is offered from now on.
        let actual = match self.families.iter().find(|(n, _)| n == font) {
            Some((_, actual)) => actual.clone(),
            None => {
                let Some(actual) = drawn_with(font, &self.installed) else { return };
                self.families.push((font.to_owned(), actual.clone()));
                actual
            }
        };
        // In the browser a family's files come when first wanted; the font is loaded once they are in.
        #[cfg(target_arch = "wasm32")]
        if !self.db.faces().any(|f| f.families.iter().any(|(n, _)| *n == actual)) {
            self.fetch(ctx, &actual);
            self.waiting.push((font.to_owned(), actual));
            return;
        }
        self.load(ctx, font, &actual, immediate);
    }

    /// Give egui the faces of the installed family `actual` under the stored name `font`.
    fn load(&mut self, ctx: &egui::Context, font: &str, actual: &str, immediate: bool) {
        let families = [fontdb::Family::Name(actual)];
        let query = |bold: bool, italic: bool| fontdb::Query {
            families: &families,
            weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
            style: if italic { fontdb::Style::Italic } else { fontdb::Style::Normal },
            ..Default::default()
        };
        let mut keys = Vec::new();
        for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
            let Some(id) = self.db.query(&query(bold, italic)) else { continue };
            let Some(info) = self.db.face(id) else { continue };
            // fontdb gives the nearest face; only a real bold or italic one will do.
            if (bold && info.weight.0 < 600) || (italic && info.style == fontdb::Style::Normal) {
                continue;
            }
            let Some((bytes, index)) = self.db.with_face_data(id, |d, i| (d.to_vec(), i)) else { continue };
            let key = face_key(font, bold, italic);
            let mut data = egui::FontData::from_owned(bytes);
            data.index = index;
            self.defs.font_data.insert(key.clone(), Arc::new(data));
            let mut chain = vec![key.clone()];
            chain.extend(self.defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
            self.defs.families.insert(FontFamily::Name(key.clone().into()), chain);
            keys.push(key);
        }
        if keys.is_empty() {
            return;
        }
        ctx.set_fonts(self.defs.clone());
        if immediate {
            LOADED.lock().unwrap().extend(keys);
        } else {
            self.pending.extend(keys);
        }
    }

    /// The font files egui draws with, by face key, and each family's chain of faces to fall back on.
    pub fn defs(&self) -> &egui::FontDefinitions {
        &self.defs
    }

    /// Whether every font asked for is usable, or will never be (neither it nor a twin installed,
    /// or its files could not be fetched). Until then pages are measured with egui's own font, and
    /// break in other places than they will.
    pub fn settled(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        if !self.waiting.is_empty() {
            return false;
        }
        self.pending.is_empty()
    }

    /// Make the fonts loaded last frame usable (egui has them from this frame on). In the browser,
    /// also load the fonts whose files have come since. Whether any font became usable: then the
    /// pages are worked out again, as they were measured without it.
    pub fn activate_pending(&mut self, ctx: &egui::Context) -> bool {
        let any = !self.pending.is_empty();
        LOADED.lock().unwrap().append(&mut self.pending);
        // Only after: what is loaded now, egui has from the next frame on, so it waits until then.
        #[cfg(target_arch = "wasm32")]
        self.take_fetched(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = ctx;
        any
    }

    /// Fetch the files of the shipped family `family`, once.
    #[cfg(target_arch = "wasm32")]
    fn fetch(&mut self, ctx: &egui::Context, family: &str) {
        let Some((_, file)) = SHIPPED.iter().find(|(f, _)| *f == family) else { return };
        if !self.fetching.insert(family.to_owned()) {
            return;
        }
        let (fetched, ctx) = (self.fetched.clone(), ctx.clone());
        let (family, file) = (family.to_owned(), (*file).to_owned());
        wasm_bindgen_futures::spawn_local(async move {
            // The faces come in together, so a family is never loaded with some of them only.
            let mut faces = Vec::new();
            for face in ["Regular", "Bold", "Italic", "BoldItalic"] {
                match crate::platform::fetch(&format!("fonts/{file}-{face}.ttf")).await {
                    Ok(bytes) => faces.push(bytes),
                    Err(e) => web_sys::console::error_1(&format!("Caprice could not fetch a font: {e}").into()),
                }
            }
            fetched.borrow_mut().push((family, faces));
            ctx.request_repaint();
        });
    }

    /// Give `db` the font files that have come, and load the fonts that waited for them.
    #[cfg(target_arch = "wasm32")]
    fn take_fetched(&mut self, ctx: &egui::Context) {
        let came = std::mem::take(&mut *self.fetched.borrow_mut());
        if came.is_empty() {
            return;
        }
        let mut families = BTreeSet::new();
        for (family, files) in came {
            for bytes in files {
                self.db.load_font_data(bytes);
            }
            families.insert(family);
        }
        // A family that could not be fetched waits no longer: it is drawn with egui's own font.
        let waiting = std::mem::take(&mut self.waiting);
        let (came, waiting): (Vec<_>, Vec<_>) = waiting.into_iter().partition(|(_, a)| families.contains(a));
        self.waiting = waiting;
        for (font, actual) in came {
            self.load(ctx, &font, &actual, false);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_font_is_drawn_with_an_installed_twin_of_the_same_widths() {
        let installed: BTreeSet<String> = ["Liberation Serif", "Arimo", "Georgia"].map(String::from).into();
        assert_eq!(drawn_with("Times New Roman", &installed).as_deref(), Some("Liberation Serif"));
        assert_eq!(drawn_with("Liberation Sans", &installed).as_deref(), Some("Arimo"));
        assert_eq!(drawn_with("Georgia", &installed).as_deref(), Some("Georgia"), "installed, it is itself");
        assert_eq!(drawn_with("Courier New", &installed), None, "no twin installed");
        assert_eq!(drawn_with("Verdana", &installed), None, "no twin at all");
    }
}
