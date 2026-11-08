# Conventions

The rules below are enforced by code (a compiler attribute, a CI gate, a test) unless the
row says otherwise. [AGENTS.md](https://github.com/ak3zaidan/V2X-World/blob/main/AGENTS.md)
states the same invariants for coding agents.

## Workspace layout and layering

The workspace (`Cargo.toml`, resolver 3, edition 2024) has every directory under `crates/`
and `tests/conformance` as members. `examples/` is a separate cargo workspace that builds
the way an external crate would. Crate roles and exact dependency edges are in
[Crates](../crates.md); the layer picture is in [Architecture](../architecture.md#layers).

| Rule | Why |
|---|---|
| `v2xw-core` depends on no other workspace crate | it is the contract every model is written against |
| each model crate owns one domain and does not reach into another's (mobility never touches radio or security; radio treats a frame as a length, MCS and access category) | a model can be replaced without touching its neighbours |
| metrics read recorded channels, never engine state | a metric computed live and from a recording must agree |
| `v2xw-engine` is the only place model crates are composed | wiring is in one place (`wiring.rs`, `driving.rs`, `phase2.rs`) |
| node, detector, attacker and MA code reads only what a fielded device could know (`v2xw_core::nodeview`, `v2xw_threat::obs`) | belief and ground truth stay apart; see [the firewall](#the-determinism-and-truth-firewall) |

Each crate's `src/lib.rs` opens with a module table naming where each concern lives.

## Determinism

Same scenario, seed, build and plug-in set give byte-identical outputs on every platform
and thread count. The full contract is on [Determinism and time](../models/determinism-and-time.md);
in code it means:

| Do | Never |
|---|---|
| draw randomness from a keyed `v2xw_core::rng::RngStream` checked out through the model's context (`ctx.rng(domain, entity)`), keyed so the draw does not depend on event order | `rand::thread_rng`, a shared generator, a private seed |
| call transcendentals through `v2xw_core::math` (pure-Rust `libm`); `sqrt` is exempt because IEEE 754 makes it exact | `f64::sin`, `.exp()`, `.ln()`, `.powf()`, `.atan2()`, `.hypot()` and the rest |
| take time from the simulation clock (`v2xw_core::SimTime`); the manifest timestamp is an argument (`Engine::build(scenario, build_utc)`) | `SystemTime::now`, `Instant::now`, `UNIX_EPOCH` below the transport |
| use `BTreeMap`, `BTreeSet`, `IndexMap` or a sorted `Vec` | `std::collections::HashMap` / `HashSet` anywhere engine-facing |
| quantise floats to their declared grid before recording, exporting or digesting (`v2xw_core::math::quantize_to`; record constructors do it) | write a raw `f64` into a digest |
| run parallel work as a pure map merged in id order | let thread scheduling order reach a result |

### The determinism and truth firewall

Text rules over the source tree, in `tests/conformance/src/firewall.rs` (driven by
`tests/kit/firewall_suite.rs`) and in the node's own sentinel
(`crates/v2xw-node/src/firewall.rs`, driven by `crates/v2xw-node/tests/firewall_sentinel.rs`):

| Check | Forbidden text | Applies to | Exempt |
|---|---|---|---|
| `ground-truth` | `Kinematics`, `.world()`, `.actors()`, `ActorId`; a `gt_` field read outside telemetry; truth types bound on the security bridge | `v2xw-node` | the rule table itself |
| `wall-clock` | `SystemTime::now`, `Instant::now`, `UNIX_EPOCH` | every crate | `crates/v2xw-cli/src/wall.rs`, `crates/v2xw-server/src/http.rs` (the transport's liveness and seek budgets) |
| `hash-order` | `HashMap`, `HashSet` | every crate | `v2xw-metrics/src/arrow_out.rs`, `v2xw-record/src/export/schema.rs` (Arrow metadata, typed as a std map by the arrow crate) |
| `transcendental` | `.sin(`, `.cos(`, `.tan(`, `.exp(`, `.exp2(`, `.ln(`, `.log10(`, `.log2(`, `.powf(`, `.atan2(`, `.asin(`, `.acos(`, `.atan(`, `.hypot(`, `.cbrt(`, `.sinh(`, `.cosh(`, `.tanh(` | every crate | none |

The scanner recurses into subdirectories, sorts its file list, skips comments and closes
`#[cfg(test)]` blocks rather than latching them. Exemptions are data in the rule table,
each with its reason. CI's `determinism` job also imports the committed fixture
`tests/fixtures/midtown-6block.osm.xml` on Linux, macOS and Windows and fails unless all
three print the same world and payload digests.

### Golden digests

`tests/conformance/golden/*.json` pin, per fixed scenario, the scenario hash, world hash, a
digest over every record and per-channel counts. A change that moves them is an output
change. Re-bless only when the change is intended:

```sh
V2XW_CONFORMANCE_BLESS=1 cargo test -p v2xw-conformance --test kit golden
```

A new optional scenario field must serialise as absent at its default
(`#[serde(skip_serializing_if = ...)]`) so existing scenario hashes do not move.

## Error handling

| Rule | Where |
|---|---|
| library crates define typed errors with `thiserror`, marked `#[non_exhaustive]`, with `#[from]` conversions up the stack (`EngineError` wraps `ScenarioError`, `RegistryError`, `WorldError`, `MobError`, `CoreError`, `MetricError`, `RecordError`) | `crates/v2xw-engine/src/error.rs` and each crate's `error` module |
| a scenario error names the dotted field and the conflict (`ScenarioError::Conflict`), never a line number and a serde message; only `scenario::validate` builds it | `crates/v2xw-engine/src/scenario/validate.rs` |
| the validator returns every error, not the first | `validate::validate(&Scenario) -> Vec<ScenarioError>` |
| where no cited value exists for a vendor- or jurisdiction-specific number, the code refuses rather than guesses | hardware profiles, speed presets |
| malformed recorded or wire input is an error in debug and release builds, not a panic | `crates/v2xw-record/tests/overflow.rs` |

## Units in names

A scenario field's unit is part of its name (`duration_s`, `mobility_step_ms`,
`tx_power_dbm`, `rate_veh_per_h`), so a value cannot be separated from its unit. The
published surface reads the unit back off the suffix (`UNIT_SUFFIXES` in
`scenario/publish.rs`: `_s`, `_ms`, `_us`, `_ns`, `_m`, `_km`, `_mps`, `_mps2`, `_hz`,
`_db`, `_dbm`, `_dbi`, `_deg`, `_kbps`, `_mbps`, `_veh_per_h`). A numeric leaf whose name
cannot carry its unit declares it in `publish.rs`, and the test
`every_number_publishes_a_unit` fails the build otherwise. Use the same suffixes for
struct fields and local variables in model code.

## Model cards

`Registry::register` refuses a model without a complete `v2xw_core::card::ModelCard`:

- an id `family/name`, a version, a tier and a purpose;
- the equations;
- every parameter with a unit, a default, a range and a source, or `todo-calibrate` with a
  calibration plan;
- assumptions, limitations, what it ignores, and how it was validated;
- determinism: whether it draws random numbers and from which declared domains.

The step-by-step is in [Extending: models](extending/models.md).

## Citing standards

A number or rule taken from a standard or paper is cited in the comment beside it, by
document and section, as the code does throughout (`IEEE 1609.2 §5.2`,
`TS 102 687 §5.4`, `SAE J2945/1 §6.3`). A hard-coded value with no source is either
marked `todo-calibrate` on its card or refused. Licensed standard text (SAE J2735, IEEE)
is never quoted at length; field names and short identifiers are fine. The bibliography
of what the code cites is [Standards and references](../about/standards.md).

## Documentation in code

- `#![deny(missing_docs)]` in every library crate except `v2xw-demand`, `v2xw-interop`
  and `v2xw-netval`.
- `#![forbid(unsafe_code)]` in every library crate except `v2xw-cli` (`deny`),
  `v2xw-demand` and `v2xw-interop`.
- A doc comment says what the item is, its unit, and its source. It does not narrate how
  the code came to be.

## Formatting and lints

| Gate | Command |
|---|---|
| rustfmt | `cargo fmt --all -- --check` |
| clippy, warnings denied, all targets | `cargo clippy --workspace --all-targets -- -D warnings` |
| tests | `cargo test --workspace` |

CI (`.github/workflows/ci.yml`) runs all three on Ubuntu, macOS and Windows. Warnings are
denied on the clippy line rather than through `RUSTFLAGS`, so a warning in a dependency
under a new compiler does not break the build. The toolchain is pinned in
`rust-toolchain.toml` (channel `1.98.1`, with `rustfmt`, `clippy` and the
`wasm32-unknown-unknown` target), and CI asserts the installed compiler matches the pin.

## Settings

A new scenario setting must reach the Studio as a menu with units, not a free text box:
a range in `validate::BOUNDS` or a closed set in `validate::CHOICES`, a `KEY_STATUS`
row, a label and description in `scenario::options`, and a level in `scenario::guide`.
The publish tests fail otherwise. See [Extending: settings](extending/settings.md).

## Checks that can fail

A check is trusted only once it has been seen to fail. Validation suites run every check
with an injected fault; for a new unit or integration test, inject the fault by hand or
with `tools/mutate/mutate.py` and confirm it goes red ([tools](tools.md#toolsmutatemutatepy)).

## Commits

Short subject in the form `<area>: <what changed>` (`docs: ...`, `scenarios: ...`,
`v2xw-sec: ...`), in neutral descriptive language, one logical change per commit. See
[Contributing](contributing.md).
