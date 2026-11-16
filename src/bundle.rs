//! A theme that is a program, as one file to give and take (R36, the plan's step 3): a
//! `.caprice-theme`, a zip of `manifest.json` (what the shelf shows of it: its name, a few words,
//! who made it, its palette; the world it is built for and the toolchain that built it),
//! `theme.wasm`, `theme.rs` if its source comes with it, and `preview.png`, its picture for the
//! gallery. Imported as a drawing is (`own::Workshop`), checked here and by `eggs::program::check`,
//! with the reasons in words when it is refused; written again by "Export…", so a theme on the
//! shelf can be given to someone as it came. A file with a source and no module, or a source alone
//! (`from_source`), is built in the browser (`platform::compiler`) before it goes on the shelf.
//!
//! Unsigned modules are welcome on one's own shelf: the sandbox is what keeps them harmless. Only
//! the gallery's are signed (`own::Signed`), and a file never carries a signature.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use eframe::egui::ColorImage;
use serde::{Deserialize, Serialize};

use crate::own::{NAMES, with_colours};
use crate::theme::Theme;

/// What a theme file is called after.
pub const EXTENSION: &str = "caprice-theme";
/// The world the themes Caprice plays are built for.
pub const WORLD: &str = "caprice:theme@1.0.0";
/// The largest file taken, and the most of its source and its picture: a module is at most
/// `eggs::program::MOST_BYTES`.
pub const MOST_FILE: usize = 4 << 20;
const MOST_SOURCE: usize = crate::platform::compiler::MOST_SOURCE;
const MOST_PREVIEW: usize = 2 << 20;
/// How wide the gallery's picture of it is kept, in pixels, as a drawing's is.
const PREVIEW_WIDTH: u32 = 320;

/// What `manifest.json` says. Everything but the world may be left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    /// The world the module is built for: `caprice:theme@1.0.0`.
    pub world: String,
    pub name: String,
    pub about: String,
    pub author: String,
    /// The theme's palette, by the palette's names (`own::NAMES`, and `"light"`): the toolbar,
    /// buttons, page bar and text are coloured from it, and the desk under the program's shapes
    /// is its two colours, which show before its first frame and if it is stopped.
    pub colours: BTreeMap<String, String>,
    /// The toolchain that built the module, as the writer's tab names it (the plan's step 4).
    pub toolchain: String,
}

/// A theme file's parts.
#[derive(Clone, Debug, Default)]
pub struct Bundle {
    pub manifest: Manifest,
    pub module: Vec<u8>,
    pub source: Option<String>,
    pub preview: Option<ColorImage>,
}

/// A theme file read and checked: its parts, what was found in it and what is worth knowing. Its
/// module is empty if it is still to be built from its source.
pub struct Opened {
    pub bundle: Bundle,
    pub found: Vec<String>,
    pub notes: Vec<String>,
}

/// Whether `bytes` look like a theme file rather than a drawing: a zip.
pub fn is_bundle(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// One file of the zip, if it is there, read to at most `most` bytes: a file that says it is small
/// and unpacks to more is read no further.
fn part(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, most: usize) -> Result<Option<Vec<u8>>, String> {
    let file = match zip.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(format!("Its {name} could not be read: {e}.")),
    };
    let mut bytes = Vec::new();
    file.take(most as u64 + 1).read_to_end(&mut bytes).map_err(|e| format!("Its {name} could not be read: {e}."))?;
    if bytes.len() > most {
        return Err(format!("Its {name} is more than the {} KB it may be.", most >> 10));
    }
    Ok(Some(bytes))
}

/// A theme file read, every part of it checked, its module by `eggs::program::check`; the
/// reasons it cannot be taken, in words, if any. `name` is the file's, for a theme that names
/// itself nothing.
pub fn open(bytes: &[u8], name: &str) -> Result<Opened, Vec<String>> {
    if bytes.len() > MOST_FILE {
        return Err(vec![format!("It is {} KB, more than the {} KB a theme file may be.", bytes.len() >> 10, MOST_FILE >> 10)]);
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| vec!["It is not a Caprice theme file (one is a zip, made by Caprice or a theme's maker).".to_string()])?;
    let (mut found, mut notes, mut problems) = (Vec::new(), Vec::new(), Vec::new());
    let mut manifest = match part(&mut zip, "manifest.json", 64 << 10) {
        Ok(Some(m)) => match serde_json::from_slice::<Manifest>(&m) {
            Ok(m) => m,
            Err(e) => return Err(vec![format!("Its manifest.json could not be read: {e}.")]),
        },
        Ok(None) => return Err(vec!["It has no manifest.json, which says what the theme is.".into()]),
        Err(why) => return Err(vec![why]),
    };
    if manifest.world != WORLD {
        let made = if manifest.world.is_empty() { "for no world its manifest names".to_string() } else { format!("for {}", manifest.world) };
        problems.push(format!("It was made {made}; this Caprice plays themes made for {WORLD}."));
    }
    let source = match part(&mut zip, "theme.rs", MOST_SOURCE) {
        Ok(Some(s)) => match String::from_utf8(s) {
            Ok(s) => Some(s),
            Err(_) => {
                notes.push("Its theme.rs is not text, and is left out.".into());
                None
            }
        },
        Ok(None) => None,
        Err(why) => {
            notes.push(format!("{why} It is left out."));
            None
        }
    };
    let module = match part(&mut zip, "theme.wasm", crate::eggs::program::MOST_BYTES) {
        Ok(Some(m)) => m,
        Ok(None) if source.is_some() => Vec::new(),
        Ok(None) => {
            problems.push("It has no theme.wasm, the theme's program, nor a theme.rs to build it from.".into());
            Vec::new()
        }
        Err(why) => {
            problems.push(why);
            Vec::new()
        }
    };
    if !module.is_empty() {
        match crate::eggs::program::check(&module) {
            Ok(()) => found.push(format!("A program for {WORLD}, {} KB.", module.len().div_ceil(1024))),
            Err(why) => problems.extend(why.into_iter().map(|w| format!("Its program cannot be played: {w}."))),
        }
    }
    match (&source, module.is_empty()) {
        (Some(s), false) => found.push(format!("Its source, {} lines.", s.lines().count())),
        (Some(s), true) => found.push(format!("Its source, {} lines, to build its program from.", s.lines().count())),
        (None, _) => {}
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    if manifest.name.trim().is_empty() {
        manifest.name = name.to_owned();
    }
    let (_, unread) = with_colours(Theme::Midnight.palette(), &manifest.colours);
    // The colours read, by the palette's names: "light" says how its widgets are drawn, and is none.
    let read = manifest.colours.keys().filter(|k| NAMES.contains(&k.as_str()) && !unread.contains(k)).count();
    match read {
        0 => notes.push("It has no palette of its own: the toolbar and the desk under it take Midnight's colours.".into()),
        n if n == NAMES.len() => found.push(format!("Its palette, all {n} colours.")),
        n => found.push(format!("Its palette: {n} of the {} colours, the rest Midnight's.", NAMES.len())),
    }
    if !unread.is_empty() {
        notes.push(format!("These colours could not be read, and are Midnight's: {}.", unread.join(", ")));
        manifest.colours.retain(|k, _| !unread.contains(k));
    }
    let preview = match part(&mut zip, "preview.png", MOST_PREVIEW) {
        Ok(Some(png)) => match image::load_from_memory_with_format(&png, image::ImageFormat::Png) {
            Ok(img) => {
                found.push("A picture for the gallery.".into());
                let img = img.resize(PREVIEW_WIDTH, PREVIEW_WIDTH, image::imageops::FilterType::Triangle).to_rgba8();
                Some(ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw()))
            }
            Err(_) => {
                notes.push("Its preview.png could not be read: the gallery shows its colours instead.".into());
                None
            }
        },
        Ok(None) => {
            notes.push("It has no picture for the gallery (preview.png): the gallery shows its colours instead.".into());
            None
        }
        Err(why) => {
            notes.push(format!("{why} The gallery shows its colours instead."));
            None
        }
    };
    let known = ["manifest.json", "theme.wasm", "theme.rs", "preview.png"];
    let others: Vec<String> = zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).filter(|n| !known.contains(&n.as_str()) && !n.ends_with('/')).collect();
    if !others.is_empty() {
        notes.push(format!("Left out, as a theme file holds nothing else: {}.", others.join(", ")));
    }
    if !cfg!(target_arch = "wasm32") {
        notes.push("Themes that are programs play in Caprice in the browser only: here the usual desk shows.".into());
    }
    Ok(Opened { bundle: Bundle { manifest, module, source, preview }, found, notes })
}

/// A theme program's source alone, as a theme to build (`platform::compiler`): named after its
/// file, with no palette or picture of its own.
pub fn from_source(bytes: &[u8], name: &str) -> Result<Opened, Vec<String>> {
    if bytes.len() > MOST_SOURCE {
        return Err(vec![format!("It is {} KB, more than the {} KB a theme's source may be.", bytes.len() >> 10, MOST_SOURCE >> 10)]);
    }
    let source = String::from_utf8(bytes.to_vec()).map_err(|_| vec!["It is not text, as a theme's source (a .rs file) is.".to_string()])?;
    let manifest = Manifest { world: WORLD.into(), name: name.to_owned(), ..Manifest::default() };
    let found = vec![format!("Its source, {} lines, to build its program from.", source.lines().count())];
    let notes = vec![
        "It has no palette of its own: the toolbar and the desk under it take Midnight's colours.".into(),
        "It has no picture for the gallery: the gallery shows its colours instead.".into(),
    ];
    Ok(Opened { bundle: Bundle { manifest, module: Vec::new(), source: Some(source), preview: None }, found, notes })
}

/// A theme file written: its manifest, module, source and picture, the same bytes for the same
/// parts (no time is written into it).
pub fn write(b: &Bundle) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(zip::DateTime::default());
    let mut put = |name: &str, bytes: &[u8]| -> Result<(), String> {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())
    };
    put("manifest.json", (serde_json::to_string_pretty(&b.manifest).map_err(|e| e.to_string())? + "\n").as_bytes())?;
    put("theme.wasm", &b.module)?;
    if let Some(source) = &b.source {
        put("theme.rs", source.as_bytes())?;
    }
    if let Some(p) = &b.preview {
        let rgba: Vec<u8> = p.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
        let img = image::RgbaImage::from_raw(p.size[0] as u32, p.size[1] as u32, rgba).ok_or("its picture could not be written")?;
        let mut png = Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).map_err(|e| e.to_string())?;
        put("preview.png", &png.into_inner())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}


#[cfg(test)]
// Tests run only on the desktop, where these are what `platform` uses anyway.
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    const SNOW: &[u8] = include_bytes!("../tests/fixtures/snow.wasm");

    /// A theme file of these parts, written as a zip, a part `None` left out.
    fn zipped(parts: &[(&str, Option<&[u8]>)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in parts {
            if let Some(bytes) = bytes {
                zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        zip.finish().unwrap().into_inner()
    }

    fn manifest(json: &str) -> Vec<u8> {
        json.as_bytes().to_vec()
    }

    #[test]
    fn the_snow_s_and_the_vineyard_s_theme_files_are_their_manifests_modules_and_sources() {
        // `tests/fixtures/<theme>.caprice-theme`, to try by hand: CAPRICE_FIXTURES=1 makes them again.
        for (name, theme) in [("snow", Theme::Snow), ("vineyard", Theme::Vineyard)] {
            let dir = root().join("themes").join(name);
            let manifest: Manifest = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
            assert_eq!(manifest.colours, crate::own::colours(&theme.palette()), "{name}: the built-in theme's palette, so its toolbar and desk agree");
            let module = std::fs::read(root().join(format!("tests/fixtures/{name}.wasm"))).unwrap();
            let source = std::fs::read_to_string(dir.join("theme.rs")).unwrap();
            let made = write(&Bundle { manifest: manifest.clone(), module: module.clone(), source: Some(source.clone()), preview: None }).unwrap();
            let file = root().join(format!("tests/fixtures/{name}.{EXTENSION}"));
            if std::env::var_os("CAPRICE_FIXTURES").is_some() {
                std::fs::write(&file, &made).unwrap();
            }
            assert!(std::fs::read(&file).unwrap() == made, "{} is not made of themes/{name} and its module: CAPRICE_FIXTURES=1 makes it again", file.display());
            let opened = open(&made, "file").unwrap();
            assert!(opened.bundle.manifest == manifest && opened.bundle.module == module && opened.bundle.source == Some(source), "{name}: read as it was written");
            assert!(opened.found.iter().any(|f| f.contains("A program")) && opened.found.iter().any(|f| f.contains("source")), "{:?}", opened.found);
            assert!(opened.found.contains(&"Its palette, all 14 colours.".to_string()), "{:?}", opened.found);
        }
    }

    #[test]
    fn a_file_that_is_not_a_theme_or_whose_program_cannot_be_played_is_refused_saying_why() {
        let world = br#"{"world": "caprice:theme@1.0.0"}"#;
        let refused = |bytes: &[u8]| open(bytes, "file").err().expect("refused").join(" ");
        assert!(refused(b"not a zip at all").contains("not a Caprice theme file"));
        assert!(refused(&zipped(&[("theme.wasm", Some(SNOW))])).contains("no manifest.json"));
        assert!(refused(&zipped(&[("manifest.json", Some(b"{ not json"))])).contains("manifest.json could not be read"));
        assert!(refused(&zipped(&[("manifest.json", Some(world))])).contains("no theme.wasm"));
        let later = manifest(r#"{"world": "caprice:theme@2.0.0"}"#);
        assert!(refused(&zipped(&[("manifest.json", Some(&later)), ("theme.wasm", Some(SNOW))])).contains("made for caprice:theme@2.0.0"));
        let unnamed = manifest("{}");
        assert!(refused(&zipped(&[("manifest.json", Some(&unnamed)), ("theme.wasm", Some(SNOW))])).contains("for no world its manifest names"));
        // A module that asks for more than Caprice's host: refused as the check says.
        let asks = wat::parse_str(r#"(module (import "env" "fetch" (func)) (memory (export "memory") 1 512))"#).unwrap();
        assert!(refused(&zipped(&[("manifest.json", Some(world)), ("theme.wasm", Some(&asks))])).contains("Its program cannot be played"));
        // One that says it is small and unpacks to more than a module may be.
        let big = vec![0u8; crate::eggs::program::MOST_BYTES + 1];
        assert!(refused(&zipped(&[("manifest.json", Some(world)), ("theme.wasm", Some(&big))])).contains("more than the 1024 KB"));
    }

    #[test]
    fn a_theme_file_or_a_source_with_no_module_is_taken_to_be_built_from_its_source() {
        let world = br#"{"world": "caprice:theme@1.0.0", "name": "Tide"}"#;
        let o = open(&zipped(&[("manifest.json", Some(world)), ("theme.rs", Some(b"// the tide\n"))]), "file").unwrap_or_else(|e| panic!("{e:?}"));
        assert!(o.bundle.module.is_empty() && o.bundle.source.as_deref() == Some("// the tide\n") && o.bundle.manifest.name == "Tide");
        assert!(o.found.contains(&"Its source, 1 lines, to build its program from.".to_string()), "{:?}", o.found);
        let o = from_source(b"// the tide\n// at dusk\n", "Tide").unwrap_or_else(|e| panic!("{e:?}"));
        assert!(o.bundle.module.is_empty() && o.bundle.manifest.world == WORLD && o.bundle.manifest.name == "Tide");
        assert!(o.notes.iter().any(|n| n.contains("no palette")), "{:?}", o.notes);
        let refused = |bytes: &[u8]| from_source(bytes, "x").err().expect("refused").join(" ");
        assert!(refused(&[0xff, 0xfe]).contains("not text"));
        assert!(refused(&vec![b' '; MOST_SOURCE + 1]).contains("more than the 512 KB"));
    }

    #[test]
    fn what_a_theme_file_lacks_or_holds_besides_is_worth_knowing_not_a_refusal() {
        let m = manifest(r##"{"world": "caprice:theme@1.0.0", "colours": {"desk": "#123456", "desk_top": "blue", "sparkle": "#fff"}}"##);
        let o = open(&zipped(&[("manifest.json", Some(&m)), ("theme.wasm", Some(SNOW)), ("theme.rs", Some(&[0xff, 0xfe])), ("notes.txt", Some(b"hi"))]), "Night walk").unwrap();
        assert_eq!(o.bundle.manifest.name, "Night walk", "named after its file, as it names itself nothing");
        assert_eq!(o.bundle.manifest.colours.keys().collect::<Vec<_>>(), ["desk"], "only the colours read are kept");
        assert!(o.found.contains(&"Its palette: 1 of the 14 colours, the rest Midnight's.".to_string()), "{:?}", o.found);
        let notes = o.notes.join(" ");
        for said in ["could not be read, and are Midnight's: desk_top, sparkle", "theme.rs is not text", "no picture for the gallery", "Left out", "notes.txt"] {
            assert!(notes.contains(said), "{said}: {notes}");
        }
        assert!(o.bundle.source.is_none());
        // A picture for the gallery, made as wide as the shelf keeps them.
        let mut png = Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(640, 360, image::Rgba([10, 20, 30, 255])).write_to(&mut png, image::ImageFormat::Png).unwrap();
        let o = open(&zipped(&[("manifest.json", Some(&m)), ("theme.wasm", Some(SNOW)), ("preview.png", Some(png.get_ref()))]), "x").unwrap();
        assert_eq!(o.bundle.preview.map(|p| p.size), Some([320, 180]));
    }
}
