# Rust API

The rustdoc of every workspace crate is built by CI (`cargo doc --workspace --no-deps`)
and served under this section of the site. Every library crate except `v2xw-demand`,
`v2xw-interop` and `v2xw-netval` denies missing docs, so every public item is documented.
Each crate's front page opens with a module table naming where each concern lives. Crate
roles and dependency order: [Crates](../crates.md).

!!! note "If a crate link below does not open"
    The rustdoc pages are published by the site's automatic deployment. Until a
    deployment has included them, build them locally with the commands below; they are
    the same pages.

## Build locally

```sh
cargo doc --workspace --no-deps --open      # all crates, opens target/doc in a browser
cargo doc -p v2xw-engine --no-deps --open   # one crate
```

## Crates

### Contract

| Crate | Main entry points |
|---|---|
| [`v2xw_core`](../api/v2xw_core/index.html) | `event::Scheduler` (the DES kernel), `rng::RngStream` / `RngRegistry` (keyed streams), `registry::Registry`, `card::ModelCard`, `model::Model`, `ctx::Ctx`, `manifest::Manifest`, `math` (deterministic transcendentals and quantisation), `nodeview` |

### Models

| Crate | Main entry points |
|---|---|
| [`v2xw_world`](../api/v2xw_world/index.html) | `model::World`, `WorldSourceSpec`, the importers (`osm`, `sumo`, OpenDRIVE, `dem`), `procedural`, `los`, `serde_vwp::WorldPayload`, `WorldError` |
| [`v2xw_mobility`](../api/v2xw_mobility/index.html) | `traits::{CarFollowing, LaneChange, IntersectionControl, Router, Demand, VruMobility, GnssModel, ClockModel}`, `engine::NativeMobility`, `Idm`, `carfollowing::library` |
| [`v2xw_demand`](../api/v2xw_demand/index.html) | `assign`, `choice`, `od`, `counts`, `purpose`, `routefile` |
| [`v2xw_radio`](../api/v2xw_radio/index.html) | `traits::{Propagation, Fading, ObstacleModel, Phy, Mac, Dcc}`, `budget::LinkBudget`, `EdcaOcbMac`, `NakagamiFading`, `cellular::CellularUu` |
| [`v2xw_net`](../api/v2xw_net/index.html) | `netlayer::NetLayer`, `frag::Fragmenter` and its strategies, `accounting::ByteLedger` |
| [`v2xw_msg`](../api/v2xw_msg/index.html) | `codec::{MessageCodec, Encoded}`, `generator`, `j2735`, `evidence` |
| [`v2xw_sec`](../api/v2xw_sec/index.html) | `envelope::SecuredPdu`, `crypto::{CryptoBackend, Real, Modeled}`, `scheme` (`SIGNATURE_SCHEMES`, `KEM_SCHEMES`), `chain`, `butterfly`, `linkage`, `hashed_id8` |
| [`v2xw_node`](../api/v2xw_node/index.html) | `runtime::ObuRuntime`, `rsu::RsuRuntime`, `policy::VerificationPolicy`, `safety::SafetyApp`, `profile::HardwareProfile`, `firewall` |
| [`v2xw_proto`](../api/v2xw_proto/index.html) | `kernel::Kernel`, `scms::CampScms`, `etsi`, `threshold::ThresholdProtocol`, `pseudonym::PseudonymStrategy` |
| [`v2xw_threat`](../api/v2xw_threat/index.html) | `attack::Attacker`, `detect::{Detector, Verdict}`, `ma::MaPipeline`, `obs::{ObservedMessage, SelfBelief}`, `report::MisbehaviourReport`, `ts103759`, `us_mbr` |
| [`v2xw_energy`](../api/v2xw_energy/index.html) | `traits::{EmissionModel, EvEnergyModel}`, `spec::EnergySpec`, `provider::EnergyProvider` |
| [`v2xw_interop`](../api/v2xw_interop/index.html) | `sumo::SumoMobility`, `traci`, `routes`, `netexport`, `xval` |

### Measurement

| Crate | Main entry points |
|---|---|
| [`v2xw_metrics`](../api/v2xw_metrics/index.html) | `MetricProvider`, `ProviderSet`, `MetricDef`, `MetricSample`, `stats`, `invariants::check_all`, `RunSummary`, `health::HealthBand` |
| [`v2xw_record`](../api/v2xw_record/index.html) | `writer::RecordingWriter`, `reader::{Reader, VerifyReport}`, `channels::CHANNELS`, `wire::Frame`, `profile::NodeProfileStripper`, `export::Exporter`, `dataset` |

### Orchestration

| Crate | Main entry points |
|---|---|
| [`v2xw_engine`](../api/v2xw_engine/index.html) | `Scenario` (`scenario::{validate, publish, options, guide, migrate, merge}`), `Engine`, `RunReport`, `MemoryRecorder`, `Event`, `city`, `EngineError`, `ScenarioError` |
| [`v2xw_experiment`](../api/v2xw_experiment/index.html) | `plan::ExperimentPlan`, `runner`, `aggregate::aggregate_cell`, `journal::Journal`, `table::ResultsTable`, `figures`, `foundry` |

### Interfaces

| Crate | Main entry points |
|---|---|
| [`v2xw_server`](../api/v2xw_server/index.html) | `serve_scenario`, `serve_stub`, `serve_replay`, `VwpServer`, `rpc::METHODS`, `openrpc::document`, `Engine` (the server's engine seam), `LiveEngine` |
| [`v2xw_cli`](../api/v2xw_cli/index.html) | `cli::Cli` (the `v2xw` command), `run`, `checkpoint`, `experiment` |
| [`_v2xw`](../api/_v2xw/index.html) (`v2xw-py`) | the native module of the Python package: `scenario`, `run`, `recording`, `metrics`, `plugins`, `conformance`, `mathmod` ([Python API](python.md)) |
| [`v2xw_wasm`](../api/v2xw_wasm/index.html) | `ReplaySession`, `Scene`, `CachedSource` |
| [`v2xw_copilot`](../api/v2xw_copilot/index.html) | `Agent`, `Copilot`, `analyst::Analysis`, `watch::Watcher`, `tools::ToolSurface`, `LlmProvider` |

### Validation

| Crate | Main entry points |
|---|---|
| [`v2xw_netval`](../api/v2xw_netval/index.html) | `Check`, `Mode`, `Layer`, `all`, `run` ([tools](../dev/tools.md#v2xw-netval-the-network-stack)) |
| [`v2xw_trafficval`](../api/v2xw_trafficval/index.html) | `Group`, `model`, `calib`, `safety`, `invariants`, `cities` ([tools](../dev/tools.md#v2xw-trafficval-traffic-and-mobility)) |
| [`v2xw_conformance`](../api/v2xw_conformance/index.html) (`tests/conformance`) | `plugin::{PluginUnderTest, run_suite}`, `golden::RunDigest`, `checklist`, `firewall` |

`v2xw-demand-cli` is a binary only (`v2xw-demand`).

!!! note "These links work on the published site"
    The pages under `api/<crate>/` are produced by `cargo doc` in CI and copied into the
    built site; they do not exist in a plain `zensical serve` preview.
