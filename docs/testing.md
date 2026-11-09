# Testing and validation

A check is only trusted if it has been seen to fail. The validation suites and the
conformance kit therefore run each check against a deliberately broken input or model as
well as the shipped one, and report a check that the fault did not turn red as unproven.

## Where tests live

| Kind | Location | Run |
|---|---|---|
| unit tests | `#[cfg(test)]` modules beside the code | `cargo test -p <crate> --lib` |
| integration tests | `crates/<crate>/tests/*.rs` | `cargo test -p <crate> --test <name>` |
| conformance kit | `tests/conformance` (`v2xw-conformance`) | `cargo test -p v2xw-conformance` |
| network validation | `crates/v2xw-netval` | `cargo run --release -p v2xw-netval --bin netval-report` |
| traffic validation | `crates/v2xw-trafficval` | `cargo run --release -p v2xw-trafficval --bin traffic_validation` |
| SUMO cross-validation | `crates/v2xw-interop`, `crates/v2xw-cli/src/bin/cross_validate.rs` | `just cross-validate` (needs SUMO) |
| Python | `python/tests/` | `pytest python/tests` after `maturin develop` |
| UI | `ui/**/test`, `ui/apps/studio/e2e*` | `pnpm --dir ui test` ([studio.md](studio.md)) |

The whole Rust suite (`cargo test --workspace`, `just test`) runs real P-256/P-384
arithmetic and many full engine runs and is long; scope with `-p` and `--test` while
iterating. Some engine and validation tests import `worlds/cache/manhattan.osm.xml`
(fetch with `worlds/fetch.sh manhattan`).

The longest whole-run tests are marked `#[ignore = "slow: ..."]`: a plain `cargo test`
lists them as ignored, and `tools/ci/slow-tests.sh` runs every one of them by exact name
(`--list` shows them; `tools/ci/slow-tests.sh 2 4` runs shard 2 of 4, as CI does). The
script fails if a mark names no test that runs. Mark a new test slow when it takes minutes
in a debug build; CI's `ci` profile (`cargo test --profile ci`, the workspace optimised at
opt-level 1 with overflow checks and debug assertions on) is what both CI jobs use.

The Studio's browser tests render WebGL on SwiftShader, a software rasteriser, so on CI's
few slow cores a frame takes several times longer than on a laptop. To reproduce that
locally, `VWP_E2E_CPU_HOGS=<n> pnpm --dir ui --filter @vwp/studio test:e2e` runs `n`
busy-loop processes alongside the suite (one per core is about a CI runner). Tests that wait
for the camera wait on the viewer's frame clock (`renderClockSeconds`), not on wall time, and
poll with probes that draw nothing; a test that needs a frame rate or a wall-clock interval
is wrong on a slow runner even when it passes on a fast one.

## Selected integration suites

| Crate | Tests | What they establish |
|---|---|---|
| `v2xw-sec` | `official_vectors.rs`, `cavp.rs`, `ieee1609_2a_d6.rs`, `crl_ctl_vectors.rs`, `ecqv_certs.rs` | butterfly and linkage values against CAMP and OpenSCMS vectors; NIST CAVP samples for ECDSA P-384, SHA-384 and AES-128-CCM; IEEE 1609.2a Annex D.6 KDF2/MAC1 vectors |
| `v2xw-sec` | `equivalence.rs`, `overhead.rs` | real and modelled backends give identical outcomes and sizes; envelope overhead equals its derivation byte for byte |
| `v2xw-sec` | `real_artefacts.rs` | the published US SCMS trust lists and chain files decode and verify, when the local cache (not redistributable) is present; `V2XW_SCMS_ARTEFACTS=1` makes absence a failure |
| `v2xw-record` | `byte_identity.rs`, `overflow.rs` | replayed frames equal live frames; malformed input is an error in debug and release |
| `v2xw-proto` | `flows.rs`, `wire_sizes.rs` | every flow emits exactly its declared stages; every message size has a provenance |
| `v2xw-node` | `firewall_sentinel.rs` | node code cannot reach ground truth (source scan, proven with injected faults) |
| `v2xw-engine` | `scenario.rs`, `sensitivity_expectations.rs`, ... | loader and validator behaviour; expectation rows load and are consistent |
| `v2xw-cli` | `cli.rs`, `longrun.rs` | two runs agree on `content_digest`; a resumed run equals the uninterrupted one |

## The conformance kit (`tests/conformance`)

```sh
cargo test -p v2xw-conformance                     # everything
cargo test -p v2xw-conformance --test vwp          # the VWP v1 wire checklist, one test per item
cargo test -p v2xw-conformance --test kit          # plug-in suite, golden digests, firewall
cargo run  -p v2xw-conformance --bin v2xw-golden-digest
```

Paths are relative to `tests/conformance/`.

| Part | Library | What |
|---|---|---|
| wire checklist | `src/checklist.rs`, `tests/vwp/` | each item of [protocol/vwp-v1.md](protocol/vwp-v1.md) §10, named after its id; items owned by another crate are delegated and the pointer is checked; items that cannot be established in process are recorded with the reason |
| plug-in suite | `src/plugin.rs` | for a contributor's model: no private RNG, no wall clock, no ground truth, valid card with a source for every default, determinism under reordering and thread count, quantised outputs, declared RNG domains match the ones used |
| golden determinism | `src/golden.rs`, `golden/*.json` | scenario hash, world hash, record digest and per-channel counts of fixed scenarios, no floats, paths or timestamps; a missing record fails; re-bless with `V2XW_CONFORMANCE_BLESS=1 cargo test -p v2xw-conformance --test kit golden` only for an intended output change |
| interface firewall | `src/firewall.rs` | the node firewall, generalised to other crates |

## Validation suites

`v2xw-netval` holds each network layer to a reference from outside the crate that
implements it: the PHY to its published equations and receiver tables, the MAC to IEEE
802.11 / EN 302 663, DCC to TS 102 687 and SAE J2945/1, the sidelink to the 3GPP
procedures, headers and messages to their tables and to an independent decoder, the
security envelope to OpenSSL, and whole runs to conservation, metamorphic and determinism
properties. Each `Check` runs in `Mode::Honest` (must pass) and `Mode::Faulted` with a
named realistic fault (must fail), on every invocation. Faults are injected at the
model-check boundary, so no rebuild per fault is needed.

`v2xw-trafficval` groups its checks as model (IDM, MOBIL, gap acceptance and social force
against their papers; ring-road fundamental diagram; string stability; the Sugiyama
ring), calibration (saturation flow, start-up lost time, speeds and accelerations,
turning, critical gaps, pedestrians, motorcycles against published measurements), safety
(TTC and PET distributions, SSAM thresholds, conflict rates), invariants (no red entry, no
conflicting greens under NEMA, no jumps at closures) and city imports. The
`traffic_calibration` study compares fundamental diagrams (Greenshields and triangular
fits, capacity against the HCM), stop-and-go wave speed, headway distributions, string
stability and the Sugiyama ring with theory and with SUMO's default driver, and exits
non-zero when a held figure leaves its band.

`just validation-report` runs both suites and writes `target/validation/network.md` and
`target/validation/traffic.md`; `just cross-validate` compares traffic with SUMO on a grid
and on Midtown and the radio with Veins and OpenCV2X results, writing
`target/validation/cross-validation.md`. `v2xw check-traffic <run>` audits any finished run.

What each suite and the Python cross-check tools prove, and how to run them:
[Validation and conformance tools](dev/tools.md). CI gates and the contribution workflow:
[Contributing](dev/contributing.md).

## Continuous integration

`.github/workflows/ci.yml` runs, on Ubuntu, macOS and Windows: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --profile ci --workspace`.
A `rust-slow` job runs the slow-marked tests on Ubuntu in four shards
(`tools/ci/slow-tests.sh <shard> 4`). A
determinism job imports `tests/fixtures/midtown-6block.osm.xml` on each platform and
publishes the world and VWP payload digests; a final job fails unless all three platforms
agree (`just world-hash` prints this machine's value).
