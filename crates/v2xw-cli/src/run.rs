//! `v2xw run` — run a scenario and write its recording, metrics and manifest.
//!
//! # What a run leaves behind
//!
//! | File | What it is | Reproducible? |
//! |---|---|---|
//! | `recording.mcap` | the MCAP container: every record, the manifest metadata, the scenario attachment | its **data section** is; its file digest is not, see below |
//! | `scenario.resolved.yaml` | the scenario after base merge, migration and defaults — the document the run actually executed | yes |
//! | `metrics.json` | every `metric.sample` record the run emitted | yes |
//! | `run-report.json` | event, frame and record counts, per channel, plus the recording's content digest | yes |
//! | `manifest.json` | [`v2xw_core::manifest::Manifest`] with the four files above digested and `data_digest` finalised | all but `build_utc` and the recording's entry |
//!
//! # Why the file digest of an MCAP is not the run's digest
//!
//! [`v2xw_record::Reader::content_digest`] documents it: the `mcap` 0.25 writer emits the
//! repeated schema and channel records of the *summary section* from a `HashMap`, so their
//! order — and with it the file's SHA-256 — can vary between two runs of the same program
//! on the same machine. The summary is an index; reordering an index changes no content.
//! So this command reports **both**: `manifest.json` carries the honest SHA-256 of the
//! bytes on disk, and `run-report.json` carries `content_digest`, which is the SHA-256 over
//! every message's topic, instant and bytes in stored order. Two runs of one scenario must
//! agree on the second. They are allowed to disagree on the first, and `v2xw run --json`
//! prints both so the operator can see which one moved.
//!
//! # No clock reaches the engine
//!
//! `--build-utc` is the manifest's caller-supplied timestamp; left out, it comes from
//! [`crate::wall::now_iso8601_utc`], the only clock read on this path. The stopwatch that
//! times the run is a measurement of the tool, never an input to it, and nothing it
//! produces is written to a digested field.

use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use v2xw_core::time::Duration;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_record::{Cadence, Profile, RecordingOptions, RecordingWriter};

use crate::error::{CliError, Result};
use crate::tally::{ChannelTally, Tally};
use crate::wall::{Stopwatch, now_iso8601_utc};

/// How `v2xw run` was asked to run.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The scenario file.
    pub scenario: PathBuf,
    /// Where the outputs go. `None` means `runs/<meta.name>`.
    pub out: Option<PathBuf>,
    /// The manifest's caller-supplied timestamp. `None` reads the clock.
    pub build_utc: Option<String>,
    /// How often a keyframe is declared in the recording's cadence, milliseconds.
    pub keyframe_ms: u64,
    /// Record the `NODE-only` profile, which refuses every ground-truth record.
    pub node_only: bool,
    /// Write a recording at all. `false` runs the engine into a counting sink, which is
    /// what a scaling measurement wants: it measures the engine and not the container.
    pub record: bool,
    /// Also attach the world payload to the recording. Off by default: a city world is
    /// tens of megabytes and most runs do not need it carried twice.
    pub attach_world: bool,
    /// Read the recording back and verify it before reporting.
    pub verify: bool,
    /// Without a recording: hash every record as it streams past (`stream_digest`).
    pub stream_digest: bool,
    /// Override `time.duration_s`. Changes the scenario hash, as it should.
    pub duration_s: Option<f64>,
    /// Override `actors.vehicles.demand.rate_veh_per_h`. Changes the scenario hash.
    pub rate_veh_per_h: Option<f64>,
    /// Run on this library city instead of the file's world ([`Scenario::with_city`]).
    pub city: Option<String>,
    /// Print the outcome as JSON on stdout instead of as a table.
    pub json: bool,
    /// How a long run is driven, watched and kept.
    pub long: LongRun,
}

/// How a long run is driven, watched and kept. Everything defaults to what a short run
/// wants, so a caller that does not care writes `long: LongRun::default()`.
#[derive(Debug, Clone, Default)]
pub struct LongRun {
    /// Write a progress line every this many simulated seconds (`progress.jsonl` in the
    /// output directory and one line on stderr). `None` picks: every 15 simulated minutes
    /// for a run of an hour or more, none for a shorter one. `Some(0.0)` turns it off.
    pub progress_every_s: Option<f64>,
    /// Write `progress.jsonl` but print nothing on stderr.
    pub quiet: bool,
    /// Write a checkpoint every this many simulated seconds (`checkpoint.json`), and
    /// record in segments closed at each one ([`crate::checkpoint`]). `None` or `0`: none.
    pub checkpoint_every_s: Option<f64>,
    /// How much of the run reaches the recording.
    pub record_level: crate::stream::RecordLevel,
    /// Stop at the first progress line or checkpoint at or after this simulated second, as a
    /// crash would, with the outputs past the last checkpoint on disk:
    /// the open segment is left unfinished. For testing resume.
    pub stop_at_s: Option<f64>,
}

impl LongRun {
    /// The progress interval this run uses, seconds, or `None` for none.
    #[must_use]
    pub fn progress_interval_s(&self, duration_s: f64) -> Option<f64> {
        match self.progress_every_s {
            Some(e) if e > 0.0 => Some(e),
            Some(_) => None,
            None if duration_s >= 3600.0 => Some(900.0),
            None => None,
        }
    }

    /// The checkpoint interval, seconds, or `None` for none.
    #[must_use]
    pub fn checkpoint_interval_s(&self) -> Option<f64> {
        self.checkpoint_every_s.filter(|s| *s > 0.0)
    }
}

/// What a run produced, in the shape `--json` prints and `run-report.json` stores.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RunOutcome {
    /// The scenario's name.
    pub scenario_name: String,
    /// Where the outputs went.
    pub out_dir: String,
    /// The scenario hash the run executed under.
    pub scenario_hash: String,
    /// The world's content hash.
    pub world_hash: String,
    /// The master seed, as hex.
    pub master_seed_hex: String,
    /// Simulated seconds the scenario asked for.
    pub duration_s: f64,
    /// The engine's own run report.
    pub report: RunReport,
    /// Per-channel record counts.
    pub channels: BTreeMap<String, ChannelTally>,
    /// How many metric samples were emitted.
    pub metric_samples: u64,
    /// SHA-256 over every record the engine emitted, in order — whatever reached the
    /// recording. The number a resumed run is checked against, and the one two runs of a
    /// scenario agree on with or without a recording.
    pub record_digest: Option<String>,
    /// How much of the run reached the recording.
    pub record_level: crate::stream::RecordLevel,
    /// The recording's segments, in order, when it was recorded in segments
    /// (`--checkpoint-every`); empty for one `recording.mcap`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recording_segments: Vec<String>,
    /// The recording's size in bytes, when one was written.
    pub recording_bytes: Option<u64>,
    /// SHA-256 of the recording file's bytes — *not* reproducible, see the module note.
    pub recording_file_sha256: Option<String>,
    /// SHA-256 over the recording's data section — the number two runs must agree on.
    pub content_digest: Option<String>,
    /// SHA-256 over every record as it was produced (`--digest` without a recording):
    /// `v2xw_engine::DigestRecorder`'s, the same stream digest the engine's tests use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_digest: Option<String>,
    /// What the reader's verification found, when `--verify` was on.
    pub verified: Option<VerifiedSummary>,
    /// Container counters the writer reported.
    pub container: Option<ContainerSummary>,
    /// Timings, which are wall-clock measurements of the tool and reach no digest.
    pub timing: Timing,
    /// When this invocation carried on from a checkpoint: from where, and what the replay
    /// to it cost. Not in the digested report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumed: Option<ResumeInfo>,
    /// What the scenario's `exporters` wrote, over the recording (`v2xw_engine::export`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<v2xw_engine::export::Exported>,
}

/// What reading the recording back found.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifiedSummary {
    /// Records walked.
    pub records: u64,
    /// VWP frames walked.
    pub frames: u64,
    /// Chunks whose stored CRC was present and matched.
    pub chunks_checksummed: u64,
    /// Chunks that declared no CRC, so nothing in them was checked.
    pub chunks_without_checksum: u64,
    /// True only when every chunk carried a checksum and it matched.
    pub integrity_verified: bool,
}

/// What the container did with the bytes.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ContainerSummary {
    /// Messages written, both encodings together.
    pub message_count: u64,
    /// Chunks the writer closed.
    pub chunk_count: u64,
    /// The largest number of uncompressed message bytes in one chunk.
    pub largest_chunk_bytes: u64,
}

/// Where a resumed run carried on from.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ResumeInfo {
    /// The checkpoint's simulated second.
    pub from_s: f64,
    /// How many times the run has now been resumed.
    pub resumes: u32,
    /// Wall-clock seconds the replay to the checkpoint took.
    pub replay_s: f64,
}

/// Wall-clock seconds, by phase. Never an input to anything.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Timing {
    /// Loading and merging the scenario.
    pub load_s: f64,
    /// [`Engine::build`], which is where the world import happens.
    pub build_s: f64,
    /// Driving the engine (from the checkpoint, when resumed).
    pub run_s: f64,
    /// Replaying to the checkpoint, when resumed.
    pub replay_s: f64,
    /// Reading the recording back.
    pub verify_s: f64,
    /// Everything, end to end.
    pub total_s: f64,
    /// Wall-clock seconds per simulated second of `run_s` alone.
    pub wall_s_per_sim_s: f64,
}

/// Runs a scenario.
///
/// # Errors
/// [`CliError::Engine`] if the scenario will not load, will not validate or the run fails;
/// [`CliError::Record`] if the recording cannot be written or read back;
/// [`CliError::Io`] for any output that cannot be written.
pub fn run(opts: &RunOptions) -> Result<RunOutcome> {
    let total = Stopwatch::start();

    let load = Stopwatch::start();
    // Validated after the overrides, because the scenario checked has to be the one that
    // runs: validating the file first refused `--duration-s 180` on a 40 s file whose
    // attacker schedule runs to 180 s, and let `--duration-s 5` through on a file whose
    // timeline has an event at 10 s — an event that would then never fire.
    let mut scenario = Scenario::load_unvalidated(&opts.scenario)?;
    if let Some(city) = &opts.city {
        scenario = scenario
            .with_city(city)
            .map_err(v2xw_engine::EngineError::Scenario)?;
    }
    apply_overrides(&mut scenario, opts);
    scenario
        .validate()
        .map_err(v2xw_engine::EngineError::Scenario)?;
    let load_s = load.elapsed_s();

    let out_dir = opts
        .out
        .clone()
        .unwrap_or_else(|| match &opts.city {
            Some(city) => PathBuf::from("runs").join(format!("{}-{city}", scenario.meta.name)),
            None => PathBuf::from("runs").join(&scenario.meta.name),
        });
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| CliError::io("cannot create output directory", &out_dir, e))?;

    let build_utc = opts
        .build_utc
        .clone()
        .unwrap_or_else(|| now_iso8601_utc().to_string());

    // Every exporter reads the recording, so a run that names one must record.
    if !scenario.exporters.is_empty() && !opts.record {
        return Err(
            v2xw_engine::EngineError::Scenario(v2xw_engine::ScenarioError::conflict(
                "exporters",
                "names exporters, and every exporter reads the recording, which --no-recording \
             turns off",
            ))
            .into(),
        );
    }
    if !scenario.exporters.is_empty() && opts.long.checkpoint_interval_s().is_some() {
        return Err(
            v2xw_engine::EngineError::Scenario(v2xw_engine::ScenarioError::conflict(
                "exporters",
                "names exporters, which read one recording.mcap, and --checkpoint-every \
                 records in segments; run the exporters over each segment afterwards",
            ))
            .into(),
        );
    }

    let build = Stopwatch::start();
    let engine = Engine::build(scenario, &build_utc)?;
    let build_s = build.elapsed_s();

    execute(
        engine,
        &Invocation {
            opts: opts.clone(),
            out_dir,
            build_utc,
            load_s,
            build_s,
            total,
        },
        None,
    )
}

/// Carries on a run from its checkpoint (`v2xw resume`): see [`crate::checkpoint`].
///
/// # Errors
/// [`CliError::Diverged`] when the rebuilt run does not reach the checkpoint's record
/// digest, and every error [`run`] can return.
pub fn resume(checkpoint: &Path, quiet: bool, json: bool) -> Result<RunOutcome> {
    let total = Stopwatch::start();
    let load = Stopwatch::start();
    let ck = crate::checkpoint::Checkpoint::read(checkpoint)?;
    let out_dir = checkpoint
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let resolved_path = out_dir.join("scenario.resolved.yaml");
    let text = std::fs::read_to_string(&resolved_path)
        .map_err(|e| CliError::io("cannot read", &resolved_path, e))?;
    let load_s = load.elapsed_s();
    let build = Stopwatch::start();
    let scenario = Scenario::parse(&text, None)?;
    let mut engine = Engine::build(scenario, &ck.build_utc)?;
    if engine.manifest().scenario_hash != ck.scenario_hash {
        // The resolved document did not hash alike (a default whose serialised form moved):
        // the scenario file and its overrides, as the run was started.
        let mut scenario = Scenario::load_unvalidated(&ck.scenario_path)?;
        // In `run`'s order: the city first, then the overrides it would otherwise restate.
        if let Some(city) = &ck.city {
            scenario = scenario
                .with_city(city)
                .map_err(v2xw_engine::EngineError::Scenario)?;
        }
        if let Some(d) = ck.duration_override_s {
            scenario.time.duration_s = d;
        }
        if let Some(r) = ck.rate_override_veh_per_h {
            scenario.actors.vehicles.demand.rate_veh_per_h = Some(r);
        }
        scenario
            .validate()
            .map_err(v2xw_engine::EngineError::Scenario)?;
        engine = Engine::build(scenario, &ck.build_utc)?;
        if engine.manifest().scenario_hash != ck.scenario_hash {
            return Err(CliError::Diverged {
                what: format!(
                    "the scenario hashes to {}, the checkpoint was taken under {}",
                    engine.manifest().scenario_hash,
                    ck.scenario_hash
                ),
            });
        }
    }
    if engine.manifest().world_hash != ck.world_hash {
        return Err(CliError::Diverged {
            what: format!(
                "the world hashes to {}, the checkpoint was taken over {} (is the map extract \
                 the same, and is this the directory the run was started in, {}?)",
                engine.manifest().world_hash,
                ck.world_hash,
                ck.cwd
            ),
        });
    }
    let build_s = build.elapsed_s();
    let opts = RunOptions {
        scenario: PathBuf::from(&ck.scenario_path),
        out: Some(out_dir.clone()),
        build_utc: Some(ck.build_utc.clone()),
        keyframe_ms: ck.keyframe_ms,
        node_only: ck.node_only,
        record: ck.record,
        attach_world: ck.attach_world,
        verify: ck.verify,
        stream_digest: false,
        duration_s: ck.duration_override_s,
        rate_veh_per_h: ck.rate_override_veh_per_h,
        city: ck.city.clone(),
        json,
        long: LongRun {
            progress_every_s: Some(ck.progress_every_s.unwrap_or(0.0)),
            quiet,
            checkpoint_every_s: Some(ck.every_s),
            record_level: ck.record_level,
            stop_at_s: None,
        },
    };
    execute(
        engine,
        &Invocation {
            opts,
            out_dir,
            build_utc: ck.build_utc.clone(),
            load_s,
            build_s,
            total,
        },
        Some(ck),
    )
}

/// What [`execute`] needs from the command that started it.
struct Invocation {
    opts: RunOptions,
    out_dir: PathBuf,
    build_utc: String,
    load_s: f64,
    build_s: f64,
    total: Stopwatch,
}

/// The first multiple of `every` strictly after `t`.
fn next_multiple_after(t: u64, every: u64) -> u64 {
    (t / every + 1).saturating_mul(every)
}

/// Runs a built engine to its end — from its checkpoint when resuming — and writes every
/// output.
#[allow(clippy::too_many_lines)]
fn execute(
    mut engine: Engine,
    inv: &Invocation,
    resume: Option<crate::checkpoint::Checkpoint>,
) -> Result<RunOutcome> {
    use crate::checkpoint::{Checkpoint, Segments, Sink};

    let opts = &inv.opts;
    let out_dir = &inv.out_dir;
    let name = engine.scenario().meta.name.clone();
    let duration_s = engine.scenario().time.duration_s;
    let mobility_step = engine.scenario().time.mobility_step();
    let horizon_ns = engine.scenario().time.horizon_ns();
    let exporters = engine.scenario().exporters.clone();

    // The resolved scenario is what the run executed, not what the author typed: bases are
    // merged, migrations applied and defaults filled. It is also what gets attached to the
    // recording, so an attachment and a hash can never describe different documents. It is
    // written first, because a resume rebuilds the run from it.
    let resolved = engine
        .scenario()
        .to_yaml()
        .map_err(v2xw_engine::EngineError::from)?;
    write_file(out_dir, "scenario.resolved.yaml", resolved.as_bytes())?;
    let manifest = engine.manifest().clone();

    let cadence = cadence_for(opts.keyframe_ms, mobility_step)?;
    let profile = if opts.node_only {
        Profile::NodeOnly
    } else {
        Profile::Full
    };
    let level = opts.long.record_level;
    let checkpoint_every_ns = opts
        .long
        .checkpoint_interval_s()
        .map(|s| ((s * 1e9).round() as u64).max(1));
    let progress_every_ns = opts
        .long
        .progress_interval_s(duration_s)
        .map(|s| ((s * 1e9).round() as u64).max(1));

    // A recording with no scene does not pay for encoding one. (The snapshot stream is
    // binary and apart from the records, so no record and no digest moves.)
    if opts.record && !level.keeps_frames() {
        engine.set_snapshots_enabled(false);
    }

    // A resume first brings the engine back to the checkpoint, with every output off, and
    // goes no further unless the record digest there is the checkpoint's.
    let mut replay = Tally::new(Sink::none());
    let mut replay_s = 0.0;
    let mut progress = match resume.as_ref() {
        Some(ck) => crate::progress::Progress::resume(
            out_dir,
            opts.long.quiet,
            ck.progress_bytes,
            ck.t_ns as f64 / 1e9,
            ck.wall_s,
        ),
        None => crate::progress::Progress::open(out_dir, opts.long.quiet),
    };
    if let Some(ck) = resume.as_ref() {
        let watch = Stopwatch::start();
        progress.note(&format!(
            "resuming: replaying the first {} with outputs off, to reach the checkpoint",
            crate::progress::fmt_sim(ck.t_ns as f64 / 1e9)
        ));
        let slice = progress_every_ns.unwrap_or(3_600_000_000_000);
        let mut target = 0;
        while target < ck.t_ns {
            target = next_multiple_after(target, slice).min(ck.t_ns);
            if engine.advance(target, &mut replay)? {
                break;
            }
            progress.note(&format!(
                "  replayed {} of {} ({:.0} s)",
                crate::progress::fmt_sim(target as f64 / 1e9),
                crate::progress::fmt_sim(ck.t_ns as f64 / 1e9),
                watch.elapsed_s()
            ));
        }
        if replay.records() != ck.records || replay.record_digest_hex() != ck.record_digest {
            return Err(CliError::Diverged {
                what: format!(
                    "at {} s the rebuilt run had emitted {} records with digest {}, the \
                     checkpoint recorded {} with digest {}",
                    ck.t_ns as f64 / 1e9,
                    replay.records(),
                    replay.record_digest_hex(),
                    ck.records,
                    ck.record_digest
                ),
            });
        }
        replay_s = watch.elapsed_s();
        progress.note(&format!(
            "  the checkpoint's record digest is reproduced ({} records); carrying on",
            ck.records
        ));
        progress.restart_slice();
    }

    // The outputs.
    let recording_path = out_dir.join("recording.mcap");
    let rec_opts = RecordingOptions {
        cadence,
        profile,
        ..RecordingOptions::default()
    };
    let sink = if !opts.record {
        Sink::none()
    } else if checkpoint_every_ns.is_some() {
        let first = resume.as_ref().map_or(0, |ck| ck.segments_done);
        // What a crash left after the last checkpoint is written again.
        for i in first..first + 10_000 {
            let p = out_dir.join(crate::checkpoint::segment_name(i));
            if !p.exists() {
                break;
            }
            let _ = std::fs::remove_file(&p);
        }
        if resume.is_none() {
            let _ = std::fs::remove_file(&recording_path);
        }
        let world = if opts.attach_world {
            Some(v2xw_world::serde_vwp::write(engine.world())?.bytes)
        } else {
            None
        };
        Sink::segmented(
            Segments {
                dir: out_dir.clone(),
                next: first,
                opts: rec_opts,
                manifest_json: manifest.to_json_pretty()?,
                scenario_yaml: resolved.clone(),
                world,
            },
            level,
        )?
    } else {
        let mut writer = RecordingWriter::create(&recording_path, rec_opts)?;
        // The manifest goes in before the first record, so that a run killed halfway still
        // says what produced the bytes it did write.
        writer.write_manifest(&manifest.to_json_pretty()?)?;
        writer.attach("scenario.yaml", "application/yaml", resolved.as_bytes())?;
        if opts.attach_world {
            let payload = v2xw_world::serde_vwp::write(engine.world())?;
            writer.attach("world.vwb", "application/octet-stream", &payload.bytes)?;
        }
        Sink::single(recording_path.clone(), writer, level)
    };
    let metrics_path = out_dir.join("metrics.json");
    let metrics = match resume.as_ref() {
        Some(ck) => {
            crate::stream::MetricsStream::resume(&metrics_path, ck.metrics_bytes, ck.metric_samples)?
        }
        None => crate::stream::MetricsStream::create(&metrics_path)?,
    };
    let mut sink = sink;
    if let Some(ck) = resume.as_ref() {
        sink.carry_totals(ck.container);
    }
    let (mut tally, _) = replay.rewrap(sink);
    tally.stream_metrics(metrics);

    // The run, in slices: a progress line and a checkpoint at their intervals. A slice
    // boundary stops the loop between two events (`Engine::advance`), so the slices change
    // nothing the run computes. A checkpoint is taken one nanosecond before its instant,
    // so everything stamped at the instant itself — the keyframe due then among it —
    // opens the next recording segment.
    let start_ns = resume.as_ref().map_or(0, |ck| ck.t_ns);
    let resumes = resume.as_ref().map_or(0, |ck| ck.resumes + 1);
    let mut next_progress = progress_every_ns.map(|e| next_multiple_after(start_ns, e));
    let mut next_checkpoint =
        checkpoint_every_ns.map(|e| next_multiple_after(start_ns.saturating_add(1), e) - 1);
    // A checkpoint on request (Ctrl-C, `kill`, the terminal closing: `crate::signal`) is
    // looked for once a simulated second, at the same one-nanosecond-early boundary.
    const POLL_NS: u64 = 1_000_000_000;
    let mut next_poll = checkpoint_every_ns.map(|_| next_multiple_after(start_ns.saturating_add(1), POLL_NS) - 1);
    if checkpoint_every_ns.is_some() {
        crate::signal::install();
    }
    let stop_at_ns = opts.long.stop_at_s.map(|s| (s * 1e9).round() as u64);
    let every_s = checkpoint_every_ns.map_or(0.0, |e| e as f64 / 1e9);
    let cwd = std::env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    // Everything a checkpoint at `c` records, once the segment is rolled and the metrics
    // flushed.
    let checkpoint_at = |c: u64,
                         segments_done: u32,
                         metrics_bytes: u64,
                         tally: &mut Tally<Sink>,
                         progress: &crate::progress::Progress| Checkpoint {
        format: crate::checkpoint::FORMAT.to_string(),
        method: "replay".to_string(),
        scenario_name: name.clone(),
        scenario_hash: manifest.scenario_hash.clone(),
        world_hash: manifest.world_hash.clone(),
        build_utc: inv.build_utc.clone(),
        scenario_path: opts.scenario.display().to_string(),
        duration_override_s: opts.duration_s,
        rate_override_veh_per_h: opts.rate_veh_per_h,
        city: opts.city.clone(),
        cwd: cwd.clone(),
        keyframe_ms: opts.keyframe_ms,
        node_only: opts.node_only,
        record: opts.record,
        record_level: level,
        attach_world: opts.attach_world,
        verify: opts.verify,
        every_s,
        progress_every_s: progress_every_ns.map(|e| e as f64 / 1e9),
        t_ns: c,
        record_digest: tally.record_digest_hex(),
        records: tally.records(),
        metric_samples: tally.metric_count(),
        metrics_bytes,
        segments_done,
        progress_bytes: progress.bytes(),
        wall_s: progress.wall_s(),
        resumes,
        container: tally.inner_mut().totals(),
    };
    let run_watch = Stopwatch::start();
    loop {
        let target = [next_progress, next_checkpoint, next_poll]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(u64::MAX);
        let done = engine.advance(target, &mut tally)?;
        let t_ns = if done { engine.now() } else { target };
        if let Some(p) = next_progress
            && (p == target || done)
        {
            let mut extra = serde_json::Map::new();
            extra.insert("records".into(), tally.records().into());
            extra.insert("metric_samples".into(), tally.metric_count().into());
            if let Some(b) = tally.inner_mut().bytes_on_disk() {
                extra.insert("recording_bytes".into(), b.into());
            }
            progress.line(t_ns as f64 / 1e9, duration_s, &engine.census(), extra);
            next_progress = progress_every_ns.map(|e| next_multiple_after(p, e));
        }
        let scheduled = !done && next_checkpoint == Some(target);
        let asked = !done && next_poll == Some(target) && crate::signal::requested();
        if (scheduled || asked) && target.saturating_add(1) < horizon_ns {
            let segments_done = tally.inner_mut().roll()?;
            let metrics_bytes = match tally.metrics_mut() {
                Some(m) => m.flush()?,
                None => 0,
            };
            checkpoint_at(target, segments_done, metrics_bytes, &mut tally, &progress)
                .write(out_dir)?;
            progress.note(&format!(
                "checkpoint at {} ({})",
                crate::progress::fmt_sim((target + 1) as f64 / 1e9),
                out_dir.join(crate::checkpoint::FILE).display()
            ));
            if asked {
                // Stopped on request: every output up to the checkpoint is complete, the
                // segment opened by the roll holds nothing and is removed.
                let _ = progress.flush();
                tally.into_inner().discard_open();
                return Err(CliError::StoppedOnRequest {
                    at_s: (target + 1) as f64 / 1e9,
                    checkpoint: out_dir.join(crate::checkpoint::FILE).display().to_string(),
                });
            }
        }
        if scheduled {
            next_checkpoint = checkpoint_every_ns.map(|e| target.saturating_add(e));
        }
        if !done && next_poll == Some(target) {
            next_poll = Some(target.saturating_add(POLL_NS));
        }
        // `--stop-at`: stop here as a crash would, after the outputs have moved on past the
        // last checkpoint — the metrics written so far are on disk, the progress line is
        // there, the open segment is never finished — so a resume has something to cut back.
        if !done && stop_at_ns.is_some_and(|s| t_ns >= s) {
            if let Some(m) = tally.metrics_mut() {
                let _ = m.flush();
            }
            tally.into_inner().abandon();
            return Err(CliError::Interrupted {
                at_s: t_ns as f64 / 1e9,
            });
        }
        if done {
            break;
        }
    }
    let report = engine.finish_run(&mut tally);
    let run_s = run_watch.elapsed_s();

    let channels = tally.by_channel().clone();
    let record_digest = tally.record_digest_hex();
    let stream_digest = opts.stream_digest.then(|| record_digest.clone());
    let (metrics_sha, metric_samples) = match tally.take_metrics() {
        Some(m) => m.finish()?,
        None => (v2xw_core::hash::sha256_hex(b""), 0),
    };
    let (files, totals) = tally.into_inner().finish()?;
    let segmented = checkpoint_every_ns.is_some() && opts.record;

    let mut outcome = RunOutcome {
        scenario_name: name,
        out_dir: out_dir.display().to_string(),
        scenario_hash: manifest.scenario_hash.clone(),
        world_hash: manifest.world_hash.clone(),
        master_seed_hex: format!("{:#x}", manifest.master_seed),
        duration_s,
        report,
        channels,
        metric_samples,
        record_digest: Some(record_digest),
        record_level: level,
        recording_segments: if segmented {
            files
                .iter()
                .filter_map(|f| f.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect()
        } else {
            Vec::new()
        },
        recording_bytes: None,
        recording_file_sha256: None,
        content_digest: None,
        stream_digest,
        verified: None,
        container: totals.map(|t| ContainerSummary {
            message_count: t.message_count,
            chunk_count: t.chunk_count,
            largest_chunk_bytes: t.largest_chunk_bytes,
        }),
        timing: Timing::default(),
        resumed: resume.as_ref().map(|ck| ResumeInfo {
            from_s: ck.t_ns as f64 / 1e9,
            resumes,
            replay_s,
        }),
        exports: Vec::new(),
    };

    let mut verify_s = 0.0;
    let mut file_digests = Vec::new();
    if opts.record {
        let mut bytes = 0;
        for f in &files {
            let (sha, len) = crate::checkpoint::file_sha256(f)?;
            bytes += len;
            let fname = f
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            file_digests.push((fname, sha));
        }
        outcome.recording_bytes = Some(bytes);
        if !segmented {
            outcome.recording_file_sha256 = file_digests.first().map(|(_, s)| s.clone());
        } else {
            let index = serde_json::json!({
                "segments": file_digests.iter().map(|(f, s)| serde_json::json!({"file": f, "sha256": s})).collect::<Vec<_>>(),
            });
            write_file(
                out_dir,
                "recording_index.json",
                serde_json::to_string_pretty(&index)
                    .unwrap_or_default()
                    .as_bytes(),
            )?;
        }
        if opts.verify {
            let v = Stopwatch::start();
            outcome.content_digest = Some(v2xw_core::hash::hex_encode(
                &crate::checkpoint::content_digest(&files)?,
            ));
            let mut sum = VerifiedSummary {
                records: 0,
                frames: 0,
                chunks_checksummed: 0,
                chunks_without_checksum: 0,
                integrity_verified: true,
            };
            for f in &files {
                let mut reader = v2xw_record::Reader::open_paged(f)?;
                let report = reader.verify()?;
                sum.records += report.records;
                sum.frames += report.frames;
                sum.chunks_checksummed += report.chunks_checksummed;
                sum.chunks_without_checksum += report.chunks_without_checksum;
                sum.integrity_verified &= report.integrity_verified();
            }
            outcome.verified = Some(sum);
            verify_s = v.elapsed_s();
        }
    }

    if !exporters.is_empty() {
        outcome.exports =
            v2xw_engine::export::run_exporters(&exporters, &recording_path, out_dir)?;
    }

    // The deterministic artefacts are digested before the manifest is written, because the
    // manifest digests them.
    let mut manifest = manifest;
    manifest.add_file("scenario.resolved.yaml", resolved.as_bytes());
    manifest.add_file_digest("metrics.json", metrics_sha);

    // The report file deliberately excludes the timings: they are wall-clock numbers, and
    // a digested artefact that carries one is an artefact that never matches itself.
    // The fields below are dropped from the digested copy, each for the same reason: they
    // are facts about *this invocation* and not about the run. Leaving any of them in makes
    // `run-report.json` differ between two runs of one scenario — or between a run and the
    // same run resumed from a checkpoint — which would put a spurious mismatch into the
    // manifest's `data_digest` and make a real one unnoticeable.
    let mut deterministic = outcome.clone();
    deterministic.timing = Timing::default();
    deterministic.recording_file_sha256 = None;
    deterministic.out_dir = String::new();
    deterministic.resumed = None;
    let report_json = serde_json::to_string_pretty(&deterministic).map_err(|e| CliError::Json {
        what: "run report",
        source: e,
    })?;
    write_and_digest(
        &mut manifest,
        out_dir,
        "run-report.json",
        report_json.as_bytes(),
    )?;
    for (f, sha) in &file_digests {
        manifest.files.push(v2xw_core::manifest::FileDigest {
            path: f.clone(),
            sha256: sha.clone(),
        });
    }
    manifest.finalize();
    let manifest_json = manifest.to_json_pretty()?;
    write_file(out_dir, "manifest.json", manifest_json.as_bytes())?;

    outcome.timing = Timing {
        load_s: inv.load_s,
        build_s: inv.build_s,
        run_s,
        replay_s,
        verify_s,
        total_s: inv.total.elapsed_s(),
        wall_s_per_sim_s: if duration_s > 0.0 {
            run_s / ((horizon_ns - start_ns.min(horizon_ns)) as f64 / 1e9).max(f64::MIN_POSITIVE)
        } else {
            f64::NAN
        },
    };
    Ok(outcome)
}

/// Runs a scenario into memory and returns the record digest, for a determinism check that
/// does not go near a file.
///
/// This is what "two runs of one scenario produce identical outputs" means at its
/// narrowest: no container, no manifest, no timestamp — just the record stream.
///
/// # Errors
/// [`CliError::Engine`] if the scenario will not load or the run fails.
pub fn digest_only(scenario: &Path, build_utc: &str) -> Result<String> {
    let scenario = Scenario::load(scenario)?;
    let mut engine = Engine::build(scenario, build_utc)?;
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder)?;
    Ok(recorder.digest_hex())
}

/// The recording cadence: keyframes every `keyframe_ms`, one delta per mobility step.
fn cadence_for(keyframe_ms: u64, mobility_step: Duration) -> Result<Cadence> {
    Cadence::new(Duration::from_millis(keyframe_ms), mobility_step).map_err(|e| {
        CliError::BadArgument {
            flag: "keyframe-ms",
            problem: format!(
                "{keyframe_ms} ms is not a usable keyframe period against this scenario's \
                 {} ms mobility step: {e}",
                mobility_step.as_nanos() / 1_000_000
            ),
        }
    })
}

/// Applies the two scale overrides, which exist so that one scenario can be swept without
/// four near-identical copies of it on disk. Both change the scenario hash, because both
/// change the scenario the engine executes.
fn apply_overrides(scenario: &mut Scenario, opts: &RunOptions) {
    if let Some(d) = opts.duration_s {
        scenario.time.duration_s = d;
    }
    if let Some(r) = opts.rate_veh_per_h {
        scenario.actors.vehicles.demand.rate_veh_per_h = Some(r);
    }
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = dir.join(name);
    let mut f = BufWriter::new(
        std::fs::File::create(&path).map_err(|e| CliError::io("cannot write", &path, e))?,
    );
    f.write_all(bytes)
        .map_err(|e| CliError::io("cannot write", &path, e))?;
    f.flush()
        .map_err(|e| CliError::io("cannot write", &path, e))
}

fn write_and_digest(
    manifest: &mut v2xw_core::manifest::Manifest,
    dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    write_file(dir, name, bytes)?;
    manifest.add_file(name, bytes);
    Ok(())
}
