# Recording format

A run's outputs and their on-disk format, for someone writing a reader, an exporter or a
tool over recordings. The user-level view (what each file is for, the channel list by
visibility, the record levels) is in [Metrics and outputs](../recording-and-metrics.md);
the binary frame layouts are normative in [VWP v1](../protocol/vwp-v1.md). Code:
`crates/v2xw-record` (container, writer, reader, index, exporters),
`crates/v2xw-core/src/manifest.rs` and `crates/v2xw-engine/src/manifest.rs` (manifest),
`crates/v2xw-cli/src/run.rs` and `checkpoint.rs` (run directory, checkpoints).

## The run directory

`v2xw run <scenario> --out <dir>` writes:

| File | Content | Reproducible |
|---|---|---|
| `recording.mcap` | the MCAP container: every record, the manifest as metadata, the resolved scenario as the attachment `scenario.yaml` (and the world payload as `world.vwb` with `--attach-world`) | the data section is; the file's SHA-256 is not ([digests](#digests)) |
| `scenario.resolved.yaml` | the scenario after migration, base merge and defaults: what ran | yes |
| `metrics.json` | every `metric.sample` record | yes |
| `run-report.json` | event, frame and record counts per channel, and the recording's `content_digest` | yes |
| `manifest.json` | the run manifest with the files above digested and `data_digest` finalised | all but `build_utc` and the recording file's own digest |
| `progress.jsonl` | progress lines (`--progress-every`) | no (wall-clock timings) |
| `checkpoint.json` | a resumable checkpoint (`--checkpoint-every`) | see [checkpoints](#checkpoints) |

Exporters named in the scenario's `exporters` list add their own files beside these
([Datasets and exports](../guide/datasets.md)).

## The MCAP container

[MCAP](https://mcap.dev) with zstd chunks, profile `v2xw`, readable by standard MCAP tools.
Two topic namespaces with different encodings, never mixed on one topic (the writer
refuses):

| Namespace | Topics | Message encoding | Schema | Holds |
|---|---|---|---|---|
| `vwp/…` | `vwp/hello`, `vwp/keyframe`, `vwp/delta`, `vwp/telemetry`, `vwp/metric`, `vwp/provenance`, `vwp/event/<channel>` | `vwp1` | `vwp.v1.<Type>`; the schema data is the layout table from the specification, so the file describes itself | VWP binary frames stored **verbatim** (transport flag bits cleared) |
| `record/…` | `record/<channel>`, one per channel family | `json` | `jsonschema`: the channel, its visibility and the grid of every float | serde `v2xw_core::OwnedRecord` rows, the input to the Parquet, Arrow IPC and JSONL exporters |

Storing the snapshot frames as the bytes that went over the wire is what makes replay
byte-identical to the live stream ([VWP §7.2](../protocol/vwp-v1.md#72-byte-identity-of-live-and-replay)):
`RecordingWriter::write_frame` stores them, `Reader::replay` hands them back, and nothing
rebuilds a frame from decoded state. `crates/v2xw-record/tests/byte_identity.rs` checks it.

### Channel metadata

Every MCAP channel carries:

| Key | Value |
|---|---|
| `channel` | the record channel name (`node.tx`, ...) |
| `visibility` | `gt`, `node`, `node-and-gt`, `public`, `derived`, `mixed` or `meta` |
| `ground_truth` | `true` when any part of the record is ground truth, so a consumer can filter without the catalogue |
| `encoding_kind` | `vwp1` or `record` |
| `vwp_channel_id`, `vwp_msg_type` | the numeric ids on `vwp/…` topics |

File-level metadata `v2xw.manifest` holds the manifest JSON.

### The channel catalogue

`v2xw_record::channels::CHANNELS` is one table for the recorder (topic, schema,
metadata), the NODE-only profile (which channels to strip) and the exporters (which tables
to write). Each `ChannelSpec` has a `name`, a `wire_id` (its id in an `Event` frame, or
`None` for a record-only channel), a `visibility` and, for `Event` payloads, a fixed
`payload_bytes`. 49 channels; with a VWP `Event` wire id:

| Wire id | Channel | Visibility | Event payload (bytes) |
|---|---|---|---|
| 1 | `gt.kinematics` | gt | [§3.6.11](../protocol/vwp-v1.md#3611-gtkinematics-channel-1-56-bytes-gt) |
| 2 | `gt.attack.action` | gt | [§3.6.12](../protocol/vwp-v1.md#3612-gtattackaction-channel-2-32-bytes-gt) |
| 3, 4 | `gt.spawn`, `gt.despawn` | gt | |
| 10 | `node.tx` | node | [§3.6.4](../protocol/vwp-v1.md#364-nodetx-channel-10-40-bytes) |
| 11 | `phy.rx` | node-and-gt | [§3.6.5](../protocol/vwp-v1.md#365-phyrx-channel-11-48-bytes) |
| 12 | `mac.cbr` | node | [§3.6.13](../protocol/vwp-v1.md#3613-maccbr-channel-12-16-bytes) |
| 13 | `net.frag` | node | [§3.6.14](../protocol/vwp-v1.md#3614-netfrag-channel-13-24-bytes) |
| 14 | `node.verify` | node | [§3.6.6](../protocol/vwp-v1.md#366-nodeverify-channel-14-48-bytes) |
| 15 | `node.telemetry` | node | |
| 16 | `node.neighbor` | node | [§3.6.15](../protocol/vwp-v1.md#3615-nodeneighbor-channel-16-32-bytes) |
| 20 | `sec.cert` | node | [§3.6.7](../protocol/vwp-v1.md#367-seccert-channel-20-40-bytes) |
| 21 | `proto.msg` | node | [§3.6.16](../protocol/vwp-v1.md#3616-protomsg-channel-21-32-bytes) |
| 22 | `proto.revocation` | public | [§3.6.10](../protocol/vwp-v1.md#3610-protorevocation-channel-22-32-bytes-public) |
| 30 | `det.observation` | node | [§3.6.8](../protocol/vwp-v1.md#368-detobservation-channel-30-32-bytes) |
| 31, 32, 33 | `ma.report`, `ma.case`, `ma.decision` | node | [§3.6.17](../protocol/vwp-v1.md#3617-mareport-macase-madecision-channels-313233-40-bytes) |
| 40 | `app.warning` | node | [§3.6.9](../protocol/vwp-v1.md#369-appwarning-channel-40-32-bytes) |
| 50 | `metric.sample` | derived | [§3.7](../protocol/vwp-v1.md#37-metricsample-0x0006) |
| 60, 61 | `snapshot.keyframe`, `snapshot.delta` | mixed | `Keyframe` / `Delta` frames ([§3.3](../protocol/vwp-v1.md#33-keyframe-0x0002), [§3.4](../protocol/vwp-v1.md#34-delta-0x0003)) |
| 70 | `manifest` | meta | |

The other channels are record-only: `app.advice`, `app.outcome`, `backend.state`,
`gt.device_exposure`, `gt.subject`, `ma.received`, `msg.latency`, `net.bytes`,
`net.reassembly`, `node.credentials`, `node.drop`, `node.p2pcd`, `node.rx`,
`node.security`, `phy.prr`, `pki.case`, `pki.flow`, `pki.knowledge`, `pki.report_fate`,
`pki.state`, `privacy.link`, `privacy.track`, `scenario.event`, `sec.pseudonym`,
`signal.priority`, `traffic.control`.

## Encoders and quantisation

No float reaches a recorded, exported or digested artefact in raw form: one writer-side
encoder quantises each value to its declared grid, and a scan fails the build if a value
sits off it.

| Quantity | Wire | Scale | Reference |
|---|---|---|---|
| keyframe x, y | `i32` | millimetres | the run's origin |
| keyframe z | `i16` | centimetres | the run's origin |
| delta x, y, z | `i16` | millimetres | the **previously transmitted quantised** value, so error does not accumulate |
| heading | `u16` | binary radians | absolute |
| speed | `i16` | 1/128 m/s | absolute |
| acceleration | `i16` | 1/64 m/s² | absolute |

(`v2xw_record::quant`; [VWP §3.2](../protocol/vwp-v1.md#32-pose-quantisation-normative-used-by-keyframe-and-delta).)
The remaining floats (the `Hello` origin and bounding box, `f32` node positions and class
dimensions, ten `Telemetry` fields, `MetricSample.value`) have grids listed in
`v2xw_record::grid`, which `scan_frame` enforces. Record channels quantise through
`v2xw_core::math::quantize_to` in the record constructors; the exporters quantise each
column to the grid its schema declares and write the grids beside the data as
`schema.json`, which `export::scan` reads back.

The snapshot producer (`v2xw_record::encoder::SnapshotEncoder`) is a pure function of the
snapshots it is fed. Its `Cadence { keyframe_period, mobility_step }` comes from the
scenario (`v2xw run --keyframe-ms`, default 1000 ms, a whole number of mobility steps);
`Cadence::DEFAULT` is 1 s keyframes and 100 ms steps. One delta per mobility step; only
actors whose quantised pose, state, verified neighbours or lane changed appear in it.

## Chunks, the index and seek

- Chunk boundaries fall **before a keyframe** once the chunk reaches
  `chunk_target_bytes`; a GOP larger than the target is split between deltas. A chunk is
  therefore bounded by about twice the target plus one message, whatever the keyframe
  period. `RecordingSummary::largest_chunk_bytes` and `largest_gop_bytes` report what a
  run reached.
- zstd runs single-threaded (`compression_threads = 0`): the multithreaded encoder is not
  byte-deterministic.
- `v2xw_record::index::SeekIndex` reads the 20-byte footer, the summary section and each
  chunk's message index, then fetches only the chunks a seek needs. Every offset and length
  from the file is checked arithmetic, bounded by the buffer it describes and cross-checked
  against the chunk index; a chunk's CRC is verified before anything in it is parsed.
  Malformed input is a named `RecordError`, never a panic (`tests/overflow.rs` in debug
  and release, `tests/fuzz_corrupt.rs`).
- `Reader::seek(t)` finds the keyframe at or before `t` and applies the deltas up to it
  ([VWP §7.3](../protocol/vwp-v1.md#73-the-seek-algorithm)); the budget is 100 ms
  ([§7.4](../protocol/vwp-v1.md#74-the-100-ms-seek-budget)).

## Visibility profiles

`v2xw_record::profile` implements the NODE-only profile
([VWP §5](../protocol/vwp-v1.md#5-visibility-and-the-node-only-profile)): ground-truth
channels are dropped whole; a `mixed` or `node-and-gt` record keeps its node-visible
fields with the ground-truth ones blanked. An unequipped actor transmits nothing, so its
slot is empty; an equipped actor keeps its pose. The live producer and the stripper share
one blanking implementation, so a `full` recording replayed as `node` equals a live `node`
run byte for byte. `v2xw run --node-only` records the profile; `ExportProfile::NodeOnly`
exports it, and a NODE export and a ground-truth export never share a file.

## The manifest

`v2xw_core::manifest::Manifest`, written as `manifest.json` and as the `v2xw.manifest`
metadata record:

| Field | Source |
|---|---|
| `schema` | the manifest's own schema id |
| `scenario_hash` | SHA-256 of the scenario's canonical JSON (`Scenario::content_hash`) |
| `master_seed` | `scenario.seed` |
| `engine_version`, `git_commit`, `platform`, `crate_versions` (including `rustc`) | the build |
| `build_hash` | SHA-256 over engine version, compiler version, platform triple, git commit, clean or dirty tree, and every workspace crate version, sorted |
| `build_utc` | supplied by the caller (`--build-utc`, or the CLI's clock); the only real-world instant, excluded from every digest |
| `plugins` | every registry entry: id, card version, card content hash |
| `model_cards` | `(id, version)` of every registered card |
| `world_hash` | the world's canonical content hash |
| `sumo_version` | when SUMO drove mobility |
| `crypto_mode` | `modeled` or `real` |
| `time_dilation_windows` | windows where radio-dependent metrics are marked not observed |
| `warnings` | validator warnings |
| `files` | `FileDigest` per output file |
| `data_digest` | the aggregate digest below |

## Digests

| Digest | Over | Where | Compare it? |
|---|---|---|---|
| `content_digest` | SHA-256 over every message's topic, `log_time` and bytes in stored order (`Reader::content_digest`) | `run-report.json` | **yes**: a run's identity; two runs of one scenario, seed and build must agree |
| file SHA-256 of `recording.mcap` | the bytes on disk | `manifest.json` `files` | no: the `mcap` writer emits summary records from a hash map, so the index order can vary |
| `data_digest` | `SHA-256(for rel in sorted(paths): rel ‖ hex(sha256(file)))`, hex text of each file digest, no separator (`Manifest::finalize`) | `manifest.json` | yes, for the reproducible files |
| `stream_digest` | SHA-256 of every record as produced, nothing written (`--no-recording --digest`) | CLI output | for scaling runs at different thread counts |
| golden records | scenario hash, world hash, a digest over every record, per-channel counts | `tests/conformance/golden/*.json` | the conformance kit ([Conventions](conventions.md#golden-digests)) |
| metric digest | over the run summary's metrics; `diagnostic` metrics are kept apart and excluded | `v2xw_metrics::RunSummary::digest` | yes |

## Checkpoints

`v2xw run --checkpoint-every 1h` writes `checkpoint.json` and closes recording segments at
each checkpoint. It holds the inputs (resolved scenario, its hash, world hash, build
timestamp), the instant `t`, the **record digest** at `t` with the record count, and how
much of each output was complete at `t` (closed segments, `metrics.json` bytes and samples,
`progress.jsonl` lines).

`v2xw resume <checkpoint.json>` rebuilds the engine, re-runs to `t` with outputs off,
**refuses to continue unless the record digest at `t` matches**, truncates the outputs to
what was complete at `t`, and carries on. The engine's memory is not serialised, so a
resume costs the re-run to `t`. The finished outputs equal an uninterrupted run's
(`crates/v2xw-cli/tests/longrun.rs`). `v2xw-server --resume
<checkpoint.json>` serves such a run paused at its checkpoint.

## Reading a recording

=== "Rust"

    ```rust
    use v2xw_record::reader::Reader;

    let mut r = Reader::open("runs/x/recording.mcap")?;     // or open_paged / open_bytes
    r.require_chunk_checksums(true);                        // refuse chunks without a CRC
    let report = r.verify()?;                               // VerifyReport
    assert!(report.integrity_verified());
    let digest = r.content_digest()?;                       // [u8; 32]
    let manifest = r.manifest_json();                       // Option<&str>
    for ch in r.channels() { /* ChannelMeta { id, topic, message_encoding, schema_id, metadata } */ }
    let rows = r.records(Some("node.tx"))?;                 // Vec<RecordedRecord>
    let frames = r.replay()?;                               // Vec<RecordedFrame>, byte-identical
    let at = r.seek(30_000_000_000)?;                       // SeekResult at t = 30 s
    let scenario = r.attachment("scenario.yaml")?;          // the resolved scenario
    ```

    `Reader::open` reads the file into memory; `open_paged` reads only the footer,
    summary and message indexes and fetches the chunks a seek needs.
    `VerifyReport` counts frames, records, keyframes, deltas, unknown message types
    (counted, not refused), provenance ids and references, the snapshot span and its
    largest gap, and chunks with and without a checksum. Tabular export:
    `v2xw_record::export` (Parquet, Arrow IPC, JSONL).

=== "Python"

    ```python
    import v2xw

    rec = v2xw.Recording.open("runs/x/recording.mcap")
    rec.require_chunk_checksums(True)
    report = rec.verify()                       # dict
    digest = rec.content_digest()               # hex
    rec.manifest, rec.channels, rec.ground_truth_channels
    rows = rec.records("node.tx", limit=100)    # list of dicts
    batch = rec.arrow("node.tx")                # pyarrow.RecordBatch; ground_truth=False drops GT columns
    ipc = rec.table("node.tx")                  # Arrow IPC bytes; v2xw.read_ipc(ipc)
    rec.table_schema("node.tx")
    ```

    See [Python API](../api/python.md#recording).

=== "WebAssembly"

    `crates/v2xw-wasm` compiles the same reader for `wasm32-unknown-unknown` and adds a
    range-fetched byte source (`ranges`), resolved pose columns at an instant (`scene`) and
    the seek loop the JavaScript class calls (`session`). The Studio replays recordings
    in the browser this way with no replay-specific code path. Building it needs a
    `clang` with the WebAssembly back end (the container's zstd is C); see
    `crates/v2xw-wasm/README.md` and `scripts/build-wasm.sh`.

=== "Command line"

    ```sh
    v2xw info runs/x/recording.mcap      # manifest, channels, verification report
    ```

    Any MCAP tool (the `mcap` CLI, Foxglove) lists topics and messages; `vwp/…` payloads
    decode with the layout tables in their channel schema or in [VWP v1](../protocol/vwp-v1.md).
