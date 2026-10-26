//! The scenario's `exporters` list: what a finished run writes beside its recording.
//!
//! 03-interfaces.md §13 lets a scenario name its exporters, and until this module the list
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
//! | `parquet` | the same tables as Apache Parquet — the primary analysis format (08-measurement-and-data.md §5) |
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
//! The SUMO-format exporters (adoption package WP-06, [`crate::sumo_outputs`]) write the
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
pub const EXPORTERS: [&str; 12] = [
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
];

/// Whether a run with these exporters records the BSM's broadcast accuracy and
/// acceleration on `node.tx` (the `veremi` exporter reads them).
#[must_use]
pub fn records_broadcast_accuracy(specs: &[ExporterSpec]) -> bool {
    specs.iter().any(|x| x.id == "veremi")
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
                    format!("'{}' needs the run's world, and this caller has none", spec.id),
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
        if records.is_none() {
            let mut reader =
                v2xw_record::Reader::open(recording).map_err(record_error(recording))?;
            records = Some(reader.records(None).map_err(record_error(recording))?);
        }
        let dir = out_dir.join(&spec.id);
        std::fs::create_dir_all(&dir).map_err(|e| EngineError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let exporter =
            v2xw_record::Exporter::new(&dir, profile_of(spec, i)?).map_err(record_error(&dir))?;
        let files = exporter
            .export_all(records.as_deref().unwrap_or(&[]), format)
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
