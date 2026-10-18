# V2X World Simulator

A simulator for connected-vehicle (V2X) networks: realistic traffic on real city maps, a frame-level radio and network stack on top of it, and the full security system that protects it (IEEE 1609.2 and the US SCMS), all in one deterministic engine with a 3D Studio and a built-in agent that can set up, run and analyse simulations from plain language.

It is built as a research instrument. Every model states its equations, its defaults and the citation behind each default; every metric says how it is computed; and every run is byte-identical for the same scenario and seed, on any machine and any number of threads.

## What it simulates

**Traffic**
- Cars, trucks, buses, motorcycles, cyclists and pedestrians on 64 city maps (mostly US) from OpenStreetMap, or on generated grids.
- Car-following (IDM by default; Krauss, EIDM, W99 and Gipps available) and lane changing (MOBIL by default; SUMO's LC2013 available), each driver with their own parameters, reaction times and style.
- Lane-level routing with strategic lane choice, junction yielding and gap acceptance, fixed-time and actuated signals, gridlock handling and rerouting, kerbside stops, pedestrian crossings and refuge islands.
- A city-scale mesoscopic tier for very large fleets, and an optional co-simulation mode where SUMO drives the traffic over TraCI.

**Radio and network**
- DSRC (IEEE 802.11p), LTE-V2X and NR-V2X sidelink.
- Per-link channel from real building geometry: 3GPP TR 37.885 line-of-sight and NLOS laws, a street-corner model, through-building loss, correlated shadowing, Nakagami fading and optional Doppler coherence.
- Cumulative interference, a windowed-SINR NIST error model, EDCA channel access with carrier sense and back-off, half-duplex and hidden terminals, and C-V2X semi-persistent scheduling.
- Congestion control (SAE J2945/1 rate and power control, ETSI reactive and adaptive DCC), WSMP and ETSI GeoNetworking with multi-hop forwarding, and BSM, CAM, DENM, CPM, VAM, SPaT and MAP generation.
- Roadside units with SPaT/MAP, relay and CRL roles and selectable backhaul; a packet-level cellular model with TCP and TLS for backend traffic.

**Security and the SCMS**
- Real IEEE 1609.2 signed messages (COER encoding, ECDSA P-256), implicit (ECQV) and explicit certificates, and full receiver validation: chain to an approved root, validity, region, permissions and revocation.
- The US SCMS as deployed: governance with 5 electors and 3-of-5 trust lists on P-384, multiple providers each with their own root, ICA, ECA, RA, ACA, linkage authorities, misbehaviour authority and CRL generator, the real IEEE 1609.2.1 messages, butterfly key provisioning, linkage-based revocation, peer-to-peer certificate distribution and the real one-week certificate calendar.
- The European CCMS, threshold authorities (FROST, CGGMP21, Umbrella), and hybrid and post-quantum signatures (ML-DSA, Falcon, SLH-DSA).
- Attackers (fake positions, replay, jamming, Sybil, compromised RSUs), on-vehicle misbehaviour detection, reporting and revocation.

**Measurement**
- 91 metrics with plain definitions and health bands: delivery ratio, latency broken into 11 stages, channel busy ratio, age of information, overhead by layer, security processing, certificate supply, revocation latency, detection quality, linkability, traffic flow and safety indicators.
- Runs can be compared with a noise test, swept one setting at a time, and checked against an expectation table of how each setting should move the metrics.

## Status

The engine runs end to end: traffic, radio, network, security and the SCMS backend in one process, with the Studio and the agent on top. It is validated by its own suites (see [Validation](#validation)) and cross-checked against SUMO, Veins, Artery, OpenCV2X and published results.

The SCMS is being rebuilt to match the real US deployment exactly. The cryptography, formats, protocols, receiver validation, multi-provider deployment and real calendar are in; the redesigned Studio PKI view and the end-to-end conformance run are in progress. The plan and its status are in [`docs/design/scms-real/`](docs/design/scms-real/00-gap-analysis-and-plan.md).

Known limits are recorded rather than hidden: [`docs/design/BUILD-STATUS.md`](docs/design/BUILD-STATUS.md) lists what each round left open, and each validation report lists its failing rows.

## Quick start

**Requirements:** Rust 1.98.1 (pinned in `rust-toolchain.toml`), Node 22 or newer with pnpm 10, and optionally [`just`](https://just.systems). Python 3.12 is only needed for the validation oracle and the SUMO tools.

**1. Optional: add model keys for the built-in agent.** Create `.env` in the repository root (it is git-ignored):

```bash
OPENAI_API_KEY=...          # or ANTHROPIC_API_KEY=...
V2XW_AGENT_PROVIDER=openai  # openai or anthropic
OPENAI_MODEL=...            # optional model override
```

The engine reads these at start-up and never prints them. Without keys everything works except the agent.

**2. Start the engine.** It builds the release binary the first time:

```bash
./run-sim.sh                                   # default scenario (Manhattan)
./run-sim.sh scenarios/portland-downtown.yaml  # any scenario
V2XW_PORT=8788 ./run-sim.sh                    # a second engine on another port
```

**3. Start the Studio** in a second terminal, then open http://127.0.0.1:5173:

```bash
pnpm --dir ui install
pnpm --dir ui --filter @vwp/studio dev
```

To restart cleanly: `pkill -f v2xw-server; ./run-sim.sh`.

## Using the Studio

- **Header:** run, pause, restart, playback speed, scenario and city, Quick settings, Settings, Metrics, Summary, Agent and the backend view (being redesigned as a full PKI view).
- **3D scene:** Map, Chase, Driver's view and Free cameras, layers for radio links, messages and signals, and click-to-inspect for any vehicle, radio link or roadside unit.
- **Settings:** about 300 settings in sections a researcher expects (Run, Traffic, Radio & Channel, Medium Access, Congestion Control, Messages, Networking, Security & Certificates, Backend & Cellular, Attackers, Output), Basic first with Advanced behind a toggle, searchable, each with a plain explanation, a cited default and a recommended range.
- **Metrics:** every metric with its chart, health band and definition, plus a run summary.
- **Compare:** "Compare with previous run" or any two saved runs: what settings changed and how every metric moved.
- **Agent:** type a request ("Midtown Manhattan, 400 vehicles, DSRC, US SCMS, 5% Sybil attackers, run 2 minutes"); it stages the settings for you to run, watches the run, flags unhealthy metrics and answers questions from the real numbers.

## Command line

The `v2xw` binary (`cargo build --release -p v2xw-cli`) runs everything headless:

| Command | What it does |
|---|---|
| `v2xw run <scenario> [--city <id>]` | Run a scenario and write its recording, metrics and manifest |
| `v2xw validate <scenario>` | Check a scenario without running it |
| `v2xw serve <scenario>` | Serve a run to the Studio, with the agent endpoints |
| `v2xw cities` | List the city library |
| `v2xw compare <runA> <runB>` | Settings that differ and every metric's change, with a noise test |
| `v2xw sensitivity <spec>` | One-at-a-time same-seed sweeps; `--catalogue` sweeps every setting, `--expect` checks the expectation table |
| `v2xw compare-signatures <scenario>` | Signing overhead of each signature scheme against ECDSA P-256 |
| `v2xw check-traffic <run>` | Traffic invariants: stalls on green, overlaps, jumps, braking bounds, red entries |
| `v2xw experiment <scenario>` | Expand a scenario's experiment block into runs and aggregate them |
| `v2xw agent "<prompt>"` | The agent builds a scenario, runs it and writes a report |
| `v2xw energy <run>` | Fuel, emissions and EV energy from a recording |
| `v2xw export-sumo <scenario>` | Write the network and demand in SUMO formats |
| `v2xw import-osm <extract>` | Import an OpenStreetMap extract |
| `v2xw info <recording>` | Print a recording's manifest and channels |
| `v2xw resume <run>` | Continue a checkpointed run |

## Scenarios and cities

[`scenarios/`](scenarios/) holds about 45 ready-made scenarios: Manhattan and Portland traffic, rush hour and peak load, pedestrians, highway congestion, DSRC / LTE-V2X / NR-V2X comparisons, the credential lifecycle on the real calendar, revocation latency, pseudonym strategies, attacks, threshold authorities, post-quantum signatures and the European CCMS. Any scenario can run on any of the 64 cities with `--city`. City maps are © OpenStreetMap contributors (ODbL 1.0).

A scenario is one YAML file: the world, traffic, radio, network, messages, security, nodes, attackers, metrics and seed. Every field is validated when it loads, and combinations where a setting would have no effect are refused with a plain message. [`docs/guides/configuration-reference.md`](docs/guides/configuration-reference.md) lists every setting with its default, range and source.

## How it works

One discrete-event kernel holds every future event in a single queue ordered by (time, priority, sequence) and jumps from one event to the next. Time is counted in nanoseconds, and the engine never reads the wall clock.

- **Traffic** is one recurring event that moves every vehicle and pedestrian every 100 ms.
- **Radio events** happen at their exact microsecond times in between: each device generates and signs its messages, waits for the channel, transmits, and every receiver decides whether it decoded the frame.
- **The SCMS backend** runs on its own event kernel, advanced in step with the vehicles, and its messages travel over the modelled cellular and backhaul links.
- **An actor is a body that moves; a node is a radio device.** Nodes only know what their GPS and their received messages tell them; metrics compare that belief with the ground truth.
- **Every random number** comes from a stream keyed by (seed, purpose, entity), so results do not depend on thread count or order. Parallel work inside an event is merged back in ID order.
- **Every record** passes through one path that feeds both the recording and the metric providers, so metrics come only from recorded events.

## Validation

- **Foundations:** a hand-checked two-vehicle run in which every metric is computed by hand ([`docs/validation/foundations.md`](docs/validation/foundations.md)).
- **Network and traffic suites:** each layer held to its reference and each traffic model to its paper and field data, with every check proved able to fail (`v2xw-netval`, `v2xw-trafficval`).
- **Cross-validation:** traffic against SUMO on the same networks, and radio against Veins, OpenCV2X, Artery and published curves ([`docs/validation/cross-validation.md`](docs/validation/cross-validation.md), [`radio-fidelity.md`](docs/validation/radio-fidelity.md)).
- **Sensitivity:** a sweep of every setting and an expectation table of more than 150 rows, each with its source, checking that settings move metrics the way theory says ([`docs/validation/sensitivity-expectations.md`](docs/validation/sensitivity-expectations.md)).
- **Security:** official IEEE 1609.2a, CAMP and NIST test vectors, an independent Python oracle written without reading the Rust, and cross-checks against the real trust lists and certificate chains published by SCMS Manager ([`docs/design/scms-real/vectors-provenance.md`](docs/design/scms-real/vectors-provenance.md)).
- **Determinism:** the same scenario and seed give byte-identical runs on 1 and 8 threads.

## Repository layout

| Path | Contents |
|---|---|
| `crates/v2xw-core` | Event kernel: clock, event queue, keyed RNG streams, registry, manifest |
| `crates/v2xw-engine` | Scenario schema and loader, the main loop, wiring, the SCMS bridge, sensitivity tools |
| `crates/v2xw-world` | Map model, OpenStreetMap and SUMO importers, procedural grids, spatial indices |
| `crates/v2xw-mobility` | Traffic: car-following, lane changing, junctions, signals, pedestrians, GNSS, meso tier |
| `crates/v2xw-demand`, `v2xw-demand-cli` | Demand and route choice: OD matrices, traffic assignment, routes from counts, GTFS |
| `crates/v2xw-radio` | Propagation, fading, shadowing, PHY, MAC, congestion control, sidelink, cellular |
| `crates/v2xw-net` | WSMP, GeoNetworking and BTP, fragmentation, byte accounting |
| `crates/v2xw-msg` | ASN.1 encoders (J2735, ETSI, IEEE 1609.2 and 1609.2.1), message generators and size models |
| `crates/v2xw-sec` | IEEE 1609.2 security: certificates, chains, ECIES, CCM, butterfly keys, linkage, CRL and CTL, cost tables |
| `crates/v2xw-node` | Device runtimes for OBUs, RSUs and pedestrian devices: queues, signer and verifier, stores, P2PCD |
| `crates/v2xw-proto` | Protocol host: the SCMS and CCMS entities, flows, governance, deployment, threshold authorities |
| `crates/v2xw-threat` | Attackers, jammers, misbehaviour detectors, misbehaviour authority, privacy observer |
| `crates/v2xw-metrics` | Metric providers and their definitions |
| `crates/v2xw-record` | Recording (MCAP), Parquet and dataset exporters |
| `crates/v2xw-energy` | Emission, fuel and EV energy models |
| `crates/v2xw-experiment` | Parameter sweeps, replications and aggregation |
| `crates/v2xw-interop` | SUMO TraCI co-simulation, route import and export, cross-validation |
| `crates/v2xw-server` | JSON-RPC control and the WebSocket stream the Studio uses |
| `crates/v2xw-copilot` | The built-in agent and its evaluation suite |
| `crates/v2xw-cli` | The `v2xw` and `v2xw-server` binaries |
| `crates/v2xw-netval`, `v2xw-trafficval` | Network and traffic validation suites |
| `crates/v2xw-py`, `v2xw-wasm` | Python bindings and the WebAssembly replay reader |
| `ui/` | pnpm workspace: `apps/studio` (the Studio), `packages/viewer` (3D), `packages/protocol`, `packages/mock-server` |
| `scenarios/` | Ready-made scenarios |
| `crates/v2xw-node/profiles/hardware/` | OBU, RSU, pedestrian-device and backend hardware profiles, each number cited |
| `tools/oracle/` | The independent Python oracle for the security code |
| `third_party/` | Standards' ASN.1 modules with provenance |
| `docs/` | Design, guides, validation reports, model cards, decision records |
| `legacy/` | The original Python reference, kept for its conformance vectors |

## Development

```bash
just build           # cargo build --workspace
just test            # cargo test --workspace
just lint            # clippy and formatting checks
just studio          # the Studio dev server
just ui-test         # Studio unit and browser tests
just validate <scenario>
just validation-report
just cross-validate
just --list          # every recipe
```

Rules the codebase keeps:
- **Determinism:** keyed RNG streams only, ordered collections only, no wall clock, and results that must not change with thread count.
- **Citations:** every constant cites its source or is labelled as an assumption.
- **Failing checks:** every test is shown to fail when its fault is injected.
- **Settings:** a new option is a validated, published scenario field that actually changes the run.

## Documentation

| Where | What |
|---|---|
| [`docs/design/`](docs/design/) | Architecture, models, protocols, measurement and the build status |
| [`docs/design/scms-real/`](docs/design/scms-real/) | SCMS research (deployment, cryptography, protocols), the gap analysis and build plan, constants and test-vector provenance |
| [`docs/guides/`](docs/guides/) | Configuration reference, parameter inventories, comparing runs, signature schemes |
| [`docs/validation/`](docs/validation/) | Every validation, sensitivity and cross-validation report |
| [`docs/adr/`](docs/adr/) | Architecture decision records |
| [`docs/site/`](docs/site/) | Generator for a browsable site built from the model cards and schema |

## Licensing

Code is Apache-2.0 ([`LICENSE`](LICENSE)); generated datasets are CC-BY-4.0. GPL tools (Veins, Artery, F2MD, ns-3) are read as references and re-implemented, never linked. SUMO is optional and runs out of process. City maps are © OpenStreetMap contributors under ODbL 1.0. Published SCMS Manager artefacts are fetched for tests, not redistributed.
