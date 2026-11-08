//! A table export's memory does not scale with the recording.
//!
//! The server runs a scenario's exporters after the run, in the same process as the run's
//! own state. A five-minute Manhattan run with `exporters: [{id: jsonl}]` records about
//! 6 GB of reception JSON, and `run_exporters_in` used to read every record of the
//! recording into one list and then copy each channel's rows out of it before writing:
//! the server's footprint went from 0.5 GB at the end of the run to 18 GB on an 8 GB
//! machine. It now streams the recording.
//!
//! This binary counts the bytes the global allocator holds and requires the export's
//! peak to stay a small fraction of the recording's record JSON, which the old code
//! could not do (it held all of it at once). It also requires the streamed files to be
//! the bytes the in-memory path writes, so the fix does not move an output.
//!
//! One test per binary on purpose: the allocator counts every thread, so a second test
//! running beside this one would be measured with it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use v2xw_core::{OwnedRecord, Visibility};
use v2xw_engine::export::run_exporters_in;
use v2xw_engine::scenario::ExporterSpec;
use v2xw_record::export::{ExportFormat, ExportProfile, Exporter};
use v2xw_record::fixture::{RunShape, live_frames, scratch_dir};
use v2xw_record::{Reader, RecordingOptions, RecordingWriter};

/// The system allocator, counting the bytes it holds and the most it has held.
struct Counting;

static HELD: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn note_alloc(size: usize) {
    let now = HELD.fetch_add(size, Ordering::SeqCst) + size;
    PEAK.fetch_max(now, Ordering::SeqCst);
}

// SAFETY: every call forwards to `System` unchanged; the counters are bookkeeping only.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract, passed through.
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            note_alloc(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `alloc` above with this `layout`.
        unsafe { System.dealloc(ptr, layout) };
        HELD.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract, passed through.
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            HELD.fetch_sub(layout.size(), Ordering::SeqCst);
            note_alloc(new_size);
        }
        p
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn a_jsonl_export_streams_the_recording_instead_of_holding_it() {
    let dir = scratch_dir("export-memory").expect("a scratch directory");
    // A live stream's frames with many reception and transmission records between them,
    // as the engine writes them: the reception channel is what made the real export large,
    // and the frames are what give the recording its chunk boundaries (a chunk only closes
    // at a frame). Small chunks, so one chunk is a small part of the whole.
    let shape = RunShape::new(4, 600);
    let frames = live_frames(&shape).expect("the fixture's frames");
    let path = dir.join("run.mcap");
    let opts = RecordingOptions {
        cadence: shape.cadence,
        profile: shape.profile,
        chunk_target_bytes: 256 << 10,
        ..Default::default()
    };
    let mut writer = RecordingWriter::create(&path, opts).expect("the recording opens");
    let per_frame = 60_000 / frames.len() as u64;
    let mut t = 0;
    for k in 0..per_frame * frames.len() as u64 {
        if k % per_frame == 0 {
            let frame = &frames[(k / per_frame) as usize];
            writer.write_frame(frame).expect("the frame is written");
            t = frame.sim_time().expect("a timed frame").max(t);
        }
        let rx = serde_json::json!({
            "t_start": t, "t_end": t, "rx": k % 97, "tx": k % 89, "msg": k,
            "rssi_dbm": -78.12, "sinr_db": 12.99, "dist_m": 123.457,
            "outcome": "ok", "candidate": true,
        });
        let tx = serde_json::json!({
            "node_id": k % 89, "msg_id": k, "bytes_on_air": 320,
            "airtime_ms": 0.526, "msg_type": 2, "tx_power_cdbm": 2000,
        });
        for (channel, visibility, value) in [
            ("phy.rx", Visibility::NodeAndGt, rx),
            ("node.tx", Visibility::Node, tx),
        ] {
            let record = OwnedRecord {
                channel,
                visibility,
                json: serde_json::to_vec(&value).expect("JSON"),
            };
            writer
                .write_record(t, &record)
                .expect("the record is written");
        }
    }
    writer.finish().expect("the recording closes");

    // How much record JSON the recording holds, read the way the old exporter read it.
    let json_bytes: usize = {
        let mut reader = Reader::open(&path).expect("the recording opens");
        let records = reader.records(None).expect("the records come back");
        records.iter().map(|r| r.json.len()).sum()
    };
    assert!(
        json_bytes > 8 << 20,
        "the fixture must be big enough for a whole-recording copy to stand out: \
         {json_bytes} bytes of record JSON"
    );

    let streamed = dir.join("streamed");
    let spec = ExporterSpec {
        id: "jsonl".to_string(),
        opts: serde_json::Value::Null,
    };
    let before = HELD.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let exported = run_exporters_in(std::slice::from_ref(&spec), &path, &streamed, None)
        .expect("the export succeeds");
    let peak = PEAK.load(Ordering::SeqCst).saturating_sub(before);
    assert!(
        peak < json_bytes / 4,
        "the export held {peak} bytes at its peak for a recording of {json_bytes} bytes of \
         record JSON: it is holding the recording, not streaming it"
    );

    // The same files, byte for byte, as the in-memory path writes.
    let reference = dir.join("reference");
    let records = Reader::open(&path)
        .expect("the recording opens")
        .records(None)
        .expect("the records come back");
    let files = Exporter::new(reference.join("jsonl"), ExportProfile::Full)
        .expect("an exporter")
        .export_all(&records, ExportFormat::Jsonl)
        .expect("the reference export succeeds");
    let got = &exported[0].files;
    assert_eq!(
        got.len(),
        files.len(),
        "one data file and one sidecar per channel"
    );
    for (g, want) in got.iter().zip(&files) {
        assert_eq!(g.path.file_name(), want.path.file_name());
        assert_eq!(g.rows, want.rows, "{:?}", want.path);
        let (a, b) = (
            std::fs::read(&g.path).expect("streamed file"),
            std::fs::read(&want.path).expect("reference file"),
        );
        assert!(a == b, "{:?} differs from the in-memory export", want.path);
    }
    assert!(files.iter().any(|f| f.rows > 0), "the export wrote rows");
    let _ = std::fs::remove_dir_all(&dir);
}
