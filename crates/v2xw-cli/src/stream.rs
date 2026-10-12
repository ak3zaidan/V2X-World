//! What keeps a long run's outputs on disk instead of in memory.
//!
//! * [`MetricsStream`] writes `metrics.json` sample by sample as the run emits them. It was
//!   a `Vec<serde_json::Value>` held for the whole run and serialised at the end: about a
//!   kilobyte of heap per sample, so a day of `metrics: [all]` at a one-second period held
//!   gigabytes before writing a byte. The bytes written are exactly what
//!   `serde_json::to_string_pretty(&json!({"samples": samples}))` produced, so the file and
//!   its digest are unchanged for any run that fitted in memory before.
//! * [`RecordLevel`] decides which records reach the recording: everything, or
//!   only the metrics and the run's summaries, for a run of days whose full recording
//!   would be too large to use.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use v2xw_core::hash::Sha256Writer;

use crate::error::{CliError, Result};

/// Writes `metrics.json` as the samples arrive, and digests what it wrote.
pub struct MetricsStream {
    path: PathBuf,
    file: BufWriter<std::fs::File>,
    hasher: Sha256Writer,
    count: u64,
    failed: Option<std::io::Error>,
}

impl std::fmt::Debug for MetricsStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MetricsStream")
            .field("path", &self.path)
            .field("count", &self.count)
            .field("bytes", &self.hasher.bytes_written())
            .finish_non_exhaustive()
    }
}

impl MetricsStream {
    /// Creates the file and writes its opening.
    ///
    /// # Errors
    /// [`CliError::Io`] if the file cannot be created.
    pub fn create(path: &Path) -> Result<MetricsStream> {
        let file =
            std::fs::File::create(path).map_err(|e| CliError::io("cannot write", path, e))?;
        let mut s = MetricsStream {
            path: path.to_path_buf(),
            file: BufWriter::with_capacity(1 << 16, file),
            hasher: Sha256Writer::new(),
            count: 0,
            failed: None,
        };
        s.put(b"{\n  \"samples\": [");
        Ok(s)
    }

    /// Carries on a stream a checkpoint stopped: the file is cut back to the `bytes` that
    /// were complete when it was taken, holding `count` samples, and appended to.
    ///
    /// # Errors
    /// [`CliError::Io`] if the file is shorter than `bytes` or cannot be reopened.
    pub fn resume(path: &Path, bytes: u64, count: u64) -> Result<MetricsStream> {
        let hasher = crate::checkpoint::truncate_and_hash(path, bytes)?;
        let file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| CliError::io("cannot reopen", path, e))?;
        Ok(MetricsStream {
            path: path.to_path_buf(),
            file: BufWriter::with_capacity(1 << 16, file),
            hasher,
            count,
            failed: None,
        })
    }

    /// Flushes what has been written and returns how many bytes the file holds: the
    /// length a checkpoint records.
    ///
    /// # Errors
    /// [`CliError::Io`] if a write failed.
    pub fn flush(&mut self) -> Result<u64> {
        if let Some(e) = self.failed.take() {
            return Err(CliError::io("cannot write", &self.path, e));
        }
        self.file
            .flush()
            .map_err(|e| CliError::io("cannot write", &self.path, e))?;
        Ok(self.hasher.bytes_written())
    }

    fn put(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
        if self.failed.is_none()
            && let Err(e) = self.file.write_all(bytes)
        {
            self.failed = Some(e);
        }
    }

    /// Appends one sample, indented as the pretty printer indents an array element two
    /// levels down.
    pub fn push(&mut self, sample: &serde_json::Value) {
        let text = serde_json::to_string_pretty(sample).unwrap_or_else(|_| "null".to_string());
        self.put(if self.count == 0 { b"\n" } else { b",\n" });
        let mut first = true;
        for line in text.split('\n') {
            if !first {
                self.put(b"\n");
            }
            first = false;
            self.put(b"    ");
            self.put(line.as_bytes());
        }
        self.count += 1;
    }

    /// How many samples were written.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Closes the document and returns the file's SHA-256 (hex) and sample count.
    ///
    /// # Errors
    /// [`CliError::Io`] if any write failed.
    pub fn finish(mut self) -> Result<(String, u64)> {
        if self.count == 0 {
            self.put(b"]\n}");
        } else {
            self.put(b"\n  ]\n}");
        }
        if let Some(e) = self.failed.take() {
            return Err(CliError::io("cannot write", &self.path, e));
        }
        self.file
            .flush()
            .map_err(|e| CliError::io("cannot write", &self.path, e))?;
        Ok((self.hasher.finish_hex(), self.count))
    }
}

/// How much of a run reaches its recording.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum RecordLevel {
    /// Every record and the binary snapshot stream: what the page replays.
    #[default]
    Full,
    /// Every record, but no snapshot frames and no per-step kinematics: every reception
    /// included. About a third of a full recording's size on the soak scenarios.
    Receptions,
    /// What happened, without the scene and without the per-reception journals
    /// (`node.rx`, `phy.rx`, `node.verify`): one record per receiver per frame grows with
    /// the square of the fleet, and in a 30-vehicle attack run those three were 95 % of
    /// the bytes (a 300 s run at the old `events` wrote 6.9 GB). Transmissions, detector
    /// verdicts, attack actions, backend and security records, and every metric sample are
    /// kept; the metrics are computed in the engine from the full stream either way.
    Events,
    /// Only the metric samples and the run's summary records — scenario events, the
    /// backend's decisions and revocations, the credential system's milestones. A day of
    /// this is megabytes, and it is what a long run's charts and summary read.
    Metrics,
}

impl RecordLevel {
    /// Whether a record on `channel` is kept at this level.
    #[must_use]
    pub fn keeps(self, channel: &str) -> bool {
        match self {
            RecordLevel::Full => true,
            RecordLevel::Receptions => channel != "gt.kinematics",
            RecordLevel::Events => !matches!(
                channel,
                "gt.kinematics" | "node.rx" | "phy.rx" | "node.verify"
            ),
            RecordLevel::Metrics => matches!(
                channel,
                "metric.sample"
                    | "manifest"
                    | "scenario.event"
                    | "ma.decision"
                    | "ma.case"
                    | "proto.revocation"
                    | "gt.spawn"
                    | "gt.despawn"
            ),
        }
    }

    /// Whether the binary snapshot stream is kept at this level.
    #[must_use]
    pub fn keeps_frames(self) -> bool {
        self == RecordLevel::Full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `events` leaves out the per-reception journals and keeps what an attack study
    /// reads; `receptions` keeps them.
    #[test]
    fn events_drops_the_reception_journals_and_receptions_keeps_them() {
        for ch in ["node.rx", "phy.rx", "node.verify"] {
            assert!(!RecordLevel::Events.keeps(ch), "{ch}");
            assert!(RecordLevel::Receptions.keeps(ch), "{ch}");
        }
        for ch in [
            "node.tx",
            "gt.attack.action",
            "ma.decision",
            "metric.sample",
        ] {
            assert!(RecordLevel::Events.keeps(ch), "{ch}");
        }
        assert!(!RecordLevel::Receptions.keeps("gt.kinematics"));
    }

    /// The streamed file is byte-for-byte what the whole-document pretty printer wrote,
    /// for none, one and several samples with nesting.
    #[test]
    fn the_stream_writes_what_the_pretty_printer_wrote() {
        let dir = std::env::temp_dir().join(format!("v2xw-metrics-stream-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let samples = [
            serde_json::json!({"metric": "a", "t": 1.5, "value": {"mean": 2, "ci": [1, 3]}, "s": "x\ny"}),
            serde_json::json!({"metric": "b", "t": 2.0, "value": null, "tags": []}),
            serde_json::json!({"metric": "c", "t": 3.0, "value": {}}),
        ];
        for n in 0..=samples.len() {
            let path = dir.join(format!("m{n}.json"));
            let mut s = MetricsStream::create(&path).unwrap();
            for v in &samples[..n] {
                s.push(v);
            }
            let (sha, count) = s.finish().unwrap();
            let want = serde_json::to_string_pretty(&serde_json::json!({"samples": samples[..n]}))
                .unwrap();
            let got = std::fs::read_to_string(&path).unwrap();
            assert_eq!(got, want, "{n} samples");
            assert_eq!(sha, v2xw_core::hash::sha256_hex(want.as_bytes()));
            assert_eq!(count, n as u64);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
