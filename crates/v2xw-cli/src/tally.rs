//! A [`RunRecorder`] that counts what goes past it on the way to another one.
//!
//! [`v2xw_engine::RunReport`] answers "how big was the run?" in events, frames and
//! records; it does not answer "what is *in* the recording?". A researcher asking whether
//! a scenario produced any signed transmissions at all wants the per-channel breakdown,
//! and reading it back out of the MCAP afterwards costs a second pass over the file for a
//! number the writer already had in its hands.
//!
//! [`Tally`] therefore sits in front of the real recorder and counts. It never filters,
//! never reorders and never rewrites, so wrapping a recorder cannot change what a run
//! records — a property `tests/tally.rs` asserts by digesting a run with and without it.

use std::collections::BTreeMap;

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::RunRecorder;

/// What one channel carried.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct ChannelTally {
    /// How many records landed on it.
    pub records: u64,
    /// How many bytes of JSON those records carried, before compression.
    pub json_bytes: u64,
    /// The first instant one was written at, in nanoseconds.
    pub first_ns: u64,
    /// The last instant one was written at, in nanoseconds.
    pub last_ns: u64,
}

/// A recorder that counts and forwards.
#[derive(Debug)]
pub struct Tally<R: RunRecorder> {
    /// Binary frames forwarded to the inner recorder.
    frames: u64,
    inner: R,
    by_channel: BTreeMap<String, ChannelTally>,
    /// Where every `metric.sample` record goes, whole: a run's metrics are the one output
    /// a caller wants next to the recording rather than inside it. Streamed to
    /// `metrics.json` as they arrive ([`crate::stream::MetricsStream`]) — they were a
    /// `Vec` held for the whole run, which a day-long run could not afford.
    metrics: Option<crate::stream::MetricsStream>,
    metric_count: u64,
    /// The record digest over every record that went past, in order — the same
    /// `SHA-256(t ‖ channel ‖ visibility ‖ json)*` [`v2xw_engine::DigestRecorder`] computes.
    /// It is taken in front of any filter, so it names what the engine emitted whatever
    /// reached the recording, and a resumed run is checked against it.
    digest: v2xw_core::hash::ThreadedSha256,
    records: u64,
}

impl<R: RunRecorder> Tally<R> {
    /// Wraps a recorder.
    pub fn new(inner: R) -> Self {
        Tally {
            inner,
            by_channel: BTreeMap::new(),
            metrics: None,
            metric_count: 0,
            digest: v2xw_core::hash::ThreadedSha256::new(),
            records: 0,
            frames: 0,
        }
    }

    /// How many binary frames were forwarded to the inner recorder.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The per-channel counts, in channel-name order.
    pub fn by_channel(&self) -> &BTreeMap<String, ChannelTally> {
        &self.by_channel
    }

    /// Streams every metric sample to `metrics` from now on.
    pub fn stream_metrics(&mut self, metrics: crate::stream::MetricsStream) {
        self.metrics = Some(metrics);
    }

    /// The metrics stream, to flush at a checkpoint.
    pub fn metrics_mut(&mut self) -> Option<&mut crate::stream::MetricsStream> {
        self.metrics.as_mut()
    }

    /// Hands the metrics stream back, to be finished.
    pub fn take_metrics(&mut self) -> Option<crate::stream::MetricsStream> {
        self.metrics.take()
    }

    /// How many metric samples went past.
    pub fn metric_count(&self) -> u64 {
        self.metric_count
    }

    /// The record digest so far, hex.
    pub fn record_digest_hex(&self) -> String {
        self.digest.finish_hex()
    }

    /// How many records went past, over every channel.
    pub fn records(&self) -> u64 {
        self.records
    }

    /// The wrapped recorder, for calls that are not record writes — writing the manifest
    /// metadata and adding an attachment, which belong to the container and not to the
    /// [`RunRecorder`] contract.
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Gives the wrapped recorder back, so it can be finished.
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// The same counts and digest in front of a different recorder, and the old recorder
    /// back: how a resumed run carries on from its silent replay into its real outputs
    /// without the digest or the per-channel counts starting again.
    pub fn rewrap<R2: RunRecorder>(self, inner: R2) -> (Tally<R2>, R) {
        (
            Tally {
                frames: self.frames,
                inner,
                by_channel: self.by_channel,
                metrics: self.metrics,
                metric_count: self.metric_count,
                digest: self.digest,
                records: self.records,
            },
            self.inner,
        )
    }
}

impl<R: RunRecorder> RunRecorder for Tally<R> {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        // Looked up by `&str` first: a run writes millions of records into a few dozen
        // channels, and an owned key per record was an allocation per record.
        if !self.by_channel.contains_key(record.channel) {
            self.by_channel.insert(
                record.channel.to_string(),
                ChannelTally {
                    first_ns: at,
                    ..ChannelTally::default()
                },
            );
        }
        let e = self
            .by_channel
            .get_mut(record.channel)
            .expect("inserted above");
        e.records += 1;
        e.json_bytes += record.json.len() as u64;
        e.last_ns = at;
        self.records += 1;
        self.digest.update(&at.to_le_bytes());
        self.digest.update(record.channel.as_bytes());
        // The same bytes `to_string()` gave, written straight into the hash.
        let _ =
            core::fmt::Write::write_fmt(&mut self.digest, format_args!("{}", record.visibility));
        self.digest.update(&record.json);
        if record.channel == "metric.sample" {
            self.metric_count += 1;
            if let Some(m) = self.metrics.as_mut()
                && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&record.json)
            {
                m.push(&v);
            }
        }
        self.inner.write(at, record);
    }

    // Forwarded, not defaulted. `RunRecorder::write_wire_frame` discards by default so a
    // record-only recorder need not know the binary path exists, but a WRAPPER that
    // forwards `write` and not this one silently drops the normative binary stream and
    // nothing in the resulting file says so. That is exactly what happened here: every
    // run wrote zero keyframes and zero deltas while reporting success.
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.frames += 1;
        self.inner.write_wire_frame(frame);
    }

    fn refused(&self) -> u64 {
        self.inner.refused()
    }

    fn cancelled(&self) -> bool {
        self.inner.cancelled()
    }
}
