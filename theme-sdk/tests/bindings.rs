//! The bindings the SDK carries are what wit-bindgen makes of `wit/theme.wit` with the options a
//! theme's build needs (the SDK's own runtime, the export macro public): change the WIT, then
//! make them again with `wit-bindgen rust wit --world theme --out-dir src --runtime-path crate::rt
//! --pub-export-macro --format` and rename `theme.rs` to `bindings.rs`.
#![allow(clippy::disallowed_methods)]

use wit_bindgen_core::wit_parser::Resolve;
use wit_bindgen_core::{Files, WorldGenerator};

#[test]
fn the_bindings_carried_are_made_again_the_same_from_the_wit() {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut resolve = Resolve::default();
    let (package, _) = resolve.push_dir(here.join("wit")).unwrap();
    let world = resolve.select_world(&[package], Some("theme")).unwrap();
    let opts = wit_bindgen_rust::Opts { format: true, runtime_path: Some("crate::rt".into()), pub_export_macro: true, ..Default::default() };
    let mut files = Files::default();
    opts.build().generate(&mut resolve, world, &mut files).unwrap();
    let (_, made) = files.iter().find(|(name, _)| *name == "theme.rs").expect("theme.rs made");
    let carried = std::fs::read(here.join("src/bindings.rs")).unwrap();
    assert!(made == carried.as_slice(), "src/bindings.rs is not what wit/theme.wit makes: make it again (see this file)");
}
