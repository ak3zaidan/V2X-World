# Extending the simulator

Every extension keeps the same rules: a model card, keyed randomness, `v2xw_core::math`
for transcendentals, no wall clock, no ground truth in node-side code, quantised outputs
([Conventions](dev/conventions.md); the invariants are also in
[AGENTS.md](https://github.com/ak3zaidan/V2X-World/blob/main/AGENTS.md)). The
conformance kit's plug-in suite checks them. Each section below is a summary; the linked
page is the step-by-step recipe.

| To add | Where it plugs in | Recipe |
|---|---|---|
| a model in any family | the family trait, a `ModelCard`, `Registry::register_model`, a slot builder in the engine | [Models](dev/extending/models.md) |
| a misbehaviour detector | `v2xw_threat::detect::Detector`, a suite in `phase2.rs` | [Detectors](dev/extending/detectors.md) |
| a metric | `v2xw_metrics::MetricProvider`, `MetricDef`, `register_all` | [Metrics](dev/extending/metrics.md) |
| a scenario setting | `schema.rs`, `validate.rs`, `options.rs`, `guide.rs` | [Settings](dev/extending/settings.md) |
| a shipped scenario or a city | `scenarios/`, `worlds/cities.yaml` | [Scenarios and cities](dev/extending/scenarios.md) |
| a signature or KEM scheme, a threshold protocol, a Python model, an out-of-tree crate | `v2xw_sec::scheme`, `v2xw_proto::threshold`, `v2xw.plugins`, `examples/` | [Plugins](dev/extending/plugins.md) |

## A model (any family)

Implement the family trait (`v2xw_radio::traits::Propagation`,
`v2xw_mobility::traits::CarFollowing`, ... ; the full table is on
[Models](dev/extending/models.md#1-implement-the-family-trait)), build its card with a unit,
default, range and source for every parameter (or `todo-calibrate` with a calibration
plan), draw randomness only through `ctx.rng(domain, entity)`, test it including a test
shown to fail, run `v2xw_conformance::plugin::run_suite`, and make it selectable where the
engine builds the slot: radio models in `crates/v2xw-engine/src/wiring.rs`
(`radio.models`), car-following in `v2xw_mobility::carfollowing::library::FACTORIES`
(`actors.vehicles.driving`), detectors, attackers and MA pipelines in `phase2.rs`, metrics
through `v2xw_metrics::register_all`.

`examples/` is an out-of-tree cargo workspace with one worked plug-in per interface:
`propagation-log-distance`, `carfollowing-constant-time-gap`, `detector-heading-rate`
(including the conformance suite) and `metric-detector-load`
(`cd examples && cargo test`).

## A detector

Implement `v2xw_threat::detect::Detector`; `on_message` receives an
`obs::ObservedMessage`, the receiver's `SelfBelief` and its map store, and nothing that
identifies the true sender. Quantise before comparing against a threshold; leave a check
whose input the receiver does not have out of the verdict rather than scoring it zero.
`detection.local` accepts the suites compiled into `phase2.rs`. See
[Detectors](dev/extending/detectors.md).

## A metric

Implement `MetricProvider` over recorded channels, never engine state, and declare each
metric with a `MetricDef` (unit, dimensions, grid, minimum samples, source, what it does
not account for). Use `stats::Proportion` (Wilson interval), `ratio_of_sums` and
`Distribution`; accumulate counts and bytes as integers. Register it in `register_all` or
behind an opt-in group in `wiring::build_metrics_for`, and give a headline metric a
direction and health band (`polarity.rs`, `health.rs`). See [Metrics](dev/extending/metrics.md).

## A setting

Add the field to `crates/v2xw-engine/src/scenario/schema.rs` (unit in the name, default
documented, skip-serialised at its default), a range in `validate::BOUNDS` or a set in
`validate::CHOICES`, a `KEY_STATUS` row, labels and presets in `scenario::options`, and a
level and guidance in `scenario::guide`; read it where the engine builds the affected
model, test that it changes a same-seed run, and run `just docs-gen`. The publish tests
require it to be described, loadable at its default and classified. See
[Settings](dev/extending/settings.md).

## A scenario

Copy the closest file in `scenarios/`, or write a short overlay on `scenarios/base/`
with `meta.base`. State what it measures and what it does not, justify each non-default
number, and use city-independent forms (`rate_veh_per_h_per_lane_km`, `rsu_placement`,
`target: auto`). `v2xw validate` must pass; add it to `scenarios/ready-made.txt` to offer
it in the Studio. See [Scenarios and cities](dev/extending/scenarios.md) and
[Scenarios](scenarios.md).

## A city

Add an entry to `worlds/cities.yaml`, fetch it with `worlds/fetch.sh <id>`, and check it
with `cargo run -p v2xw-cli --bin city_audit -- <scenario.yaml> --only <id>`. See
[Scenarios and cities](dev/extending/scenarios.md#adding-a-city).

## A signature or KEM scheme

Copy `crates/v2xw-sec/src/scheme/template.rs`, fill in the id, the parts in wire order
with their primitives and hardware-profile operations, and add one line to
`SIGNATURE_SCHEMES` or `KEM_SCHEMES`; `hybrid_mldsa65.rs` is a worked example. See
[Plugins](dev/extending/plugins.md#a-signature-or-kem-scheme).

## A model in Python

`python/v2xw/plugins.py` puts a Python object behind an engine trait: `CarFollowing`
(abstract tier, per vehicle per step) and `Detector` (once per window with an Arrow record
batch), both callable and conformance-checked through `v2xw.conformance`, and `MaPolicy`,
which runs inside a run started with `v2xw.run(..., ma_policy=...)`. A Python plug-in
draws no randomness of its own, reads no clock, uses `v2xw.math` instead of `math`, and
keeps hashed iteration order out of its results. Example: `python/examples/python_idm.py`.
See [Plugins](dev/extending/plugins.md#python-plug-ins).
