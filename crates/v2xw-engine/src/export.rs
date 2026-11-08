//! The scenario's `exporters` list: what a finished run writes beside its recording.
//!
//! A scenario may name its exporters, and until this module the list
//! was refused outright: the engine had no exporter stage, and a list nobody read was worse
//! than none. The stage is deliberately *after* the run and *over the recording*, not inside
//! the run loop: every exporter reads the same stored records a reviewer would read, so an
//! export can never disagree with the recording beside it, and the run loop stays free of
//! file formats.
//!
//! | id | What it writes |
//! |---|---|
//! | `recording` | keeps the MCAP recording itself (it is written whenever an exporter is named) |
//! | `jsonl` | one JSON Lines table per recorded channel, with a schema sidecar |
//! | `parquet` | the same tables as Apache Parquet — the primary analysis format |
//! | `arrow` | the same tables as Arrow IPC |
//!
//! | `veremi` | VeReMi traces per receiver plus the ground-truth log (`v2xw_record::dataset::veremi`) |
//!
//! `opts.profile` is `full` (the default) or `node`: the `node` profile drops every
//! ground-truth channel and column (§5), which is what a blind evaluation hands a detector.
//!
//! `veremi` takes `opts.codebook` — `veremi-ext` (the default: the VeReMi Extension's F2MD
//! attack enum 0–19, sliced `-A<code>` files, Kamel et al. ICC 2020) or `veremi-1`
//! (VeReMi 1.0's bit codes 1/2/4/8/16, Van der Heijden et al. SecureComm 2018) — and
//! `opts.slice_s`, the slice length in seconds (default 3600, F2MD `VeReMiSliceTime`;
//! 1 s to one week). Naming it also makes every BSM's `node.tx` content carry the
//! broadcast position accuracy and acceleration the trace's `pos_noise` and `acl` are
//! read from ([`records_broadcast_accuracy`]); a run that names no `veremi` exporter
//! records exactly what it did before. The written traces go through the leakage linter
//! and the export fails if it finds anything.
//!
//! The SUMO-format exporters ([`crate::sumo_outputs`]) write the
//! file SUMO's output of the same name would, readable by SUMO's tools, into
//! `out_dir/<id>/`. Every one needs a `full` recording (they read `gt.kinematics`), and
//! naming any of them makes the run record `gt.spawn` and `gt.despawn`
//! ([`records_lifecycle`]); a run that names none records exactly what it did before.
//!
//! | id | file | SUMO equivalent | options (default, unit; SUMO option) |
//! |---|---|---|---|
//! | `sumo-fcd` | `fcd.xml` | `--fcd-output` | `period_s` (every step, s; `--device.fcd.period`), `precision` (2; `--precision`) |
//! | `sumo-tripinfo` | `tripinfo.xml` | `--tripinfo-output` | `write_unfinished` (false; `--tripinfo-output.write-unfinished`), `precision` |
//! | `sumo-edgedata` | `edgedata.xml` | `<edgeData>` | `period_s` (whole run, s; `period`), `precision` |
//! | `sumo-lanedata` | `lanedata.xml` | `<laneData>` | `period_s`, `precision` |
//! | `sumo-summary` | `summary.xml` | `--summary-output` | `period_s` (every step, s; `--summary-output.period`), `precision` |
//! | `sumo-e1` | `e1.xml` | `<inductionLoop>` | `detectors: [{id, lane, pos_m}]` (required; `lane` a SUMO lane id), `period_s` (whole run, s; `period`), `precision` |
//! | `sumo-e2` | `e2.xml` | `<laneAreaDetector>` | `detectors: [{id, lane, pos_m, length_m}]`, `period_s`, `time_threshold_s` (1 s), `speed_threshold_mps` (1.39 m/s), `jam_threshold_m` (10 m; SUMO's `timeThreshold`, `speedThreshold`, `jamThreshold`), `precision` |

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{EngineError, Result};
use crate::scenario::schema::ExporterSpec;

/// The exporter ids this build implements.
pub const EXPORTERS: [&str; 13] = [
    "recording",
    "jsonl",
    "parquet",
    "arrow",
    "veremi",
    "sumo-fcd",
    "sumo-tripinfo",
    "sumo-edgedata",
    "sumo-lanedata",
    "sumo-summary",
    "sumo-e1",
    "sumo-e2",
    "dataset",
];

/// Whether a run with these exporters records the BSM's broadcast accuracy and
/// acceleration on `node.tx` (the `veremi` exporter reads them).
#[must_use]
pub fn records_broadcast_accuracy(specs: &[ExporterSpec]) -> bool {
    specs.iter().any(|x| {
        x.id == "veremi"
            || (x.id == "dataset"
                && v2xw_record::dataset::release::ReleaseOptions::from_opts(&x.opts)
                    .is_ok_and(|o| o.compat_veremi))
    })
}

/// Whether the scenario names the dataset release exporter (docs/dataset/schema.md).
/// Everything the record contract adds (docs/dataset/record-contract.md) is recorded only
/// then, so a default run records exactly what it did before.
#[must_use]
pub fn records_dataset(specs: &[ExporterSpec]) -> bool {
    specs.iter().any(|x| x.id == "dataset")
}

/// The `dataset` exporter's options, or the scenario conflict naming the bad one.
///
/// # Errors
/// [`EngineError::Scenario`] naming `exporters[i].opts.<field>`.
pub fn dataset_options_of(
    spec: &ExporterSpec,
    index: usize,
) -> Result<v2xw_record::dataset::release::ReleaseOptions> {
    v2xw_record::dataset::release::ReleaseOptions::from_opts(&spec.opts).map_err(|(field, why)| {
        EngineError::Scenario(crate::ScenarioError::conflict(
            format!("exporters[{index}].{field}"),
            why,
        ))
    })
}

/// The share of vehicles that attack: the populations' `fraction`s, plus the vehicles named
/// by `actor_ids` (or `count`) over the fleet size. `None` when the scenario names no
/// vehicle attacker.
fn attacker_fraction(attackers: &Value, fleet: Option<f64>) -> Option<f64> {
    let list = attackers.as_array().filter(|a| !a.is_empty())?;
    let mut share = 0.0;
    let mut named = 0.0;
    for a in list {
        if let Some(f) = a.get("fraction").and_then(Value::as_f64) {
            share += f;
        } else if let Some(ids) = a
            .get("actor_ids")
            .and_then(Value::as_array)
            .filter(|i| !i.is_empty())
        {
            named += ids.len() as f64;
        } else if let Some(c) = a.get("count").and_then(Value::as_f64) {
            named += c;
        }
    }
    if named > 0.0 {
        share += named / fleet.filter(|f| *f > 0.0)?;
    }
    Some(v2xw_core::math::quantize_to(share, 1e-6))
}

/// A scenario parameter block as `name=value` strings, sorted by name (a list of strings
/// rather than JSON text, so a dataset viewer can filter on it).
fn mix_params(p: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(m)) = p else {
        return Vec::new();
    };
    let mut out: Vec<String> = m
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| match v {
            Value::String(x) => format!("{k}={x}"),
            other => format!("{k}={other}"),
        })
        .collect();
    out.sort();
    out
}

/// The threats a scenario declares outside `threats.attackers`, as `attack_mix` entries
/// under their catalogue names (`crates/v2xw-threat/src/catalog.rs`): jammers
/// (`ConstantJamming`, `RandomDutyJamming`, `ReactiveJamming`), compromised roadside
/// units (their `params.kind`, `PoisonForwardedReports` by default) and the passive
/// eavesdropper (`PrivacyObserver`).
fn other_threats(scenario: &Value) -> Vec<v2xw_record::dataset::release::AttackMix> {
    use v2xw_record::dataset::release::AttackMix;
    let threats = scenario.get("threats").cloned().unwrap_or(Value::Null);
    let window = |p: Option<&Value>, k: &str| p.and_then(|p| p.get(k)).and_then(Value::as_f64);
    let mut out = Vec::new();
    for j in threats
        .get("jammers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = match j.get("id").and_then(Value::as_str) {
            Some("attacker/jammer/constant") => "ConstantJamming",
            Some("attacker/jammer/pulsed") => "RandomDutyJamming",
            Some("attacker/jammer/reactive") => "ReactiveJamming",
            Some(other) => other,
            None => continue,
        };
        let p = j.get("params");
        out.push(AttackMix {
            id: id.to_string(),
            fraction: None,
            count: None,
            from_s: window(p, "from_s"),
            to_s: window(p, "to_s"),
            params: mix_params(p),
        });
    }
    let units = threats
        .get("compromised_rsus")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if units > 0 {
        let p = threats
            .get("compromised_rsu_attack")
            .and_then(|m| m.get("params"));
        let kind = p
            .and_then(|p| p.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("PoisonForwardedReports");
        let mut params = mix_params(p);
        params.retain(|x| !x.starts_with("kind="));
        params.push(format!("units={units}"));
        params.sort();
        out.push(AttackMix {
            id: kind.to_string(),
            fraction: None,
            count: None,
            from_s: window(p, "from_s"),
            to_s: window(p, "to_s"),
            params,
        });
    }
    if let Some(e) = threats.get("eavesdropper").filter(|e| !e.is_null()) {
        out.push(AttackMix {
            id: "PrivacyObserver".to_string(),
            fraction: None,
            count: None,
            from_s: None,
            to_s: None,
            params: mix_params(e.get("params")),
        });
    }
    out
}

/// `runs.parquet`'s row for a recording: the scenario the run executed
/// (`scenario.resolved.yaml` beside the recording, when it is there) and the recording's
/// manifest. A value neither carries is left out rather than guessed.
#[must_use]
pub fn dataset_meta(
    recording: &Path,
    opts: &v2xw_record::dataset::release::ReleaseOptions,
) -> v2xw_record::dataset::release::RunMeta {
    use v2xw_record::dataset::release::{AttackMix, RunMeta};
    let manifest: Value = v2xw_record::Reader::open_paged(recording)
        .ok()
        .and_then(|r| r.manifest_json().and_then(|j| serde_json::from_str(j).ok()))
        .unwrap_or(Value::Null);
    let scenario: Value = recording
        .parent()
        .map(|d| d.join("scenario.resolved.yaml"))
        .filter(|p| p.is_file())
        .and_then(|p| crate::Scenario::load_unvalidated(p).ok())
        .and_then(|s| serde_json::to_value(&s).ok())
        .unwrap_or(Value::Null);
    let at = |v: &Value, path: &str| -> Value {
        let mut cur = v;
        for k in path.split('.') {
            cur = match cur.get(k) {
                Some(x) => x,
                None => return Value::Null,
            };
        }
        cur.clone()
    };
    let s = |v: &Value, path: &str| -> Option<String> {
        match at(v, path) {
            Value::String(x) => Some(x),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };
    let f = |v: &Value, path: &str| at(v, path).as_f64();
    let attackers = at(&scenario, "threats.attackers");
    let mut mix: Vec<AttackMix> = attackers
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| AttackMix {
            id: a
                .get("id")
                .and_then(Value::as_str)
                .map(|i| v2xw_record::dataset::release::crosswalk::short_name(i).to_string())
                .unwrap_or_default(),
            fraction: a.get("fraction").and_then(Value::as_f64),
            count: a
                .get("actor_ids")
                .and_then(Value::as_array)
                .filter(|i| !i.is_empty())
                .map(|i| i.len() as i64)
                .or_else(|| a.get("count").and_then(Value::as_i64)),
            from_s: f(a, "schedule.from_s"),
            to_s: f(a, "schedule.to_s"),
            params: mix_params(a.get("params")),
        })
        .collect();
    mix.extend(other_threats(&scenario));
    let name = s(&scenario, "meta.name");
    let scenario_hash = s(&manifest, "scenario_hash");
    let local: Vec<String> = at(&scenario, "detection.local")
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();
    let scms_mode = if scenario.is_null() {
        None
    } else if s(&scenario, "security.protocol.id").is_some() {
        Some("in-loop".to_string())
    } else {
        Some("off".to_string())
    };
    RunMeta {
        run_id: opts
            .run_id
            .clone()
            .unwrap_or_else(|| RunMeta::default_run_id(name.as_deref(), scenario_hash.as_deref())),
        seed: s(&manifest, "master_seed")
            .or_else(|| s(&scenario, "seed"))
            .unwrap_or_else(|| "0".into()),
        name,
        scenario_hash,
        engine_version: s(&manifest, "engine_version"),
        git_sha: s(&manifest, "git_commit"),
        city: s(&scenario, "world.source.city").or_else(|| {
            s(&scenario, "world.source.path").and_then(|p| {
                Path::new(&p)
                    .file_stem()
                    .map(|x| x.to_string_lossy().into_owned())
            })
        }),
        world_source: s(&scenario, "world.source.kind"),
        t0_utc: s(&scenario, "time.t0"),
        duration_s: f(&scenario, "time.duration_s"),
        origin_lat_deg: None,
        origin_lon_deg: None,
        vehicles: at(&scenario, "actors.vehicles.demand.fleet_size").as_i64(),
        equipped_fraction: f(&scenario, "actors.vehicles.equipped_fraction"),
        demand_veh_per_h: f(&scenario, "actors.vehicles.demand.params.veh_per_h"),
        rat: s(&scenario, "radio.rat"),
        scms_mode,
        calendar: s(&scenario, "time.calendar"),
        local_suite: (!local.is_empty()).then(|| local.join(",")),
        ma_policy: s(&scenario, "detection.ma.id"),
        attacker_fraction: attacker_fraction(
            &attackers,
            at(&scenario, "actors.vehicles.demand.fleet_size").as_f64(),
        ),
        attack_mix: mix,
    }
}

/// The dataset release: `<out>/dataset/` (docs/dataset/schema.md §3), with the VeReMi
/// compatibility traces under `compat/veremi/<run_id>/` written first so the release's
/// manifest covers them.
fn run_dataset(
    spec: &ExporterSpec,
    index: usize,
    recording: &Path,
    out_dir: &Path,
    world: Option<&v2xw_world::World>,
) -> Result<Exported> {
    let opts = dataset_options_of(spec, index)?;
    let dir = out_dir.join("dataset");
    let mut meta = dataset_meta(recording, &opts);
    // The world's geodetic origin anchors the x_m / y_m columns (runs.origin_*); the
    // recording does not carry it, the run's world does (a procedural world: (0, 0)).
    if let Some(w) = world {
        let o: v2xw_core::geo::GeoOrigin = w.origin.into();
        meta.origin_lat_deg = Some(v2xw_core::math::quantize_to(o.lat_deg, 1e-7));
        meta.origin_lon_deg = Some(v2xw_core::math::quantize_to(o.lon_deg, 1e-7));
    }
    // The scenario that regenerates the run travels with it (schema.md §3 `scenarios/`).
    if let Some(src) = recording
        .parent()
        .map(|d| d.join("scenario.resolved.yaml"))
        .filter(|p| p.is_file())
    {
        let dst = dir.join("scenario.yaml");
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::copy(&src, &dst).map(|_| ()))
            .map_err(|e| EngineError::Io {
                path: dst.display().to_string(),
                source: e,
            })?;
    }
    if opts.compat_veremi {
        let vdir = dir.join("compat").join("veremi").join(&meta.run_id);
        // Streamed from the recording in two passes, never copied: a copy of every decoded
        // reception was most of the export's peak memory.
        let mut reader =
            v2xw_record::Reader::open_paged(recording).map_err(record_error(recording))?;
        let export = v2xw_record::dataset::veremi::from_source_with(
            &mut reader,
            &v2xw_record::dataset::veremi::VeremiOptions::default(),
        )
        .map_err(record_error(&vdir))?;
        drop(reader);
        let files =
            v2xw_record::dataset::veremi::write(&vdir, &export).map_err(record_error(&vdir))?;
        let lint =
            v2xw_record::dataset::veremi::lint(&files, &export).map_err(record_error(&vdir))?;
        if !lint.is_clean() {
            return Err(EngineError::Io {
                path: vdir.display().to_string(),
                source: std::io::Error::other(format!(
                    "the VeReMi compatibility traces failed the leakage linter: {}",
                    lint.summary()
                )),
            });
        }
    }
    let mut reader = v2xw_record::Reader::open_paged(recording).map_err(record_error(recording))?;
    let written = v2xw_record::dataset::release::write(&mut reader, &dir, &opts, &meta)
        .map_err(record_error(&dir))?;
    Ok(Exported {
        exporter: spec.id.clone(),
        files: written
            .files
            .into_iter()
            .map(|f| ExportedPath {
                bytes: std::fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0),
                rows: f
                    .rows
                    .map_or(0, |r| usize::try_from(r).unwrap_or(usize::MAX)),
                path: f.path,
            })
            .collect(),
    })
}

/// Whether a run with these exporters records `gt.spawn` and `gt.despawn` (every SUMO
/// exporter reads them: depart and arrival, why a vehicle left, its length).
#[must_use]
pub fn records_lifecycle(specs: &[ExporterSpec]) -> bool {
    specs.iter().any(|x| crate::sumo_outputs::is_sumo(&x.id))
}

/// A SUMO exporter's options.
///
/// # Errors
/// A conflict naming the option when one is unknown or out of range.
pub fn sumo_options_of(
    spec: &ExporterSpec,
    index: usize,
) -> Result<crate::sumo_outputs::SumoOptions> {
    crate::sumo_outputs::options_of(&spec.id, &spec.opts).map_err(|(field, why)| {
        EngineError::Scenario(crate::ScenarioError::conflict(
            format!("exporters[{index}].{field}"),
            why,
        ))
    })
}

/// The `veremi` exporter's options.
///
/// # Errors
/// A conflict naming the option when one is unknown or out of range.
pub fn veremi_options_of(
    spec: &ExporterSpec,
    index: usize,
) -> Result<v2xw_record::dataset::veremi::VeremiOptions> {
    v2xw_record::dataset::veremi::VeremiOptions::from_opts(&spec.opts).map_err(|(field, why)| {
        EngineError::Scenario(crate::ScenarioError::conflict(
            format!("exporters[{index}].{field}"),
            why,
        ))
    })
}

/// What one exporter wrote.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Exported {
    /// The exporter id.
    pub exporter: String,
    /// Every file it wrote, with its row count and size.
    pub files: Vec<ExportedPath>,
}

/// One written file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportedPath {
    /// Where it is.
    pub path: PathBuf,
    /// Rows, for a table; 0 for a sidecar or the recording.
    pub rows: usize,
    /// Bytes on disk.
    pub bytes: u64,
}

/// The profile an exporter's `opts` asks for.
///
/// # Errors
/// A conflict naming the field when `opts.profile` is neither `full` nor `node`.
pub fn profile_of(spec: &ExporterSpec, index: usize) -> Result<v2xw_record::ExportProfile> {
    match spec.opts.get("profile").and_then(Value::as_str) {
        None | Some("full") => Ok(v2xw_record::ExportProfile::Full),
        Some("node") => Ok(v2xw_record::ExportProfile::NodeOnly),
        Some(other) => Err(EngineError::Scenario(crate::ScenarioError::conflict(
            format!("exporters[{index}].opts.profile"),
            format!("is '{other}'; an export profile is 'full' or 'node'"),
        ))),
    }
}

/// Runs every exporter in `specs` over the finished recording at `recording`, writing into
/// `out_dir`. Exporters run in list order and each writes into `out_dir/<id>/`.
///
/// # Errors
/// [`EngineError::Io`] if the recording cannot be read or a file cannot be written, and a
/// scenario conflict for an id this build does not implement.
pub fn run_exporters(
    specs: &[ExporterSpec],
    recording: &Path,
    out_dir: &Path,
) -> Result<Vec<Exported>> {
    run_exporters_in(specs, recording, out_dir, None)
}

/// [`run_exporters`] with the run's world, which the SUMO exporters need for lane and
/// edge ids, lengths and speed limits.
///
/// # Errors
/// As [`run_exporters`], and a scenario conflict when a SUMO exporter is named without a
/// world.
pub fn run_exporters_in(
    specs: &[ExporterSpec],
    recording: &Path,
    out_dir: &Path,
    world: Option<&v2xw_world::World>,
) -> Result<Vec<Exported>> {
    let mut out = Vec::new();
    let mut records = None;
    let mut tracks = None;
    for (i, spec) in specs.iter().enumerate() {
        if crate::sumo_outputs::is_sumo(&spec.id) {
            let opts = sumo_options_of(spec, i)?;
            let Some(world) = world else {
                return Err(EngineError::Scenario(crate::ScenarioError::conflict(
                    format!("exporters[{i}].id"),
                    format!(
                        "'{}' needs the run's world, and this caller has none",
                        spec.id
                    ),
                )));
            };
            if records.is_none() {
                let mut reader =
                    v2xw_record::Reader::open(recording).map_err(record_error(recording))?;
                records = Some(reader.records(None).map_err(record_error(recording))?);
            }
            let dir = out_dir.join(&spec.id);
            let io = |why: String| EngineError::Io {
                path: dir.display().to_string(),
                source: std::io::Error::other(why),
            };
            if tracks.is_none() {
                tracks = Some(
                    crate::sumo_outputs::Tracks::from_records(records.as_deref().unwrap_or(&[]))
                        .map_err(io)?,
                );
            }
            let tr = tracks.as_ref().expect("read above");
            let (path, rows) =
                crate::sumo_outputs::write(&spec.id, &opts, tr, world, &dir).map_err(io)?;
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push(Exported {
                exporter: spec.id.clone(),
                files: vec![ExportedPath { path, rows, bytes }],
            });
            continue;
        }
        if spec.id == "dataset" {
            out.push(run_dataset(spec, i, recording, out_dir, world)?);
            continue;
        }
        let format = match spec.id.as_str() {
            "recording" => {
                let bytes = std::fs::metadata(recording).map(|m| m.len()).unwrap_or(0);
                out.push(Exported {
                    exporter: spec.id.clone(),
                    files: vec![ExportedPath {
                        path: recording.to_path_buf(),
                        rows: 0,
                        bytes,
                    }],
                });
                continue;
            }
            "jsonl" => v2xw_record::ExportFormat::Jsonl,
            "parquet" => v2xw_record::ExportFormat::Parquet,
            "arrow" => v2xw_record::ExportFormat::ArrowIpc,
            "veremi" => {
                let opts = veremi_options_of(spec, i)?;
                if records.is_none() {
                    let mut reader =
                        v2xw_record::Reader::open(recording).map_err(record_error(recording))?;
                    records = Some(reader.records(None).map_err(record_error(recording))?);
                }
                let dir = out_dir.join(&spec.id);
                let export = v2xw_record::dataset::veremi::from_records_with(
                    records.as_deref().unwrap_or(&[]),
                    &opts,
                )
                .map_err(record_error(&dir))?;
                let files = v2xw_record::dataset::veremi::write(&dir, &export)
                    .map_err(record_error(&dir))?;
                let lint = v2xw_record::dataset::veremi::lint(&files, &export)
                    .map_err(record_error(&dir))?;
                if !lint.is_clean() {
                    return Err(EngineError::Io {
                        path: dir.display().to_string(),
                        source: std::io::Error::other(format!(
                            "the VeReMi traces failed the leakage linter: {}",
                            lint.summary()
                        )),
                    });
                }
                out.push(Exported {
                    exporter: spec.id.clone(),
                    files: files
                        .into_iter()
                        .map(|f| ExportedPath {
                            path: f.path,
                            rows: f.rows,
                            bytes: f.bytes,
                        })
                        .collect(),
                });
                continue;
            }
            other => {
                return Err(EngineError::Scenario(crate::ScenarioError::conflict(
                    format!("exporters[{i}].id"),
                    format!(
                        "'{other}' is not an exporter this build has; one of {}",
                        EXPORTERS.join(", ")
                    ),
                )));
            }
        };
        let dir = out_dir.join(&spec.id);
        std::fs::create_dir_all(&dir).map_err(|e| EngineError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let exporter =
            v2xw_record::Exporter::new(&dir, profile_of(spec, i)?).map_err(record_error(&dir))?;
        // A table export streams the recording rather than reading every record into
        // memory first: on a five-minute Manhattan run that list alone was gigabytes, and
        // the server ran it after the run, beside the run's own state. The records are
        // reused only when an exporter above already had to read them.
        let files = match records.as_deref() {
            Some(records) => exporter.export_all(records, format),
            None => {
                let mut reader =
                    v2xw_record::Reader::open(recording).map_err(record_error(recording))?;
                exporter.export_recording(&mut reader, format)
            }
        }
        .map_err(record_error(&dir))?;
        out.push(Exported {
            exporter: spec.id.clone(),
            files: files
                .into_iter()
                .map(|f| ExportedPath {
                    path: f.path,
                    rows: f.rows,
                    bytes: f.bytes,
                })
                .collect(),
        });
    }
    Ok(out)
}

fn record_error(path: &Path) -> impl Fn(v2xw_record::RecordError) -> EngineError + '_ {
    move |e| EngineError::Io {
        path: path.display().to_string(),
        source: std::io::Error::other(e.to_string()),
    }
}

#[cfg(test)]
mod dataset_meta_tests {
    use serde_json::json;

    /// `runs.attack_mix` lists every threat the scenario declares under its catalogue
    /// name, with the parameters as `name=value` strings (never JSON text).
    #[test]
    fn the_attack_mix_names_jammers_compromised_units_and_the_eavesdropper() {
        let s = json!({"threats": {
            "jammers": [
                {"id": "attacker/jammer/constant", "params": {"power_dbm": 23, "from_s": 5.0, "to_s": 9.0}},
                {"id": "attacker/jammer/pulsed", "params": {"period_ms": 50, "duty": 0.3}},
                {"id": "attacker/jammer/reactive", "params": {"trigger_dbm": -85}}
            ],
            "compromised_rsus": [0, 2],
            "compromised_rsu_attack": {"id": "threat/attacker/compromised-rsu",
                                       "params": {"kind": "FalseCrl", "fabricated_entries": 4}},
            "eavesdropper": {"id": "threat/observer/passive-privacy", "params": {"sniffer_fraction": 0.5}}
        }});
        let mix = super::other_threats(&s);
        let ids: Vec<&str> = mix.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "ConstantJamming",
                "RandomDutyJamming",
                "ReactiveJamming",
                "FalseCrl",
                "PrivacyObserver"
            ]
        );
        assert_eq!(mix[0].from_s, Some(5.0));
        assert_eq!(mix[0].params, ["from_s=5.0", "power_dbm=23", "to_s=9.0"]);
        assert_eq!(mix[3].params, ["fabricated_entries=4", "units=2"]);
        assert!(mix.iter().all(|m| m.fraction.is_none()));
        let a = json!([{"actor_ids": [3, 9]}, {"fraction": 0.05}, {"count": 2}]);
        assert_eq!(super::attacker_fraction(&a, Some(40.0)), Some(0.15));
        assert_eq!(super::attacker_fraction(&json!([]), Some(40.0)), None);
        let none = super::other_threats(&json!({"threats": {"attackers": []}}));
        assert!(none.is_empty());
        assert_eq!(
            super::mix_params(Some(&json!({"b": "x", "a": 1, "n": null}))),
            ["a=1", "b=x"]
        );
    }
}
