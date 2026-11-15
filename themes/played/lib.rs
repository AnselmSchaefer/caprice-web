//! The theme programs of `themes/`, built for the machine: Caprice's tests play them in place of
//! their modules (`tests/fixtures/*.wasm`), which are these same files built by `themes/build.sh`.

#[path = "../echo/theme.rs"]
pub mod echo;
#[path = "../fireflies/theme.rs"]
pub mod fireflies;
#[path = "../snow/theme.rs"]
pub mod snow;
#[path = "../vineyard/theme.rs"]
pub mod vineyard;
