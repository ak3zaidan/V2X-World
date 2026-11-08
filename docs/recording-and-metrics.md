# Recording, metrics and exports

Code: `crates/v2xw-record` (container, encoder, reader, exporters, datasets),
`crates/v2xw-metrics` (providers, statistics, invariants, summary), the output step in
`crates/v2xw-cli/src/run.rs`, and `crates/v2xw-experiment` for sweeps.

## What `v2xw run` writes

| File | Contents | Reproducible |
|---|---|---|
| `recording.mcap` | every record, the manifest metadata, the resolved scenario as an attachment | the data section is; the file hash is not (see below) |
| `scenario.resolved.yaml` | the scenario after migration, base merge and defaults: what actually ran | yes |
| `metrics.json` | every `metric.sample` the run emitted | yes |
| `run-report.json` | event, frame and record counts per channel; `content_digest` | yes |
| `manifest.json` | `v2xw_core::manifest::Manifest`: build, models and card hashes, world, scenario and seed, crypto mode, the files above digested | all but `build_utc` and the recording's file hash |
| `progress.jsonl`, `checkpoint.json` | progress lines; a resumable checkpoint (`--checkpoint-every`) | |

**The digest to compare is `content_digest`**: SHA-256 over every message's topic,
`log_time` and bytes in stored order (`v2xw_record::Reader::content_digest`). The MCAP
writer emits its summary-section records from a hash map, so the file's own SHA-256 can
differ between two identical runs; the summary is an index and does not change content.
`--digest` with `--no-recording` streams the same kind of hash without writing a file,
for comparing scaling runs at different thread counts. A run resumed from a checkpoint
must produce the same digest as the uninterrupted run.

`--record-level` trades detail for size: `full` (scene and every record), `receptions`
(no scene), `events` (no per-reception records), `metrics` (metric samples and summaries
only, for runs of days). `--node-only` records the NODE-only profile, which refuses every
ground-truth record.

## The container

MCAP (readable with standard MCAP tools and Foxglove). Two namespaces, never mixed on one
topic:

- `vwp/...`: the VWP binary frames (`Keyframe`, `Delta`, and others) stored verbatim, so a
  replayed stream is byte-identical to the live one (VWP §7.2;
  `crates/v2xw-record/tests/byte_identity.rs`);
- `record/...`: serde records for events, telemetry and metrics.

The reader (`reader::Reader`) verifies chunk checksums, seeks by index, replays frames and
treats malformed input as an error, never a panic. `v2xw info` prints the manifest,
channels and verification report. The same reader runs in the browser through
`crates/v2xw-wasm`.

## Channels

`crates/v2xw-record/src/channels.rs` lists every channel with its visibility class
(`v2xw_core::ctx::Visibility`). Main groups:

| Visibility | Channels |
|---|---|
| `gt` | `gt.kinematics`, `gt.spawn`, `gt.despawn`, `gt.attack.action`, `gt.subject`, `gt.device_exposure`, `phy.prr`, `net.reassembly` |
| `node` | `node.tx`, `node.verify`, `node.drop`, `node.neighbor`, `node.security`, `node.credentials`, `node.telemetry`, `mac.cbr`, `net.bytes`, `net.frag`, `sec.cert`, `sec.pseudonym`, `proto.msg`, `det.observation`, `ma.report`, `ma.received`, `ma.case`, `ma.decision`, `app.warning`, `app.advice`, `privacy.link`, `privacy.track`, `pki.case`, `pki.flow` |
| `node-and-gt` | `node.rx`, `phy.rx`, `msg.latency`, `app.outcome`, `pki.knowledge`, `pki.report_fate` |
| `public` | `proto.revocation`, `backend.state`, `pki.state`, `signal.priority`, `traffic.control`, `scenario.event` |
| `derived` | `metric.sample` |
| `mixed` | `snapshot.keyframe`, `snapshot.delta` |

A `node-and-gt` record carries node-visible fields beside ground-truth ones; NODE-only
exports project the ground-truth fields out. The table in `channels.rs` is authoritative for each channel's class.

A run that names the `dataset` exporter also records `node.rx_sec`, `node.pki_event`,
`node.report_built` and `ma.crl_entry` (`node`) and `gt.tx_truth` and `gt.device` (`gt`),
which the release's views are built from; any other run never writes them
([Dataset record contract](dataset/record-contract.md)).

## Metrics

A `MetricProvider` reads recorded events and reduces them; it never touches engine
state. Each metric's `MetricDef` declares its name, unit, dimensions, aggregation,
visibility, definition, source, grid, the minimum sample count below which it reports
insufficiency, and what it does not account for (which validation refuses to leave
empty). Select with `metrics: [all]` or a list of metric names; a provider is installed
when the list names `all` or one of its metrics. The [metric catalogue](metrics/catalogue.md)
lists every metric with its unit, aggregation, visibility, definition and source; it is
generated from the definitions.

| Family | Module | Metrics |
|---|---|---|
| communication | `comms` | `pdr` by distance bin, `pdr_all_pairs`, `per`, `pdr_by_cause`, `cbr`, `pir`, `goodput`, `bytes_air`, `bytes_uu_ul`, `bytes_uu_dl`, `bytes_backhaul`, `bytes_backend`, `bytes_total`, `airtime_per_node` |
| latency | `latency` | `e2e_latency` (p50, p95, p99) per flow and message type, `latency_stage`, `latency_stage_share`, `latency_trace_rejected` |
| awareness | `awareness` | `aoi`, `aoi_peak`, `nar`, `delivery_ratio` |
| load | `load` | `channel_occupancy`, `channel_load`, `offered_load`, `carried_load`, `loss_rate`, `collision_rate`, `half_duplex_rate`, `mac_queue_depth`, `mac_drops`, `mac_access_delay` |
| overhead | `overhead` | `security_overhead`, `net_header_overhead`, `link_overhead`, `cert_bytes_share`, `air_bytes_per_payload_byte`, `bytes_per_vehicle_hour` |
| fragmentation | `frag` | `frag_sdu_loss`, `frag_sdu_loss_predicted`, `frag_sdu_loss_independent`, `frag_fragment_loss`, `frag_content_loss`, `frag_content_loss_predicted` |
| security | `security` | `verify_rate`, `verify_cost`, `verify_wait`, `verify_queue_depth`, `unverified_ratio`, `full_cert_share`, `p2pcd_requests`, `digest_misses`, `envelope_overhead`, `revocation_latency_stage`, `crl_entries`, `crl_bytes` |
| signing overhead | `signing` (opt-in: `metrics: [all, signing]`) | `sig_*`: what the signature scheme costs on the air, in devices and in the backend |
| detection | `detection` | the confusion matrix (`det_tp`, `det_fp`, `det_tn`, `det_fn`) and `det_recall`, `det_fpr`, `det_precision`, `det_f1`, `det_accuracy`, `time_to_detect`, `time_to_decision`, `false_accusations` |
| detection scores | `detection` (opt-in: `metrics: [all, detection-scores]`) | `det_mcc`, `det_informedness`, `det_markedness`, `det_kappa`, and the per-message matrix `det_message_tp`, `det_message_fp`, `det_message_fn`, `det_message_tn` |
| mobility and safety | `safety` | `ttc_min`, `ttc_conflicts`, `pet`, `drac`, `headway_time`, `headway_distance`, Edie `flow`/`density`/`mean_speed`, `speed`, `acceleration`; with `metrics: [all, ssm]` also the full surrogate safety measures (`ssm_*`) |
| pedestrians | `vru` | `ped_conflicts`, `ped_near_misses`, `ped_ttc`, `ped_wait`, `ped_midblock_share` |
| applications | `apps` (when `apps.enabled` lists any) | `app_warnings`, `app_precision`, `app_miss_ratio`, `app_lead_time_s` |
| traffic control | `traffic` (with a `traffic:` section or `metrics: [all, traffic]`) | gridlock (`traffic_jammed`, `traffic_teleports`, `traffic_removed`) and actuated-signal counts (`signal_gap_outs`, `signal_max_outs`) |
| credentials and privacy | `crates/v2xw-engine/src/privacy_metrics.rs` (runs with a security path) | `pseudonym_change_rate`, `cert_pool_valid`, `linkability`, `backend_link_up` ([Privacy](models/privacy.md)) |
| energy | `v2xw-energy` provider (with an `energy:` section) | emissions, fuel and EV energy per model, per link and per kilometre ([Energy and emissions](models/energy.md)) |
| runtime | `runtime` (library only) | `events_processed`, `events_per_second`, `wall_clock_per_sim_second`, `memory_high_water_mark`: diagnostics, excluded from every digest; the engine does not install this provider in a run, because it reads no clock |

`time.warmup_s` suppresses samples for windows that end inside the warm-up period.

### Statistics

Enforced by the types in `stats.rs`:

1. every aggregate carries its sample count;
2. a proportion carries a Wilson score interval; a ratio of sums (bytes over bytes) carries
   none and says so;
3. too few samples report insufficiency, never a point estimate;
4. percentiles state their interpolation rule (type 7, linear);
5. a zero denominator is insufficiency, not NaN.

Counts, bytes and airtime accumulate as integers; float reductions sort into IEEE-754
total order before summing; binning is on integer grid indices, so a value on a bin edge
lands in the same bin on every platform. `invariants.rs` turns each stated invariant
(for example byte conservation) into a runnable check that names what failed and by how
much. `health.rs` and `polarity.rs` give each headline metric a direction and health
bands, used by `v2xw compare`.

## Exporters (`exporters`)

Exporters run after the run, over the recording, and write into `<out>/<id>/`.

| Id | Output |
|---|---|
| `recording` | keeps the MCAP recording |
| `jsonl`, `parquet`, `arrow` | one table per recorded channel with a `schema.json` of declared grids; `opts.profile: node` drops ground truth |
| `veremi` | per-receiver traces and a ground-truth log in the VeReMi format (`codebook: veremi-ext`, the F2MD attack enumeration 0-19, or `veremi-1`, VeReMi 1.0 bit codes; `--veremi` on `v2xw run`) |
| `sumo-fcd`, `sumo-tripinfo`, `sumo-edgedata`, `sumo-lanedata`, `sumo-summary`, `sumo-e1`, `sumo-e2` | SUMO output formats, for tools written against SUMO |

Options and file layouts: [Datasets and exports](guide/datasets.md). Dataset profiles, the
datasheet and the leakage linter that keeps identity and labels out of node-visible
features are in `crates/v2xw-record/src/dataset/`. The public dataset is on
[Hugging Face](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini).

## Across runs

- `v2xw experiment run`: one results table per sweep, with replication counts, sample
  counts and intervals; a row with one usable replication carries no interval.
  `figures.rs` turns a results table into Plotly JSON.
- `v2xw compare a b`: the settings that differ and every headline metric's change, better
  or worse by its health direction (`v2xw/run-comparison/1`; JSON, Markdown or CSV).
- `v2xw sensitivity`: one-at-a-time perturbations on one seed with expected directions
  ([Sensitivity analysis](guide/sensitivity.md)).

Guides: [Running and recording](guide/running-and-recording.md),
[Comparing runs](guide/comparing-runs.md), [Experiments and sweeps](guide/experiments-and-sweeps.md).
