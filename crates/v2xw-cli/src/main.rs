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

fn sensitivity(args: &v2xw_cli::cli::SensitivityArgs) -> v2xw_cli::Result<String> {
    use v2xw_engine::sensitivity as sens;
    if args.catalogue {
        return catalogue(args);
    }
    if args.expect {
        return expect(args);
    }
    let Some(spec_path) = &args.spec else {
        return Err(v2xw_cli::CliError::Agent(
            "sensitivity: give a spec file or --catalogue".into(),
        ));
    };
    let (spec, base) = sens::Spec::load(spec_path)?;
    if args.base_only {
        let m = sens::measure(base)?;
        let mut out = String::new();
        for (k, v) in &m.values {
            out.push_str(&format!("{k:<24} {v:.4}\n"));
        }
        for k in [
            "range_m",
            "range90_m",
            "prr_0_200",
            "prr_200_400",
            "prr_400_600",
            "prr_600_1000",
        ] {
            out.push_str(&format!("{k:<24} {:?}\n", m.get(k)));
        }
        // The PRR curve, one 20 m bin per entry out to 1 km: PRR (receivers).
        out.push_str("prr per 20 m bin         ");
        for (c, d) in m.prr_bins.iter().take(50) {
            if *c == 0 {
                out.push_str("- ");
            } else {
                out.push_str(&format!("{:.2}({c}) ", *d as f64 / *c as f64));
            }
        }
        out.push('\n');
        return Ok(out);
    }
    let results = sens::run_spec(&spec, &base, |r| {
        eprintln!(
            "{} {} ({}: {:?} -> {:?})",
            if r.pass { "PASS" } else { "FAIL" },
            r.row.name,
            r.row.metric,
            r.base,
            r.value
        );
    })?;
    if let Some(path) = &args.markdown {
        std::fs::write(path, sens::markdown(&results))
            .map_err(|e| v2xw_cli::CliError::Agent(format!("{}: {e}", path.display())))?;
    }
    let text = if args.json {
        json(&results, "sensitivity results")?
    } else {
        sens::table(&results)
    };
    if results.iter().all(|r| r.pass) {
        Ok(text)
    } else {
        print!("{text}");
        Err(v2xw_cli::CliError::Agent(
            "sensitivity: a row did not move as expected".into(),
        ))
    }
}

/// `v2xw sensitivity --expect`: the expectation table and the interaction checks.
fn expect(args: &v2xw_cli::cli::SensitivityArgs) -> v2xw_cli::Result<String> {
    use v2xw_engine::sensitivity::{expect as ex, sweep};
    let families = sweep::Families::load(&args.families)?;
    let mut expectations = sweep::load_expectations(&args.expectations)?;
    let mut interactions = ex::load_interactions(&args.interactions)?;
    if !args.only.is_empty() {
        let keep = |p: &str| args.only.iter().any(|o| p.starts_with(o.as_str()));
        expectations.retain(|e| keep(&e.setting));
        interactions.retain(|i| i.factor.keys().chain(i.condition.keys()).any(|k| keep(k)));
    }
    let (settings, unplanned) = ex::expect_plan(&families, &expectations);
    if args.plan {
        let runs: usize = settings.iter().map(|s| s.values.len()).sum();
        return Ok(format!(
            "{} rows: {} settings, {runs} perturbations; {} interactions ({} runs at most); {} unplanned\n{}",
            expectations.len(),
            settings.len(),
            interactions.len(),
            interactions.len() * 4,
            unplanned.len(),
            unplanned.join("\n")
        ));
    }
    let bases = families.bases(&args.families)?;
    let clock = v2xw_cli::wall::Stopwatch::start();
    let mut result = sweep::run_cached(
        settings,
        vec![],
        &bases,
        &expectations,
        args.jobs,
        args.cache.as_deref(),
        |n, total, o| eprintln!("[{n}/{total}] {} = {}", o.path, o.label),
    )?;
    let inter = ex::run_interactions(
        &interactions,
        &bases,
        args.jobs,
        args.cache.as_deref(),
        |n, total, what| eprintln!("[x {n}/{total}] {what}"),
    )?;
    result.wall_s = clock.elapsed_s();
    if let Some(path) = &args.markdown {
        let note = "Generated by `v2xw sensitivity --expect` (families `scenarios/sensitivity/catalogue.yaml`, rows `crates/v2xw-engine/tests/expectations/*.yaml`, interactions `crates/v2xw-engine/tests/expectations/interactions/*.yaml`).";
        std::fs::write(path, ex::markdown(&result, &inter, note))
            .map_err(|e| v2xw_cli::CliError::Agent(format!("{}: {e}", path.display())))?;
    }
    let text = if args.json {
        json(&(&result.verdicts, &inter), "expectation verdicts")?
    } else {
        ex::report(&result, &inter, &unplanned)
    };
    let failed = result.verdicts.iter().any(|v| !v.pass) || inter.iter().any(|v| !v.pass);
    if failed || !unplanned.is_empty() {
        print!("{text}");
        return Err(v2xw_cli::CliError::Agent(
            "sensitivity --expect: a row failed or could not be planned".into(),
        ));
    }
    Ok(text)
}

/// `v2xw sensitivity --catalogue`: the catalogue-wide sweep.
fn catalogue(args: &v2xw_cli::cli::SensitivityArgs) -> v2xw_cli::Result<String> {
    use v2xw_engine::sensitivity::sweep;
    let families = sweep::Families::load(&args.families)?;
    let expectations = sweep::load_expectations(&args.expectations)?;
    let (mut settings, mut skipped) = sweep::plan(&families, &expectations);
    if !args.only.is_empty() {
        let keep = |p: &str| args.only.iter().any(|o| p.starts_with(o.as_str()));
        settings.retain(|s| keep(&s.path));
        skipped.retain(|(p, _)| keep(p));
    }
    if args.plan {
        let mut out = String::new();
        for s in &settings {
            let vals: Vec<String> = s.values.iter().map(|(l, v)| format!("{l}={v}")).collect();
            out.push_str(&format!(
                "{:<58} {:<8} {:<16} [{}] {}\n",
                s.path,
                s.kind,
                s.status,
                s.families.join(","),
                vals.join(" ")
            ));
        }
        let runs: usize = settings.iter().map(|s| s.values.len()).sum();
        out.push_str(&format!(
            "{} settings, {runs} perturbations, {} skipped\n",
            settings.len(),
            skipped.len()
        ));
        return Ok(out);
    }
    let bases = families.bases(&args.families)?;
    let clock = v2xw_cli::wall::Stopwatch::start();
    let mut result = sweep::run_cached(
        settings,
        skipped,
        &bases,
        &expectations,
        args.jobs,
        args.cache.as_deref(),
        |n, total, o| {
            eprintln!(
                "[{n}/{total}] {} = {} on {}: {} moved{}",
                o.path,
                o.label,
                o.family.as_deref().unwrap_or("-"),
                o.moved.len(),
                if o.family.is_none() {
                    " (refused)"
                } else if o.error.is_some() {
                    " (run failed)"
                } else {
                    ""
                }
            );
        },
    )?;
    result.wall_s = clock.elapsed_s();
    if let Some(path) = &args.markdown {
        let note = format!(
            "Generated by `v2xw sensitivity --catalogue` (families `{}`, expectations `{}`).",
            args.families.display(),
            args.expectations
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("`, `")
        );
        std::fs::write(path, sweep::markdown(&result, &note))
            .map_err(|e| v2xw_cli::CliError::Agent(format!("{}: {e}", path.display())))?;
    }
    let bad = result.contradictions().len();
    let text = if args.json {
        json(&result, "catalogue sweep")?
    } else {
        format!(
            "{} settings, {} runs, {} dead, {} contradictions of {} expectations, {} refused, {:.0} s\n",
            result.settings.len(),
            result.outcomes.len(),
            result.dead().len(),
            bad,
            result.verdicts.len(),
            result.refused().len(),
            result.wall_s
        )
    };
    if args.strict && bad > 0 {
        print!("{text}");
        return Err(v2xw_cli::CliError::Agent(
            "sensitivity --catalogue: an expectation was contradicted".into(),
        ));
    }
    Ok(text)
}

fn dispatch(cli: &Cli) -> v2xw_cli::Result<String> {
    match &cli.command {
        Command::Agent(args) => agent::agent(&args.to_options()),
        Command::Serve(args) => agent::serve(&args.to_options()),
        Command::CheckTraffic(args) => {
            let mut o = v2xw_cli::check_traffic::CheckOptions::new(&args.dir);
            o.green_stop_s = args.green_stop_s;
            o.examples = args.examples;
            o.trace = args.trace;
            let c = v2xw_cli::check_traffic::check_traffic(&o)?;
            let text = if args.json {
                json(&c, "traffic check")?
            } else {
                v2xw_cli::check_traffic::format(&c)
            };
            if c.ok {
                Ok(text)
            } else {
                print!("{text}");
                Err(v2xw_cli::CliError::Agent(
                    "check-traffic: the run failed its traffic invariants".into(),
                ))
            }
        }
        Command::Sensitivity(args) => sensitivity(args),
        Command::MaReplay(args) => v2xw_cli::ma_replay::run(&args.dir),
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
        Command::ExportSumo(args) => {
            let outcome = v2xw_cli::export_sumo::export(&args.scenario, &args.out)?;
            if args.json {
                json(&outcome, "export outcome")
            } else {
                Ok(v2xw_cli::export_sumo::format(&outcome))
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
        Command::Compare(args) => {
            use v2xw_cli::runcompare::Format;
            let format = if args.json {
                Format::Json
            } else if args.markdown {
                Format::Markdown
            } else if args.csv {
                Format::Csv
            } else {
                Format::Text
            };
            v2xw_cli::runcompare::run(&args.a, &args.b, format, args.all)
        }
        Command::CompareSignatures(args) => {
            let outcome = v2xw_cli::sigcompare::run(&args.to_options())?;
            if args.json {
                json(&outcome, "signature comparison")
            } else {
                Ok(v2xw_cli::sigcompare::format(&outcome))
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
        if let Some(list) = v
            .get_mut("cities")
            .and_then(serde_json::Value::as_array_mut)
        {
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
