// Included by the build scripts of the crates whose tests run on an imported city. Not a module
// of any crate: `include!`d into `build.rs`, so it is plain functions.
//
// The city extracts under `worlds/cache/` are OpenStreetMap downloads: ODbL, tens of MB, and
// git-ignored, so a fresh checkout (CI's) has none of them. A test that builds its world from
// one cannot run there, and before this it failed there instead, naming the missing file.
// Each such test now carries
//
//     #[cfg_attr(not(v2xw_map = "manhattan"), ignore = "needs worlds/cache/manhattan.osm.xml (gitignored)")]
//
// so it runs wherever the extract is present and is reported as ignored, not passed, where it
// is not. A test that skipped itself with an early `return` would read as a pass on CI.
//
// Rebuilds: cargo reruns a build script on every build when a `rerun-if-changed` path does not
// exist, so a missing extract is not watched by name. The present ones are, and while any is
// missing the directory is, so fetching or linking an extract brings its tests back on the
// next build.

/// The extracts a test may need, by their `worlds/cache/<id>.osm.xml` id.
const CACHED_MAPS: [&str; 3] = ["manhattan", "portland", "berlin"];

/// Emits `--cfg v2xw_map="<id>"` for each extract this checkout has.
fn emit_cached_maps() {
    let values: Vec<String> = CACHED_MAPS.iter().map(|id| format!("\"{id}\"")).collect();
    println!("cargo:rustc-check-cfg=cfg(v2xw_map, values({}))", values.join(", "));
    let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../worlds/cache");
    let mut missing = false;
    for id in CACHED_MAPS {
        let path = cache.join(format!("{id}.osm.xml"));
        if path.is_file() {
            println!("cargo:rerun-if-changed={}", path.display());
            println!("cargo:rustc-cfg=v2xw_map=\"{id}\"");
        } else {
            missing = true;
        }
    }
    if missing && cache.is_dir() {
        println!("cargo:rerun-if-changed={}", cache.display());
    }
}
