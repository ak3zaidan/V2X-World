//! `dump_settings` — prints the engine's published settings surface as JSON.
//!
//! ```text
//! cargo run -p v2xw-engine --example dump_settings > settings.json
//! cargo run -p v2xw-engine --example dump_settings -- --out settings.json
//! ```
//!
//! The output is [`v2xw_engine::scenario::publish::surface`], the same bundle the server
//! returns from the `scenario.schema` and `scenario.get` RPCs and that the Studio's
//! settings window is generated from: the JSON Schema, one row per editable leaf
//! (`fields`, with path, type, unit, default, bounds or choices, status and description),
//! the sections (`groups`), the model slots and every registered model card.
//!
//! The documentation site's configuration reference is generated from this output by
//! `tools/docs/gen_config_reference.py`, so the reference cannot drift from the schema.

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let surface = v2xw_engine::scenario::publish::surface();
    let text = serde_json::to_string_pretty(&surface)?;
    match args.as_slice() {
        [] => {
            let mut out = std::io::stdout().lock();
            out.write_all(text.as_bytes())?;
            out.write_all(b"\n")?;
        }
        [flag, path] if flag == "--out" => std::fs::write(path, text + "\n")?,
        _ => return Err("usage: dump_settings [--out <file.json>]".into()),
    }
    Ok(())
}
