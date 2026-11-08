//! Tells the tests which of the git-ignored city extracts this checkout has: a test that
//! builds its world from one is ignored where the extract is absent (a fresh checkout, CI),
//! not failed. The engine's build script does the same for the engine's tests.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../v2xw-engine/cached_maps.rs");
    emit_cached_maps();
}

include!("../v2xw-engine/cached_maps.rs");
