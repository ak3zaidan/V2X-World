# Crates

The workspace (`Cargo.toml`) has every directory under `crates/` and `tests/conformance` as
a member. Each crate's `src/lib.rs` opens with a module table naming where each concern
lives; read it before the module. `examples/` is a separate cargo workspace of out-of-tree
plug-ins ([extending.md](extending.md)).

## Contract

| Crate | Owns | Entry points | Depends on |
|---|---|---|---|
| `v2xw-core` | DES kernel, keyed RNG streams, model registry and card schema, provenance, run manifest, ids, deterministic math, shared value types (kinematics, beliefs, weather) | `event::Scheduler`, `rng::RngStream`, `registry::Registry`, `card::ModelCard`, `manifest::Manifest`, `ctx::Ctx`, `math` | none |

## Models

| Crate | Owns | Entry points | Depends on |
|---|---|---|---|
| `v2xw-world` | the static world: lanes, junctions, connections, signal plans, buildings, terrain; importers (OSM, SUMO `net.xml`, OpenDRIVE, DEM); procedural grid/radial/random generators; spatial indices; the content hash | `model::World`, `osm`, `sumo`, `procedural`, `dem`, `los`, `validate` | core |
| `v2xw-mobility` | ground-truth kinematics: car-following, lane change, lateral, intersections, routing, demand, VRUs, kerbside stops, mesoscopic tier, GNSS error and clock drift | `traits::{CarFollowing, LaneChange, IntersectionControl, Router, Demand, VruMobility, GnssModel, ClockModel}`, `engine::NativeMobility` | core, world |
| `v2xw-demand` | dynamic traffic assignment, route choice, OD matrices, routes from counts, GTFS transit, trip purposes | `assign`, `choice`, `od`, `counts`, `transit`, `purpose` | core, world, mobility |
| `v2xw-radio` | propagation, fading, obstacles, 802.11p PHY and EDCA MAC, LTE-V2X and NR-V2X sidelink (SPS, BLER), DCC, cellular Uu, jamming emission, channel accounting | `traits::{Propagation, Fading, ObstacleModel, Phy, Mac, Dcc}`, `budget`, `cellular::CellularUu` | core, world |
| `v2xw-net` | WSMP and GeoNetworking/BTP header sizes, GeoNetworking router state machines, fragmentation strategies, loss amplification, byte accounting | `netlayer::NetLayer`, `frag::Fragmenter`, `stack`, `accounting` | core, radio |
| `v2xw-msg` | message generation and encoding: generated ASN.1 for ETSI messages and the 1609.2 envelope types, hand-written J2735 codecs, size models, per-message byte-exactness evidence | `codec::MessageCodec`, `codec::Encoded`, `evidence`, `j2735`, `sec_types` | core |
| `v2xw-sec` | the IEEE 1609.2 / TS 103 097 envelope, certificates and chains, CRLs and CTLs, ECIES/CCM, real and modelled crypto backends, signature-scheme catalogue, butterfly keys, linkage values | `envelope::SecuredPdu`, `crypto::{Real, Modeled, CryptoBackend}`, `scheme`, `butterfly`, `linkage` | core, msg |
| `v2xw-node` | device runtimes: hardware profiles, queues, CPU/HSM service, stores, verification policies, message generation, P2PCD, RSU roles, VRU devices, safety applications, telemetry | `runtime::ObuRuntime`, `rsu::RsuRuntime`, `policy::VerificationPolicy`, `safety::SafetyApp`, `profiles` | core, sec, msg, radio, net, record |
| `v2xw-proto` | credential-system protocols as entities with queues on modelled links: CAMP SCMS, ETSI TS 102 941, threshold protocols; flows and their stage stamps | `kernel::Kernel`, `scms`, `etsi`, `threshold::ThresholdProtocol`, `spec` | core, sec, msg |
| `v2xw-threat` | attackers, compromised RSUs, report poisoning, privacy observer, detector suites, misbehaviour reports, MA pipelines | `attack::Attacker`, `detect::Detector`, `ma::MaPipeline`, `catalog`, `ts103759`, `us_mbr` | core |
| `v2xw-energy` | emissions, fuel and EV energy from recorded kinematics | `traits::{EmissionModel, EvEnergyModel}`, `spec::EnergySpec` | core, metrics |
| `v2xw-interop` | SUMO: TraCI client, SUMO-driven mobility, `.rou.xml` import/export, network export, cross-validation study | `sumo::SumoMobility`, `traci`, `routes`, `netexport`, `xval` | core, world, mobility, metrics, radio |

## Measurement

| Crate | Owns | Entry points | Depends on |
|---|---|---|---|
| `v2xw-metrics` | metric providers and definitions, statistics with intervals, invariant checks, the run summary and metric digest | `MetricProvider`, `ProviderSet`, `MetricDef`, `stats`, `invariants`, `summary` | core |
| `v2xw-record` | VWP frame encoding, the MCAP recorder and replay reader, seek index, channel catalogue, Parquet/Arrow/JSONL exporters, dataset profiles and leakage linter | `writer::RecordingWriter`, `reader::Reader`, `wire`, `channels`, `export`, `dataset` | core, metrics |

## Orchestration

| Crate | Owns | Entry points | Depends on |
|---|---|---|---|
| `v2xw-engine` | scenario schema, loader, validator and published settings surface; the event enum and main loop; wiring of every model crate; backend and security lifecycle; city library; manifest; sensitivity analysis | `Scenario`, `Engine`, `MemoryRecorder`, `scenario::{validate, publish}`, `run`, `phase2`, `city` | core, world, mobility, radio, net, msg, sec, node, proto, threat, record, metrics, energy, demand, interop |
| `v2xw-experiment` | sweep expansion, replications, derived seeds, resumable journal, aggregation with intervals, results tables, figure presets, a MAP-Elites attack search | `plan::ExperimentPlan`, `runner::RunExecutor`, `aggregate`, `journal`, `foundry` | core, engine, metrics, record, threat |

## Interfaces

| Crate | Owns | Entry points | Depends on |
|---|---|---|---|
| `v2xw-server` | VWP v1 over WebSocket, JSON-RPC control plane, OpenRPC document, live, replay and synthetic-fixture engines | `serve_scenario`, `serve_replay`, `serve_stub`, `rpc::METHODS` | core, world, metrics, mobility, engine, record, sec, msg, proto, node |
| `v2xw-cli` | binaries `v2xw` (`src/cli.rs`), `v2xw-server`, `cross_validate`, `city_audit` | `cli::Cli` | most crates |
| `v2xw-demand-cli` | binary `v2xw-demand`: demand generation on a scenario's world | `src/main.rs` | demand, engine, world |
| `v2xw-py` | native module of the `v2xw` Python package (`python/`) | `scenario`, `run`, `recording`, `metrics`, `plugins` | core, mobility, metrics, record, world, engine |
| `v2xw-wasm` | the `v2xw-record` replay reader compiled to `wasm32-unknown-unknown`, with range-fetched input | `session`, `scene`, `ranges` | core, record |
| `v2xw-copilot` | an optional natural-language assistant that drives the server's RPC surface and summarises runs from their own metrics; needs a model-provider API key | `agent`, `analyst`, `tools`, `watch` | core, metrics, server, engine |

## Validation

| Crate | Owns | Depends on |
|---|---|---|
| `v2xw-netval` | network validation: each radio, MAC, network, message and security layer against its reference, each check run honest and with an injected fault (`netval-report` binary) | core, world, radio, net, msg, sec, node, engine, metrics |
| `v2xw-trafficval` | traffic validation: models against their papers and field data, safety surrogates, traffic invariants, city imports, each with a fault (`traffic_validation`, `traffic_calibration` binaries) | core, world, mobility, metrics, engine |
| `tests/conformance` (`v2xw-conformance`) | VWP v1 wire checklist, plug-in conformance suite, golden determinism harness (`v2xw-golden-digest` binary), interface firewall | core, world, record, server, node, engine |

See [testing.md](testing.md) and [dev/tools.md](dev/tools.md) for how these are run. The
rustdoc of every crate, with its main entry points, is linked from the
[Rust API](api/index.md) page.
