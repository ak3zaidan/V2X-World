# Datasets and exports

The recording is the run's complete record. Exporters turn it into files other tools read:
columnar tables, VeReMi-style misbehaviour traces and SUMO output files. They run **after**
the run and **over the recording**, never inside the event loop, so an export cannot
disagree with the recording beside it. Code: `crates/v2xw-engine/src/export.rs`,
`crates/v2xw-engine/src/sumo_outputs.rs`, `crates/v2xw-record/src/export/` and
`crates/v2xw-record/src/dataset/`.

## Naming exporters in a scenario

```yaml
exporters:
  - id: parquet
    opts: {profile: node}          # drop every ground-truth channel and column
  - id: veremi
    opts: {codebook: veremi-ext, slice_s: 3600}
  - id: sumo-fcd
    opts: {period_s: 1.0}
```

Each entry is [`exporters[].id`](../configuration/reference/exporters.md#s-exporters-id)
with its [`opts`](../configuration/reference/exporters.md#s-exporters-opts), and writes
into `<out>/<id>/`. Naming any exporter makes the run keep `recording.mcap`; a run that
names exporters refuses `--no-recording` and `--checkpoint-every`. `v2xw run --veremi
<codebook>` adds the `veremi` exporter from the command line. An id this build does not
have is refused at load with the list of valid ids.

| id | Writes | Options |
|---|---|---|
| `recording` | keeps the MCAP recording itself | |
| `parquet` | one Apache Parquet table per recorded channel, the primary analysis format | `profile`: `full` (default) or `node` |
| `arrow` | the same tables as Arrow IPC | `profile` |
| `jsonl` | the same tables as JSON Lines | `profile` |
| `veremi` | VeReMi traces per receiver and a ground-truth log | `codebook`, `slice_s` |
| `sumo-fcd`, `sumo-tripinfo`, `sumo-edgedata`, `sumo-lanedata`, `sumo-summary`, `sumo-e1`, `sumo-e2` | SUMO's output files | see [below](#sumo-output-files) |
| `dataset` | one run of the V2X-SCMS-MBD release: the local, global and truth views as Parquet, a datasheet and, by default, VeReMi traces | `run_id`, `split`, `tier`, `instrumented_receivers`, `heldout_attacks`, `compat_veremi`; see [below](#the-dataset-release-exporter) |

## Tables: Parquet, Arrow, JSON Lines

One table per recorded channel (`node.tx`, `node.rx`, `phy.rx`, `mac.cbr`, `metric.sample`,
`ma.report`, …; the channel list and each channel's visibility class are in
[Metrics and outputs](../recording-and-metrics.md#channels)). Every float is quantised to
the grid its column declares, the grids are written beside the data as `schema.json`, and a
scan reads the files back and fails on any value off its grid.

`opts.profile: node` is the blind export: it drops the ground-truth channels entirely and
projects the ground-truth columns out of mixed (`node-and-gt`) channels, so a node export
and a ground-truth export never share a file. Each file's schema records which it is.

From Python, any channel of a recording is one call away without an exporter:
`v2xw.Recording.open(path).arrow("node.rx")` ([Python](../getting-started/python.md)).

## VeReMi traces

The format the misbehaviour-detection literature reads: VeReMi (van der Heijden,
Lukaseder, Kargl, SecureComm 2018) and the VeReMi Extension (Kamel et al., ICC 2020), so a
detector written against them reads this engine's output unchanged. Field names are
VeReMi's (`rcvTime`, `sendTime`, `senderPseudo`, `messageID`, `pos`, `pos_noise`, `spd`,
`spd_noise`, `acl`, `acl_noise`, `hed`, `hed_noise`, `RSSI`, `attackerType`).

| `codebook` | Attack codes | Files |
|---|---|---|
| `veremi-ext` (default) | the F2MD attack enumeration 0–19 | `traceJSON-<rx>-<module>-A<code>-<t0>-<slice>.json` per receiver and slice; `traceGroundTruthJSON-<slice>.json` |
| `veremi-1` | VeReMi 1.0 bit codes: 1 ConstPos, 2 ConstPosOffset, 4 RandomPos, 8 RandomPosOffset, 16 EventualStop | `traceJSON-<receiver>.json` per receiver; `GroundTruthJSONlog.json` |

`slice_s` is the slice length (default 3600 s, F2MD's `VeReMiSliceTime`; 1 s to one week).
An engine attack family gets a code only where its behaviour matches the codebook's attack;
any other is code 1024, which a consumer can recognise rather than misread.

!!! warning "One deliberate deviation from VeReMi"
    VeReMi's per-receiver `sender` field holds the transmitter's real module id, a
    ground-truth column inside a receiver-visible file that lets a detector link every
    pseudonym for free. This exporter writes `sender` as a stable integer derived from the
    **pseudonym**; the true identity is only in the ground-truth file. The written traces
    go through the leakage linter, and the export fails if it finds anything.

Naming `veremi` also makes every BSM's `node.tx` record carry the broadcast position
accuracy and acceleration that `pos_noise` and `acl` are read from; a run without it
records exactly what it did before.

## SUMO output files

Each writes the file SUMO's option of the same name would, readable by SUMO's tools. They
need a `full` recording (they read `gt.kinematics`), and naming one makes the run record
`gt.spawn` and `gt.despawn`.

| id | File | SUMO equivalent | Options (default) |
|---|---|---|---|
| `sumo-fcd` | `fcd.xml` | `--fcd-output` | `period_s` (every step), `precision` (2) |
| `sumo-tripinfo` | `tripinfo.xml` | `--tripinfo-output` | `write_unfinished` (false), `precision` |
| `sumo-edgedata` | `edgedata.xml` | `<edgeData>` | `period_s` (whole run), `precision` |
| `sumo-lanedata` | `lanedata.xml` | `<laneData>` | `period_s`, `precision` |
| `sumo-summary` | `summary.xml` | `--summary-output` | `period_s` (every step), `precision` |
| `sumo-e1` | `e1.xml` | `<inductionLoop>` | `detectors: [{id, lane, pos_m}]` (required; `lane` a SUMO lane id), `period_s`, `precision` |
| `sumo-e2` | `e2.xml` | `<laneAreaDetector>` | `detectors: [{id, lane, pos_m, length_m}]`, `period_s`, `time_threshold_s` (1), `speed_threshold_mps` (1.39), `jam_threshold_m` (10), `precision` |

To export the network and demand themselves (not outputs), use
[`v2xw export-sumo`](sumo-cosimulation.md#exporting-to-sumo).

## The dataset release exporter

`dataset` writes one run of the V2X-SCMS-MBD misbehaviour-detection dataset into
`<out>/dataset/`, in the layout of the [release schema](../dataset/schema.md): what each
vehicle's on-board unit saw (`local/`), what the Misbehavior Authority saw (`global/`) and
the ground truth (`truth/`), in separate files so no label sits next to a feature.

```yaml
exporters:
  - id: dataset
    opts: {run_id: s01, split: train, tier: mini, instrumented_receivers: 10}
```

| Option | Values (default) |
|---|---|
| `run_id` | the run's partition key (derived from the scenario name and hash) |
| `split` | `train`, `validation`, `test` or `unassigned` (`unassigned`) |
| `tier` | `mini`, `standard` or `full` (`standard`) |
| `instrumented_receivers` | how many on-board units get `local/rx` and `local/ego` rows; 0 is every one (25) |
| `heldout_attacks` | attack names kept out of `train` (none) |
| `compat_veremi` | also write VeReMi traces into `compat/veremi/<run_id>/` (true) |
| `wire` | raw signed bytes; not built, `true` is refused (false) |

Naming `dataset` makes the run record what the views are built from (the
[record contract](../dataset/record-contract.md)) and turns
`security.scms.report_records` on; a run that does not name it records exactly what it did
before. Every float is put on its declared grid, and the leakage linter inspects the
node-visible files and refuses to finish if identity or labels leak into them.

`tools/dataset/` merges per-run folders into a release, checks it and publishes it
([Release tools](../dataset/tools.md)). The published release, v1.1, is a one-run sample
(`s01`: 60 vehicles for 75 s on a procedural grid with every US SCMS entity, the ten main
attacks on three vehicles each and 15 % faulty honest devices), built from
`scenarios/dataset/v11-sample.yaml`; [Samples](../dataset/samples.md) shows real rows of it
and [Hugging Face release](../dataset/hf-release.md) its versions. It is hosted on Hugging
Face as [akzaidan/v2x-scms-mbd-mini](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini)
(CC BY 4.0):

```python
from datasets import load_dataset
reports = load_dataset("akzaidan/v2x-scms-mbd-mini", "global_reports")
```

## Library dataset profiles

`v2xw_record::dataset` also holds library writers over a finished recording: the
`ma-dataset` profile (two schema versions), `receiver-logs`, `telemetry` and `net-trace`,
and a `DATASHEET.md` writer, gated by the same two checks (grids scanned back, the leakage
linter). They are not ids of the scenario's `exporters` list, and the live server's
`export.dataset` method answers that it does not host them.
