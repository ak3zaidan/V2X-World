# AGENTS.md

Entry point for coding agents working in this repository. Read this, then the file in
`docs/` that matches the task ([docs/README.md](docs/README.md) has the reading order).

## Map

| Where | What |
|---|---|
| `Cargo.toml` | workspace: `crates/*` and `tests/conformance`; shared dependency versions |
| `crates/v2xw-core` | contract crate: DES kernel, RNG streams, registry, model cards, manifest, `math` |
| `crates/v2xw-{world,mobility,radio,net,msg,sec,node,proto,threat,metrics,energy,demand}` | the model crates, one domain each |
| `crates/v2xw-engine` | scenario schema and loader, event enum, main loop, wiring, manifest |
| `crates/v2xw-record` | MCAP recorder, replay reader, VWP frame encoder, exporters |
| `crates/v2xw-server` | VWP WebSocket and JSON-RPC control plane |
| `crates/v2xw-cli` | the `v2xw` and `v2xw-server` binaries |
| `crates/v2xw-{experiment,interop,py,wasm,copilot,demand-cli}` | sweeps, SUMO, Python, WebAssembly reader, assistant, demand tool |
| `crates/v2xw-{netval,trafficval}` | validation suites with fault injection |
| `tests/conformance` | wire checklist, plug-in suite, golden determinism digests, firewall |
| `scenarios/` | scenario YAML files; `base/` holds the shared deployments |
| `ui/` | pnpm workspace: `apps/studio`, `packages/{protocol,viewer,mock-server}` |
| `examples/` | out-of-tree plug-in examples (own cargo workspace) |

Crate roles and dependency order: [docs/crates.md](docs/crates.md). Data flow:
[docs/architecture.md](docs/architecture.md).

## Commands

```sh
cargo build --release -p v2xw-cli                 # the v2xw and v2xw-server binaries
cargo check -p <crate> --all-targets               # fast feedback, scoped
cargo test -p <crate>                              # scope tests; the full workspace suite is long
cargo test -p <crate> --test <file>                # one integration test file
cargo fmt --all -- --check                         # CI gate
cargo clippy --workspace --all-targets -- -D warnings   # CI gate
cargo test -p v2xw-conformance                     # conformance kit
v2xw validate <scenario>                           # every scenario error, field by field
v2xw run <scenario> --out runs/<name>              # a run and its outputs
pnpm --dir ui typecheck && pnpm --dir ui test      # UI
```

`just --list` shows the task-runner recipes (`just ci` = fmt-check, lint, test). Tests
that import the Manhattan extract need `worlds/cache/manhattan.osm.xml`
(`worlds/fetch.sh manhattan` downloads it; it is git-ignored).

## Invariants (do not break)

- **Determinism.** Same scenario + seed + build + plug-in set gives byte-identical outputs
  on every platform and thread count. In practice:
  - every random draw comes from a keyed `v2xw_core::rng::RngStream`
    (`SHA-256(master_seed || domain || entity)`), checked out through the model's
    context; never `rand::thread_rng` or a shared generator;
  - every transcendental goes through `v2xw_core::math` (pure-Rust `libm`), never `f64::sin`
    and friends; `sqrt` is exempt;
  - no wall clock below the transport: the engine takes its manifest timestamp as an
    argument (`Engine::build(scenario, build_utc)`);
  - no `HashMap`/`HashSet` iteration reaches an output: use `BTreeMap`, `BTreeSet` or a
    sorted `Vec`;
  - floats are quantised to their declared grid before they are recorded, exported or
    digested (`v2xw_core::math::quantize_to`; record constructors do it for you);
  - parallel work is a pure map merged in id order; the event loop is single-threaded.
- **Canonical digest.** A run's identity is `content_digest` in `run-report.json`
  (SHA-256 over every recorded message's topic, time and bytes), not the MCAP file hash.
  The golden records in `tests/conformance/golden/` pin, per fixed scenario, the scenario
  hash, world hash, a digest over every record and per-channel counts; re-bless only for
  an intended output change (`V2XW_CONFORMANCE_BLESS=1 cargo test -p v2xw-conformance --test kit golden`).
- **Scenario hashes hold.** A new optional scenario field must serialise as absent at its
  default (`#[serde(skip_serializing_if = ...)]`), so existing scenario hashes do not move.
- **Belief versus ground truth.** Nodes, detectors, attackers and the MA pipeline read
  only what a fielded device could know (`v2xw_core::nodeview`, `v2xw_threat::obs`).
  Every recording channel carries a visibility class (`v2xw_core::ctx::Visibility`:
  `gt`, `node`, `node-and-gt`, `public`, `derived`, `mixed`, `meta`); NODE-only outputs
  refuse ground truth. The firewall tests enforce it.
- **Model cards.** A model cannot be registered without a `v2xw_core::card::ModelCard`:
  every parameter has a unit, a default and a source; an unsourced number is marked
  `todo-calibrate` with a calibration plan.
- **No invented defaults.** Hardware service times, speed presets and similar
  jurisdiction- or vendor-specific numbers are data with citations; where none exists the
  code refuses rather than guesses.

## Conventions enforced in code

- `#![deny(missing_docs)]` in every library crate except `v2xw-demand`, `v2xw-interop`
  and `v2xw-netval`; `#![forbid(unsafe_code)]` in every library crate except `v2xw-cli`
  (`deny`), `v2xw-demand` and `v2xw-interop`.
- `cargo fmt` clean and `clippy -D warnings` clean on all targets; CI runs both on
  Linux, macOS and Windows, plus a cross-platform determinism comparison.
- Toolchain pinned in `rust-toolchain.toml`; edition 2024.
- A new scenario setting gets a range (`scenario::validate::BOUNDS`) or a closed value
  set (`CHOICES`), an implementation-status row (`KEY_STATUS`) and a label and
  description (`scenario::options`), so the Studio renders a menu with units rather than
  a free text box. Validation errors name the dotted field and the conflict.
- A check is only trusted if it can fail: validation suites run each check with an
  injected fault and require it to go red.

## Where to go next

| Task | Read |
|---|---|
| understand a run end to end | [docs/architecture.md](docs/architecture.md) |
| write or edit a scenario | [docs/scenarios.md](docs/scenarios.md), [docs/configuration.md](docs/configuration.md) |
| a model's equations and sources | [docs/models/](docs/models/) |
| add a model, detector, metric, setting or scenario | [docs/extending.md](docs/extending.md), then the recipe in [docs/dev/extending/](docs/dev/extending/) |
| code conventions, CI gates, docs workflow | [docs/dev/conventions.md](docs/dev/conventions.md), [docs/dev/contributing.md](docs/dev/contributing.md) |
| recordings, metrics, exports | [docs/recording-and-metrics.md](docs/recording-and-metrics.md), [docs/dev/recording-format.md](docs/dev/recording-format.md) |
| the browser UI, the server and the wire protocol | [docs/studio.md](docs/studio.md), [docs/dev/server.md](docs/dev/server.md), [docs/protocol/vwp-v1.md](docs/protocol/vwp-v1.md) |
| tests and validation suites | [docs/testing.md](docs/testing.md), [docs/dev/tools.md](docs/dev/tools.md) |
| an unfamiliar acronym | [docs/glossary.md](docs/glossary.md) |
