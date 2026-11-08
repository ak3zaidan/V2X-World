# Architecture

## Layers

```text
 interfaces   v2xw-cli (v2xw, v2xw-server)   v2xw-server (VWP + JSON-RPC)   v2xw-py   v2xw-wasm   ui/
                         |                              |
 orchestration        v2xw-experiment  v2xw-engine (scenario, event loop, wiring, manifest)
                         |
 runtime              v2xw-node (OBU/RSU/VRU device runtimes)   v2xw-proto (credential protocols)
                         |
 models               v2xw-world  v2xw-mobility  v2xw-demand  v2xw-radio  v2xw-net  v2xw-msg
                      v2xw-sec  v2xw-threat  v2xw-energy  v2xw-interop
                         |
 measurement          v2xw-metrics   v2xw-record
                         |
 contract             v2xw-core (kernel, RNG, registry, model cards, manifest, math)
```

Each model crate owns one domain and does not reach into another's: mobility never
touches radio or security, radio treats a frame as a length, MCS and access category,
metrics read recorded events and never engine state. `v2xw-engine` is where the model
crates are composed into a run. Exact dependency edges: [crates.md](crates.md).

## A run

1. **Load.** `Scenario::load` (`crates/v2xw-engine/src/scenario/`) parses YAML or JSON,
   migrates the schema, merges `meta.base` overlays on the untyped document,
   deserialises and validates, returning every error with its dotted field
   ([scenarios.md](scenarios.md)).
2. **Build.** `Engine::build(scenario, build_utc)` imports or generates the world
   (`v2xw-world`), registers every selected model with its card in the
   `v2xw_core::Registry`, spawns nodes and backend entities (`wiring.rs`, `phase2.rs`) and
   assembles the manifest.
3. **Run.** `Engine::run(&mut recorder)` drives one event heap until the end sentinel.
   Every record goes to a `Recorder` (`MemoryRecorder`, or the MCAP writer in
   `v2xw-record`) and to the metric providers.
4. **Finish.** The CLI writes the recording, the resolved scenario, metrics, the run
   report and the manifest, then reads the recording back to verify it
   ([recording-and-metrics.md](recording-and-metrics.md); on-disk format:
   [dev/recording-format.md](dev/recording-format.md)).

## The event loop

`crates/v2xw-engine/src/run.rs`. One single-threaded loop pops events in the total order
`(time, priority, seq)`: priority is fixed per event class, `seq` is assigned at schedule
time, so ties resolve by rule. A few event classes are *phases*: pure maps over actors,
receivers or nodes that may run on a thread pool, merged in id order.

```text
pop (time, priority, seq)
  Control       scenario timeline: weather, outages, demand, parameter changes
  MobilityStep  phase over actors; publish gt.kinematics and a VWP Keyframe/Delta, advance
                the world by dt, rebuild the snapshot, spawn and retire nodes
  PhyEnd        phase over one frame's receivers: link budget, SINR, reception outcome
  PhyStart      one frame goes on the air
  NodePhase     phase over nodes: generation, signing, verification, stores, detectors;
                node-local queues drain inside the map and never reach the heap
  Observe       metric flush and the end-of-run sentinel
```

Mobility is periodic (`time.mobility_step_ms`). A radio event strictly inside a step reads
positions by the published extrapolation rule `pos(t') = pos(t) + vel(t) (t' - t)`
(`v2xw_core::kinematics::Kinematics::extrapolate`, applied only in `Engine::position_at`).
Each mobility step is a Jacobi update: every decision reads a frozen start-of-step
snapshot, so actor visiting order cannot change the result.

**Time dilation and fast-forward.** A scenario may declare windows
(`time.fast_forward`) in which traffic is frozen and no radio event is generated, and the
engine jumps between credential-system events (top-ups, shuffles, CRL publication and
fetches, re-enrolment) on the mobility-step grid. Weeks of PKI behaviour fit in a short
run; every window is recorded in the manifest.

## Fidelity tiers

Each model family has `abstract`, `medium` and `high` tiers (`v2xw_core::card::Tier`):
closed-form or probabilistic stand-ins for very large runs, the literature-standard
models, and frame-level detail. A scenario picks tiers per family (`radio.tiers`) and may
name a specific model by id (`radio.models`). The validator refuses combinations that
cannot compose, for example `phy: high` with `mac: medium`. At city scale
(`scale`), a microscopic focus region runs inside a mesoscopic queue network
(`mobility/tiered/meso-micro`) with the radio matched to it.

## Plug-in seams

Every model family is a trait (for example `v2xw_radio::traits::Propagation`,
`v2xw_mobility::traits::CarFollowing`, `v2xw_threat::Detector`,
`v2xw_metrics::MetricProvider`) plus a model card. Models reach the engine only through
a narrowed context (`v2xw_core::ctx::Ctx`, implemented once in
`crates/v2xw-engine/src/ctx.rs`, narrowed per family in `adapters.rs`), which supplies
the clock, keyed RNG streams and the reads that family is allowed. Models run in process:
Rust models link into the engine (the registry refuses a copyleft licence in process),
and Python models sit behind the same traits through the adapters in `v2xw-py`.
[extending.md](extending.md) walks through each seam.

## Determinism contract

Same scenario file + seed + engine build + plug-in set gives byte-identical outputs on
macOS, Linux and Windows, x86-64 and arm64, native or WebAssembly, any thread count.
The mechanisms, all in `v2xw-core`:

| Mechanism | Where |
|---|---|
| counter-based RNG: each stream keyed by `SHA-256(master_seed, domain, entity)`, samplers with fixed draw counts | `rng.rs` |
| pure-Rust transcendentals (`libm`); platform `f64` functions are not used | `math.rs` |
| fixed event priorities and monotonic sequence numbers | `event.rs` |
| dense, totally ordered ids; reductions sorted by id | `ids.rs` |
| writer-side quantisation of every recorded float to its declared grid | `math::quantize_to`, `v2xw-world/src/quant.rs`, `v2xw-record/src/grid.rs` |

No component below the transport reads the wall clock; ordered collections are used
wherever iteration order reaches an output. The run manifest (`v2xw_core::manifest`,
assembled in `crates/v2xw-engine/src/manifest.rs`) pins the engine version, a build hash
over compiler, platform, git commit and dirty state, every registered model with its card
hash, the world hash, the scenario hash over canonical JSON, the master seed and the
crypto mode. Runtime diagnostics (wall time, memory) are excluded from every digest by
construction (`v2xw_metrics::summary::DigestSet`).

## Belief and ground truth

The simulator separates what is true from what a device could know:

- A node (`v2xw-node`) holds its own GNSS fix, its own drifting clock, what it received and
  what is in its stores. It sees the world through `v2xw_core::nodeview`, never through
  actor state.
- Detectors and the MA pipeline consume `v2xw_threat::obs::ObservedMessage` and the
  receiver's own belief. Attackers receive an `AttackerView` with no world, no actor
  index and no actor ids; every action that changes bytes on the air is logged host-side
  on a ground-truth channel with the true actor id, which is what labels a dataset.
- Entities in the credential system talk only through modelled links that charge for
  bytes and compute (`v2xw_proto::kernel`); an end entity reads no backend state.
- Recording channels carry a visibility class; NODE-only recordings and datasets refuse
  ground-truth records.

Source scans in `crates/v2xw-node/tests/firewall_sentinel.rs` and the conformance kit's
firewall suite enforce these boundaries.

## Interfaces

| Interface | Code | Notes |
|---|---|---|
| CLI | `crates/v2xw-cli/src/cli.rs` (clap) | `v2xw run`, `validate`, `info`, `experiment`, `compare`, `sensitivity`, ... ([getting-started.md](getting-started.md)) |
| Server | `crates/v2xw-server` | WebSocket `/vwp/v1`, JSON-RPC on the socket and on `POST /rpc`, world payload at `/world/{hash}.vwb`; method list in `rpc::METHODS`, OpenRPC document from `rpc.discover` ([dev/server.md](dev/server.md)) |
| Browser | `ui/` | the Studio ([studio.md](studio.md)) |
| Python | `crates/v2xw-py`, `python/` | scenarios, runs, recordings, Arrow metric tables, Python plug-ins ([api/python.md](api/python.md)) |
| WebAssembly | `crates/v2xw-wasm` | the replay reader compiled for the browser, no server needed |
| SUMO | `crates/v2xw-interop` | TraCI co-simulation, `.rou.xml` import/export, network export |
