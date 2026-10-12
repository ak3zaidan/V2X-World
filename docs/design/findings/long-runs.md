# Long runs: days of simulated time

Status 2026-10-07. What broke when the simulator was asked for hours and days, what was
changed, what a simulated day costs, and the ways to make one cheaper with what each gives
up. Measurements on the owner's 8 GB M-series laptop with other engineers building beside
them (wall clock is noisy; counts are not), release build with LTO off.

## What broke first

Measured at commit `c4c4721` plus a progress line (`ffa2bbb`, which changes nothing a run
computes): `v2xw run scenarios/manhattan-5min.yaml --duration 6h --rate-veh-per-h 300`, the
default scenario at a fifth of its density (37–49 vehicles on the map), recording on.

**The run did not survive two simulated hours.** The machine's 4.5 GB cap stopped it at
1 h 30 min simulated, 13.5 min of wall clock:

| simulated | wall per simulated hour | memory | metric samples held | recording |
|---|---|---|---|---|
| 0:15 | 393 s | 713 MB | 283,345 | 226 MB |
| 0:30 | 498 s | 1,418 MB | 601,283 | 530 MB |
| 0:45 | 603 s | 2,161 MB | 934,970 | 894 MB |
| 1:00 | 559 s | 2,894 MB | 1,268,005 | 1,231 MB |
| 1:15 | 509 s | 3,603 MB | 1,589,631 | 1,528 MB |
| 1:30 | 677 s | 4,357 MB | 1,929,131 | 1,935 MB |

What broke, in the order a long run meets it:

1. **Memory: 2.9 GB per simulated hour**, in the command line tool, not the engine. `v2xw run`
   kept every `metric.sample` record as a parsed `serde_json::Value` until the end of the run
   to write `metrics.json` (about 2.4 kB of heap each). With `metrics: [all]` a run emits 78
   series and about 330 samples per simulated second; a 20-minute run with no recording at all
   reached 957 MB the same way.
2. **The end of the run would have read the whole recording into memory** to digest and verify
   it (`std::fs::read`): a 6-hour recording is about 8 GB, so no recording longer than a couple
   of hours could have been verified on this machine even had the run reached its end.
3. **Output sizes.** The full recording grows 1.3 GB per simulated hour at 40–50 vehicles —
   about 31 GB a day — and `metrics.json` at the one-second metric period 12 GB a day
   (437 bytes a sample).
4. **Cost per simulated hour** varied 393–677 s with the fleet (37–49 vehicles) and the
   shared machine; over 90 minutes it showed no trend of its own. The engine's stores
   (`Engine::census`) grew with vehicles ever spawned, not with work in flight: by 1 h 30 min,
   332 SCMS device records, 56,319 backend wire steps; the event heap, pending receptions and
   frames on the air followed the live fleet.
5. **No checkpoint**: a crash at hour 40 of a two-day run lost the run.

## What was changed

| Where | Change |
|---|---|
| `v2xw run` | **Metrics streamed to `metrics.json`** as they are emitted (`stream::MetricsStream`), byte-identical to the old whole-document write. They were a `Vec<serde_json::Value>` held for the whole run: the first thing to break. |
| `v2xw run` | **The recording verified and digested from disk**, chunk by chunk (`Reader::open_paged`, `checkpoint::file_sha256`), not read into memory whole. |
| `v2xw run` | **`--record-level full\|receptions\|events\|metrics`**: `metrics` keeps the metric samples and the run's summary records (scenario events, the MA's cases and decisions, revocations, spawns and despawns) and no scene. `events` drops the scene and the per-reception journals (`node.rx`, `phy.rx`, `node.verify`: 95 % of an attack run's bytes); `receptions` keeps them. |
| `v2xw run` | **`--checkpoint-every 1h`** and **`v2xw resume <out>/checkpoint.json`** (below). |
| `v2xw run` | **`--progress-every`** (default every 15 simulated minutes for runs of an hour or more): `progress.jsonl` in the output directory, one line on stderr — wall clock per simulated hour, the physical memory footprint, the fleet, and the sizes of the collections the engine keeps (`Engine::census`). `--duration 6h`, `--quiet`. |
| engine | **`Engine::advance(until)` + `Engine::finish_run`**: a run driven in slices, which change nothing it computes (a boundary stops the loop between two events). `Engine::run` is the two together. |
| scenario | **`time.metric_period_s`** (default 1 s): how often every metric provider flushes a sample. The engine had it fixed at one second. |
| scenario | **`actors.vehicles.demand.params.daily`**: demand by the hour of the local day from NYC DOT counts (`v2xw_mobility::demand::daily`). |
| engine | A retired node's J2945/1 sub-interval is dropped (`j2945_quality_at` grew with every vehicle). |
| server | **A seek before the retained history computes the run again** from the start (`Engine::rewind`), with the page's progress bar, instead of being refused: a run of days keeps only its last hour or so in memory. `--resume <checkpoint.json>` opens a CLI run's checkpoint in the page. `Hello.metric_period_ns` is the scenario's. |
| page | Run duration presets from a minute to a week in the transport bar (`RunDuration`); the clock counts days (`1d 02:03:04`) and shows the run's own clock (`Thu 08:15`); chart axes tick on round hours and days; the sun, sky and headlamps follow the run's clock. |
| scenarios | `scenarios/manhattan-day.yaml`: a day of Midtown (below). |

## Checkpoint and resume

A checkpoint is a **verified replay point**, not a memory image (`crates/v2xw-cli/src/checkpoint.rs`
has the whole design):

* every `--checkpoint-every` the run writes `checkpoint.json` atomically: the inputs (the
  resolved scenario beside it, its hash, the world hash, the build timestamp), the instant,
  the record digest at that instant, and how far each output had got;
* the recording is written in segments closed at each checkpoint (`recording.0000.mcap`, …),
  each a complete recording, so a crash loses only the segment being written;
* `v2xw resume` rebuilds the engine, runs it to the checkpoint with every output off,
  **refuses to carry on unless the record digest there equals the checkpoint's**, cuts
  `metrics.json` and `progress.jsonl` back to the checkpoint and carries on.

The finished run equals the uninterrupted one by digest — record digest, content digest over
every recorded message, `metrics.json`, `run-report.json` and every file digest in the
manifest except the MCAP files' own bytes, which are not reproducible even between two
uninterrupted runs (the container writes its summary from a `HashMap`; that is why the content
digest exists). `crates/v2xw-cli/tests/longrun.rs` proves it on a 90 s run stopped 15 s past
its 30 s checkpoint, with the metrics and the progress of those 15 s already on disk; with the
resume not cutting `metrics.json` back, it fails. RESUME_MEASURED

**What it does not do: store the engine's memory.** Resuming at hour 40 replays 40 hours
(REPLAY_COST). A memory-image checkpoint needs every type the engine's state reaches to be
serialisable — an estimated 700 types across thirteen crates, about 280 of which derive
`Deserialize` today, plus the trait objects (the mobility model, the propagation and fading
models, every node's verification policy, the metric providers, the attacker models) — which
is a change to every crate and is the open item this design leaves.

## A day that looks like a day

DAY_SECTION

## What a simulated day costs, and how to make it cheaper

OPTIONS_SECTION

## What still grows

GROWTH_SECTION

## Not done

* **A memory-image checkpoint** (above). Resume replays.
* **Signal plans by time of day.** The plan data does not allow it: the worlds come from
  OpenStreetMap, which carries where the signals are and not their timing; the importer
  derives one fixed, coordinated plan per junction (`world.signals.coordinate`). NYC DOT
  does not publish per-intersection time-of-day timing as open data. A mechanism for
  scenario-declared plan windows (`{from: "22:00", cycle_s: …}`) without the data behind it
  would be an invented night plan, so none was added.
* **Vehicles that come back.** Every vehicle is a new device that drives once and leaves; a
  real fleet is the same cars every day. So the SCMS sees a new enrolment for every trip, its
  device records grow with trips (about 2,300 a day at the manhattan-day rate) rather than
  with the fleet, and a weekly i-period's top-up — which a car needs once a week — happens in
  a run only because every new device fetches its next period on joining. A device pool that
  spawned trips onto returning devices is the change that would make a week of credentials
  look like a week; it touches the node-id contract the live server's projector relies on.
* **An i-period boundary inside a day.** The SCMS epoch is the run's start, so a run shorter
  than a week never crosses a weekly i-period boundary; a run of eight days does.
* **Downsampling stored data.** Nothing stored is downsampled: the engine's metric history
  keeps every sample (16 bytes each), the charts ask for a binned display (`metrics.query`
  with a bin count) and the expanded chart decimates only its drawing.
* **The page attaching to a running CLI run.** A headless `v2xw run` serves nothing; the page
  watches it through its finished recording segments (open one in the run browser) or by
  `v2xw-server --resume <out>/checkpoint.json`, which computes to the checkpoint and serves
  the run from there.
