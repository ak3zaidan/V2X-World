//! Checkpoints of a long run, and the segmented recording that makes them resumable.
//!
//! # What a checkpoint is in this build: a verified replay point
//!
//! A checkpoint written by `v2xw run --checkpoint-every 1h` is `checkpoint.json` in the
//! output directory. It holds what is needed to bring the run back to the instant it was
//! taken *exactly*, and to prove it was brought back exactly:
//!
//! * the resolved scenario (beside it, `scenario.resolved.yaml`), its hash, the world hash
//!   and the build timestamp — every input the engine has;
//! * the instant `t`, and the **record digest** at `t`: SHA-256 over every record the
//!   engine emitted up to and including `t` (`crate::tally`), with the record count;
//! * how much of each output was complete at `t`: the recording segments closed at `t`,
//!   the bytes and samples of `metrics.json`, the lines of `progress.jsonl`.
//!
//! `v2xw resume <checkpoint.json>` rebuilds the engine from those inputs, runs it to `t`
//! with every output switched off, **refuses to continue unless the record digest at `t`
//! equals the checkpoint's**, cuts the outputs back to what was complete at `t`, and
//! carries on writing them. Because the engine is deterministic (ADR 0004), the run that
//! carries on is the run that was interrupted, and the digest check is what turns that
//! from an assumption into a fact on every resume. The finished outputs equal an
//! uninterrupted run's: `tests/checkpoint.rs` proves it by digest.
//!
//! **What this does not do.** It does not store the engine's memory. The engine's state is
//! several hundred types across thirteen crates — the mobility model, every radio and
//! node, the SCMS entities and their keys, the metric providers — and serialising them
//! all is a change to every crate, recorded as the open item in
//! `docs/design/findings/long-runs.md`. So resuming costs the replay: re-running the
//! simulated time before `t` with no recording, which measured at about 0.6 of the cost
//! of the original run (the table in that document). What a checkpoint saves is
//! everything written before `t` — the recording segments, the metrics, the progress —
//! and the certainty that the resumed run is the same run.
//!
//! # The segmented recording
//!
//! An MCAP file is only readable once its footer is written, and a writer cannot be
//! reopened to append. So a run with checkpoints records into segments: at every
//! checkpoint the open segment is finished (footer, summary, index) and the next one
//! opened, `recording.0000.mcap`, `recording.0001.mcap`, … . A crash loses at most the
//! segment being written, which resume deletes and writes again. Each segment is a
//! complete recording in its own right — manifest metadata and the scenario attached —
//! so `v2xw info` and the page open any one of them, and the run's content digest is
//! taken over all of them in order (`recording_index.json` lists them).

use std::io::Write;
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::RunRecorder;
use v2xw_record::{RecordingOptions, RecordingWriter};

use crate::error::{CliError, Result};
use crate::stream::RecordLevel;

/// The checkpoint file's format tag.
pub const FORMAT: &str = "v2xw-checkpoint/1";

/// The checkpoint file's name in the output directory.
pub const FILE: &str = "checkpoint.json";

/// One checkpoint: the run's inputs, the instant, and how far each output had got.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    /// [`FORMAT`].
    pub format: String,
    /// How the state is brought back: `"replay"` (see the module note).
    pub method: String,
    /// The scenario's name.
    pub scenario_name: String,
    /// The scenario hash the run executes under; resume refuses a different one.
    pub scenario_hash: String,
    /// The world's content hash; resume refuses a different one.
    pub world_hash: String,
    /// The manifest's build timestamp.
    pub build_utc: String,
    /// The scenario file the run was started from, and its two overrides: the fallback
    /// if the resolved scenario does not reproduce the hash.
    pub scenario_path: String,
    /// `--duration-s` as given.
    pub duration_override_s: Option<f64>,
    /// `--rate-veh-per-h` as given.
    pub rate_override_veh_per_h: Option<f64>,
    /// `--city` as given: the library city the run was moved onto. Absent in checkpoints
    /// written before it was recorded, which resume on the scenario file's own world.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// The working directory the run was started in, against which the scenario's
    /// relative paths (the map extract) resolve.
    pub cwd: String,
    /// The recording's keyframe period.
    pub keyframe_ms: u64,
    /// The NODE-only profile.
    pub node_only: bool,
    /// Whether a recording is written.
    pub record: bool,
    /// How much of the run reaches it.
    pub record_level: RecordLevel,
    /// Whether the world was attached.
    pub attach_world: bool,
    /// Whether the recording is read back at the end.
    pub verify: bool,
    /// The checkpoint interval, seconds.
    pub every_s: f64,
    /// The progress interval, seconds, if any.
    pub progress_every_s: Option<f64>,
    /// The simulated instant, ns: every event at or before it has run.
    pub t_ns: u64,
    /// The record digest at `t_ns`, hex.
    pub record_digest: String,
    /// Records emitted up to `t_ns`.
    pub records: u64,
    /// Metric samples emitted up to `t_ns`.
    pub metric_samples: u64,
    /// The bytes of `metrics.json` complete at `t_ns`.
    pub metrics_bytes: u64,
    /// Recording segments finished at `t_ns`.
    pub segments_done: u32,
    /// The bytes of `progress.jsonl` complete at `t_ns`.
    pub progress_bytes: u64,
    /// Wall-clock seconds the run had spent in the engine up to `t_ns`, across resumes.
    /// A measurement, never an input.
    pub wall_s: f64,
    /// How many times this run has been resumed.
    pub resumes: u32,
    /// What the container had written in the finished segments, so the run report's totals
    /// cover the whole run after a resume.
    #[serde(default)]
    pub container: SinkTotals,
}

impl Checkpoint {
    /// Writes the checkpoint into `dir` atomically: a temporary file, synced, then renamed
    /// over the previous one, so a crash during the write leaves the last good checkpoint.
    ///
    /// # Errors
    /// [`CliError::Io`] if the file cannot be written.
    pub fn write(&self, dir: &Path) -> Result<()> {
        let path = dir.join(FILE);
        let tmp = dir.join(format!("{FILE}.partial"));
        let json = serde_json::to_vec_pretty(self).map_err(|e| CliError::Json {
            what: "checkpoint",
            source: e,
        })?;
        let mut f =
            std::fs::File::create(&tmp).map_err(|e| CliError::io("cannot write", &tmp, e))?;
        f.write_all(&json)
            .and_then(|()| f.sync_all())
            .map_err(|e| CliError::io("cannot write", &tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| CliError::io("cannot write", &path, e))
    }

    /// Reads a checkpoint.
    ///
    /// # Errors
    /// [`CliError::Io`] if it cannot be read, [`CliError::BadArgument`] if it is not one.
    pub fn read(path: &Path) -> Result<Checkpoint> {
        let bytes = std::fs::read(path).map_err(|e| CliError::io("cannot read", path, e))?;
        let ck: Checkpoint =
            serde_json::from_slice(&bytes).map_err(|e| CliError::BadArgument {
                flag: "checkpoint",
                problem: format!("{} is not a v2xw checkpoint: {e}", path.display()),
            })?;
        if ck.format != FORMAT {
            return Err(CliError::BadArgument {
                flag: "checkpoint",
                problem: format!(
                    "{} is a '{}' checkpoint; this build reads '{FORMAT}'",
                    path.display(),
                    ck.format
                ),
            });
        }
        Ok(ck)
    }
}

/// The name of recording segment `i`.
#[must_use]
pub fn segment_name(i: u32) -> String {
    format!("recording.{i:04}.mcap")
}

/// What the container did with the bytes, summed over every segment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SinkTotals {
    /// Messages written.
    pub message_count: u64,
    /// Chunks closed.
    pub chunk_count: u64,
    /// The largest chunk.
    pub largest_chunk_bytes: u64,
}

/// Where a run's records go: nowhere, one recording, or a sequence of segments.
pub struct Sink {
    level: RecordLevel,
    writer: Option<RecordingWriter<std::io::BufWriter<std::fs::File>>>,
    /// `Some` when the recording is segmented.
    segmented: Option<Segments>,
    single_path: Option<PathBuf>,
    totals: SinkTotals,
    /// Counts for a run without a recording, as `NullRecorder` keeps them.
    written: u64,
    frames: u64,
}

/// What opening a segment needs.
pub struct Segments {
    /// The output directory.
    pub dir: PathBuf,
    /// The next segment's index.
    pub next: u32,
    /// The container options.
    pub opts: RecordingOptions,
    /// The manifest, written into every segment.
    pub manifest_json: String,
    /// The resolved scenario, attached to every segment.
    pub scenario_yaml: String,
    /// The world payload, attached to the first segment when asked for.
    pub world: Option<Vec<u8>>,
}

impl Sink {
    /// No recording.
    #[must_use]
    pub fn none() -> Sink {
        Sink {
            level: RecordLevel::Full,
            writer: None,
            segmented: None,
            single_path: None,
            totals: SinkTotals::default(),
            written: 0,
            frames: 0,
        }
    }

    /// One recording at `path`, already opened with its manifest and attachments.
    #[must_use]
    pub fn single(
        path: PathBuf,
        writer: RecordingWriter<std::io::BufWriter<std::fs::File>>,
        level: RecordLevel,
    ) -> Sink {
        Sink {
            level,
            writer: Some(writer),
            segmented: None,
            single_path: Some(path),
            ..Sink::none()
        }
    }

    /// A segmented recording; the first segment is opened at [`Segments::next`].
    ///
    /// # Errors
    /// As [`RecordingWriter::create`].
    pub fn segmented(segments: Segments, level: RecordLevel) -> Result<Sink> {
        let mut sink = Sink {
            level,
            segmented: Some(segments),
            ..Sink::none()
        };
        sink.open_segment()?;
        Ok(sink)
    }

    fn open_segment(&mut self) -> Result<()> {
        let Some(seg) = self.segmented.as_mut() else {
            return Ok(());
        };
        let path = seg.dir.join(segment_name(seg.next));
        let mut w = RecordingWriter::create(&path, seg.opts.clone())?;
        w.write_manifest(&seg.manifest_json)?;
        w.attach(
            "scenario.yaml",
            "application/yaml",
            seg.scenario_yaml.as_bytes(),
        )?;
        if seg.next == 0
            && let Some(world) = seg.world.as_ref()
        {
            w.attach("world.vwb", "application/octet-stream", world)?;
        }
        seg.next += 1;
        self.writer = Some(w);
        Ok(())
    }

    fn close_writer(&mut self) -> Result<()> {
        if let Some(w) = self.writer.take() {
            let s = w.finish()?;
            self.totals.message_count += s.message_count;
            self.totals.chunk_count += s.chunk_count;
            self.totals.largest_chunk_bytes = self.totals.largest_chunk_bytes.max(s.largest_chunk_bytes);
        }
        Ok(())
    }

    /// Finishes the open segment and opens the next. Returns how many segments are
    /// finished. A no-op returning 0 for an unsegmented sink.
    ///
    /// # Errors
    /// As [`RecordingWriter::finish`] and [`RecordingWriter::create`].
    pub fn roll(&mut self) -> Result<u32> {
        if self.segmented.is_none() {
            return Ok(0);
        }
        self.close_writer()?;
        self.open_segment()?;
        Ok(self.segments_done())
    }

    /// The container's totals over the segments finished so far.
    #[must_use]
    pub fn totals(&self) -> SinkTotals {
        self.totals
    }

    /// Starts the totals from what earlier segments wrote (a resumed run).
    pub fn carry_totals(&mut self, totals: SinkTotals) {
        self.totals = totals;
    }

    /// Segments finished so far.
    #[must_use]
    pub fn segments_done(&self) -> u32 {
        self.segmented
            .as_ref()
            .map_or(0, |s| s.next.saturating_sub(u32::from(self.writer.is_some())))
    }

    /// The bytes on disk of what this sink has written, for a progress line.
    #[must_use]
    pub fn bytes_on_disk(&self) -> Option<u64> {
        if let Some(p) = self.single_path.as_ref() {
            return std::fs::metadata(p).ok().map(|m| m.len());
        }
        let seg = self.segmented.as_ref()?;
        Some(
            (0..seg.next)
                .filter_map(|i| std::fs::metadata(seg.dir.join(segment_name(i))).ok())
                .map(|m| m.len())
                .sum(),
        )
    }

    /// Finishes the recording and returns the files it wrote, in order, with the totals.
    ///
    /// # Errors
    /// As [`RecordingWriter::finish`].
    pub fn finish(mut self) -> Result<(Vec<PathBuf>, Option<SinkTotals>)> {
        let had_writer = self.writer.is_some() || self.segmented.is_some();
        self.close_writer()?;
        let files = if let Some(p) = self.single_path.take() {
            vec![p]
        } else if let Some(seg) = self.segmented.as_ref() {
            (0..seg.next).map(|i| seg.dir.join(segment_name(i))).collect()
        } else {
            Vec::new()
        };
        Ok((files, had_writer.then_some(self.totals)))
    }

    /// Finishes nothing more: the open segment, which holds nothing yet (it was opened by the
    /// roll at a checkpoint taken on request), is closed and removed.
    pub fn discard_open(mut self) {
        if let (Some(w), Some(seg)) = (self.writer.take(), self.segmented.as_ref()) {
            let _ = w.finish();
            let _ = std::fs::remove_file(seg.dir.join(segment_name(seg.next.saturating_sub(1))));
        }
    }

    /// Drops the open segment without finishing it — what a crash leaves behind. Test use.
    pub fn abandon(mut self) {
        if let Some(w) = self.writer.take() {
            std::mem::forget(w);
        }
    }
}

impl RunRecorder for Sink {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        match self.writer.as_mut() {
            Some(w) => {
                if self.level.keeps(record.channel) {
                    let _ = w.write_record(at, record);
                }
            }
            None => self.written += 1,
        }
    }

    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        match self.writer.as_mut() {
            Some(w) => {
                if self.level.keeps_frames() {
                    let _ = w.write_frame(frame);
                }
            }
            None => self.frames += 1,
        }
    }
}

/// The content digest over a sequence of recordings, in order: the same
/// `SHA-256(len ‖ topic ‖ t ‖ len ‖ bytes)*` [`v2xw_record::Reader::content_digest`] takes
/// over one file, continued across files, so a segmented recording and the same messages
/// in one file digest alike. Each file is read through the paged reader, chunk by chunk.
///
/// # Errors
/// Whatever the reader returns.
pub fn content_digest(files: &[PathBuf]) -> Result<[u8; 32]> {
    let hasher = std::cell::RefCell::new(v2xw_core::Sha256Writer::new());
    let feed = |topic: &str, at: SimTime, bytes: &[u8]| {
        let mut h = hasher.borrow_mut();
        h.update(&(topic.len() as u64).to_le_bytes());
        h.update(topic.as_bytes());
        h.update(&at.to_le_bytes());
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    for f in files {
        let mut reader = v2xw_record::Reader::open_paged(f)?;
        reader.for_each_message(
            |fr| {
                feed(&fr.topic, fr.sim_time, fr.frame.as_bytes());
                Ok(())
            },
            |r| {
                feed(&r.channel, r.sim_time, &r.json);
                Ok(())
            },
        )?;
    }
    Ok(hasher.into_inner().finish())
}

/// SHA-256 of a file, read in blocks: a day's recording is not read into memory to be
/// hashed.
///
/// # Errors
/// [`CliError::Io`] if it cannot be read.
pub fn file_sha256(path: &Path) -> Result<(String, u64)> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).map_err(|e| CliError::io("cannot read", path, e))?;
    let mut h = v2xw_core::Sha256Writer::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| CliError::io("cannot read", path, e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let bytes = h.bytes_written();
    Ok((h.finish_hex(), bytes))
}

/// Cuts `path` back to its first `len` bytes and returns the SHA-256 state over them, so
/// a stream can carry on appending as if it had never stopped.
///
/// # Errors
/// [`CliError::Io`] if the file is shorter than `len` or cannot be read or cut.
pub fn truncate_and_hash(path: &Path, len: u64) -> Result<v2xw_core::Sha256Writer> {
    use std::io::Read;
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|e| CliError::io("cannot open", path, e))?;
    let have = f
        .metadata()
        .map_err(|e| CliError::io("cannot read", path, e))?
        .len();
    if have < len {
        return Err(CliError::io(
            "is shorter than its checkpoint says",
            path,
            std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("{have} bytes on disk, the checkpoint recorded {len}"),
            ),
        ));
    }
    f.set_len(len)
        .map_err(|e| CliError::io("cannot truncate", path, e))?;
    let mut h = v2xw_core::Sha256Writer::new();
    let mut r = std::io::BufReader::new(f).take(len);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = r
            .read(&mut buf)
            .map_err(|e| CliError::io("cannot read", path, e))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `--city` a run was started with survives the checkpoint, and a checkpoint written
    /// before the field existed still reads (as a run on the file's own world).
    #[test]
    fn the_city_is_recorded_and_old_checkpoints_still_read() {
        let mut ck = serde_json::json!({
            "format": FORMAT, "method": "replay", "scenario_name": "s", "scenario_hash": "h",
            "world_hash": "w", "build_utc": "2026-10-07T00:00:00Z", "scenario_path": "s.yaml",
            "duration_override_s": null, "rate_override_veh_per_h": null, "cwd": ".",
            "keyframe_ms": 1000, "node_only": false, "record": true, "record_level": "full",
            "attach_world": false, "verify": true, "every_s": 30.0, "progress_every_s": null,
            "t_ns": 0, "record_digest": "", "records": 0, "metric_samples": 0,
            "metrics_bytes": 0, "segments_done": 0, "progress_bytes": 0, "wall_s": 0.0,
            "resumes": 0
        });
        let old: Checkpoint = serde_json::from_value(ck.clone()).expect("an old checkpoint reads");
        assert_eq!(old.city, None);
        ck["city"] = serde_json::json!("houston");
        let new: Checkpoint = serde_json::from_value(ck).expect("reads");
        assert_eq!(new.city.as_deref(), Some("houston"));
        let back: Checkpoint =
            serde_json::from_slice(&serde_json::to_vec(&new).expect("writes")).expect("reads");
        assert_eq!(back, new);
    }
}
