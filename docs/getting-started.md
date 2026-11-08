# Install and build

## Prerequisites

| Need | For | Notes |
|---|---|---|
| Rust via rustup | everything | `rust-toolchain.toml` pins the compiler (1.98.1) and installs `rustfmt`, `clippy` and the `wasm32-unknown-unknown` target on first use |
| `just` | optional task runner | `just --list`; every recipe is a plain cargo or pnpm command |
| Node 22+, pnpm 10 | the Studio | `ui/package.json` pins `pnpm@10.15.0` (`corepack` can provide it) |
| Python 3.9+, maturin | the Python package | `cd python && maturin develop` ([Python](getting-started/python.md)) |
| SUMO | optional | co-simulation (`mobility.provider: sumo`), `export-sumo` with `netconvert`, cross-validation; found through `SUMO_HOME` or `V2XW_SUMO_VENV` ([SUMO co-simulation](guide/sumo-cosimulation.md)) |
| OSM extracts | real-city scenarios | `worlds/fetch.sh <city-id>` writes `worlds/cache/<id>.osm.xml` (git-ignored, ODbL) ([Worlds and maps](guide/worlds-and-maps.md)) |
| A WebAssembly-capable `clang` | optional: replaying recordings in the browser with no engine | Apple's `clang` has no `wasm32` back end; Homebrew's `llvm` does ([Studio tour](getting-started/studio-tour.md#8-replay-a-recording-without-an-engine)) |

`just setup` fetches the Rust, Python and UI dependencies in one step.

## Get the code

```sh
git clone https://github.com/ak3zaidan/V2X-World.git && cd V2X-World
```

Every command on these pages runs from the repository root.

## Build

```sh
cargo build --release -p v2xw-cli      # target/release/v2xw and target/release/v2xw-server
cargo build --release -p v2xw-demand-cli   # target/release/v2xw-demand (demand and route tools)
cargo build --workspace                # everything, debug
```

Third-party crates are optimised even in debug and test profiles (`Cargo.toml`
`[profile.dev.package."*"]`), because the engine runs real P-256 and P-384 arithmetic. On a
machine with little memory, build with `-j 1` (as `just build-cli` and `just build-server`
do).

## Check the install

```sh
./target/release/v2xw --version
./target/release/v2xw validate scenarios/highway-congestion.yaml   # procedural road, no download
./target/release/v2xw run scenarios/phase1-grid.yaml --out runs/phase1
./target/release/v2xw info runs/phase1/recording.mcap
```

`phase1-grid.yaml` is one minute of a Midtown-shaped procedural grid with signed BSMs; it
finishes in about a minute of wall time. Then follow [Your first
simulation](getting-started/first-simulation.md).

## The Studio

```sh
pnpm --dir ui install
./run-sim.sh                                   # engine on :8787, paused (terminal 1)
pnpm --dir ui --filter @vwp/studio dev         # http://127.0.0.1:5173 (terminal 2)
```

[Studio tour](getting-started/studio-tour.md) walks through it; [Studio](studio.md)
describes its packages and checks.

## Where next

| To | Read |
|---|---|
| every command and flag | [Command-line tool](getting-started/cli.md) |
| run, record, checkpoint and resume | [Running and recording](guide/running-and-recording.md) |
| the server and its RPC methods | [Engine server and RPC](dev/server.md) |
| the assistant and its keys | [Assistant](guide/assistant.md) |

## Task-runner recipes

| Recipe | Command |
|---|---|
| `just build`, `just check`, `just test` | workspace build, check (all targets), tests |
| `just fmt`, `just fmt-check`, `just lint`, `just ci` | formatting and the CI gate (`ci` = fmt-check, lint, test) |
| `just run S`, `just validate S`, `just inspect R`, `just experiment S` | CLI in release |
| `just build-cli`, `just build-server` | release builds, one compile job at a time |
| `just serve S`, `just serve-fixture [N]`, `just studio`, `just studio-build`, `just ui-test` | server and UI |
| `just world-hash` | this machine's canonical world hash for `tests/fixtures/midtown-6block.osm.xml` |
| `just validation-report`, `just cross-validate` | the validation suites ([Building and testing](testing.md)) |
| `just docs-gen`, `just docs-serve`, `just docs-build` | regenerate the configuration reference; serve or build this site |
