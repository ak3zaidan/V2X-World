//! `v2xw` — the command-line control surface (ADR 0010).
//!
//! The binary is thin on purpose: it parses arguments, calls a library function, prints
//! the outcome and picks an exit code. Everything it can do is available to a test as a
//! plain function call, which is why `tests/` never shells out to the binary.
//!
//! Exit codes: `0` success, `1` the command failed, `2` the arguments were wrong (clap's
//! own code). A failure prints to stderr with the field or path it is about, never a bare
//! "error".

use std::process::ExitCode;

use clap::Parser;
use v2xw_cli::cli::{Cli, Command, ExperimentCommand};
use v2xw_cli::{agent, energy, experiment, fmt, import, info, run, validate};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(&cli) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("v2xw: {e}");
            // A scenario error knows which key it is about; saying so is the difference
            // between an error an author can act on and one they have to bisect
            // (03-interfaces.md §13).
            if let v2xw_cli::CliError::Engine(v2xw_engine::EngineError::Scenario(s)) = &e
                && let Some(field) = s.field()
            {
                eprintln!("v2xw: the offending key is `{field}`");
            }
            let mut source = std::error::Error::source(&e);
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn dispatch(cli: &Cli) -> v2xw_cli::Result<String> {
    match &cli.command {
        Command::Agent(args) => agent::agent(&args.to_options()),
        Command::Serve(args) => agent::serve(&args.to_options()),
        Command::Cities(args) => Ok(cities(args.filter.as_deref(), args.json)),
        Command::Run(args) => {
            let outcome = run::run(&args.to_options())?;
            if args.json {
                json(&outcome, "run outcome")
            } else {
                Ok(fmt::run(&outcome))
            }
        }
        Command::Resume(args) => {
            let outcome = run::resume(&args.checkpoint, args.quiet, args.json)?;
            if args.json {
                json(&outcome, "run outcome")
            } else {
                Ok(fmt::run(&outcome))
            }
        }
        Command::Validate(args) => {
            let outcome = validate::validate(&args.scenario)?;
            if args.json {
                json(&outcome, "validation outcome")
            } else {
                Ok(fmt::validate(&outcome))
            }
        }
        Command::ImportOsm(args) => {
            let (outcome, report) = import::import_osm_extract(&import::ImportOptions {
                extract: args.extract.clone(),
                out: args.out.clone(),
                imported_at: args.imported_at.clone(),
                speed_preset: args.speed_preset.clone(),
                bbox: args.bbox.clone(),
            })?;
            if args.json {
                json(&outcome, "import outcome")
            } else {
                Ok(fmt::import(&outcome, &report))
            }
        }
        Command::Info(args) => {
            let outcome = info::info(&args.recording)?;
            if args.json {
                json(&outcome, "recording info")
            } else {
                Ok(fmt::info(&outcome))
            }
        }
        Command::Energy(args) => {
            let outcome = energy::energy(&args.to_options())?;
            if args.json {
                json(&outcome, "energy outcome")
            } else {
                Ok(energy::format(&outcome))
            }
        }
        Command::Experiment(args) => match &args.command {
            // `run` and `resume` are one code path with one flag between them, so there is
            // no second implementation to drift.
            ExperimentCommand::Run(a) | ExperimentCommand::Resume(a) => {
                let resume = matches!(&args.command, ExperimentCommand::Resume(_));
                let outcome = experiment::run(&a.to_options(resume))?;
                if a.json {
                    json(&outcome, "experiment outcome")
                } else {
                    Ok(fmt::experiment_run(&outcome))
                }
            }
            ExperimentCommand::Status(a) => {
                let outcome = experiment::status(&a.scenario, a.out.as_deref())?;
                if a.json {
                    json(&outcome, "experiment status")
                } else {
                    Ok(fmt::experiment_status(&outcome))
                }
            }
        },
    }
}

/// `v2xw cities`: the library, grouped by state, or as JSON.
fn cities(filter: Option<&str>, as_json: bool) -> String {
    let wanted = filter.map(str::to_lowercase);
    let matches = |c: &v2xw_engine::city::City| {
        wanted.as_deref().is_none_or(|w| {
            [&c.id, &c.name, &c.state, &c.state_name]
                .iter()
                .any(|f| f.to_lowercase().contains(w))
        })
    };
    let registry = v2xw_engine::city::registry();
    if as_json {
        let mut v = v2xw_engine::city::list_json();
        if let Some(list) = v.get_mut("cities").and_then(serde_json::Value::as_array_mut) {
            list.retain(|c| {
                c.get("id")
                    .and_then(serde_json::Value::as_str)
                    .and_then(v2xw_engine::city::find)
                    .is_some_and(matches)
            });
        }
        return serde_json::to_string_pretty(&v).unwrap_or_default() + "\n";
    }
    let mut out = String::new();
    let mut state = String::new();
    let mut missing = 0;
    for c in registry.cities.iter().filter(|c| matches(c)) {
        if c.state_name != state {
            state.clone_from(&c.state_name);
            out.push_str(&format!("{state}\n"));
        }
        let present = c.extract_present();
        if !present {
            missing += 1;
        }
        out.push_str(&format!(
            "  {:<16} {:<30} {:<17} {}{}\n",
            c.id,
            c.name,
            c.highway_preset,
            c.description,
            if present { "" } else { "  [not fetched]" }
        ));
    }
    for d in &registry.dropped {
        out.push_str(&format!("dropped: {} — {}\n", d.id, d.reason));
    }
    if missing > 0 {
        out.push_str(&format!(
            "\n{missing} extract(s) not fetched yet: worlds/fetch.sh <id> downloads one, \
             worlds/fetch.sh --all every missing one.\n"
        ));
    }
    out.push_str(&format!(
        "\nRun any scenario on a city with `v2xw run <scenario> --city <id>`.\nMap data {}\n",
        registry.attribution
    ));
    out
}

fn json<T: serde::Serialize>(value: &T, what: &'static str) -> v2xw_cli::Result<String> {
    let mut s = serde_json::to_string_pretty(value)
        .map_err(|source| v2xw_cli::CliError::Json { what, source })?;
    s.push('\n');
    Ok(s)
}
