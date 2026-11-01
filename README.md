# V2X World Simulator

A simulator for connected-vehicle (V2X) networks. It combines realistic traffic on real city maps, a frame-level radio and network stack on top of that traffic, and the full security system that protects it: IEEE 1609.2 and the US Security Credential Management System (SCMS) as deployed. Everything runs in one deterministic engine. A 3D Studio sits on top, with a built-in agent that can set up, run and analyse simulations from plain language.

It is built as a research instrument:
- every model states its equations, its defaults and the citation behind each default;
- every metric says how it is computed;
- every run is byte-identical for the same scenario and seed, on any machine and any number of threads.


https://github.com/user-attachments/assets/527e9bd5-5d25-4480-9bf7-0818f529f154


## What it simulates

**Traffic**
- Cars, trucks, buses, motorcycles, cyclists and pedestrians on 64 city maps (mostly US, listed in [`worlds/cities.yaml`](worlds/cities.yaml)) from OpenStreetMap, or on generated grids.
- Car-following (IDM by default; Krauss, EIDM, W99 and Gipps available) and lane changing (MOBIL by default; SUMO's LC2013 available). Each driver has their own parameters, reaction times and style.
- Lane-level routing with strategic lane choice, junction yielding and gap acceptance, fixed-time and actuated signals, gridlock handling and rerouting, kerbside stops, pedestrian crossings and refuge islands.
- A city-scale mesoscopic tier for very large fleets, and an optional co-simulation mode in which SUMO drives the traffic over TraCI.

**Radio and network**
- DSRC (IEEE 802.11p), LTE-V2X and NR-V2X sidelink.
- A per-link channel from real building geometry:
  - 3GPP TR 37.885 line-of-sight and NLOS laws, plus a street-corner model;
  - through-building loss and correlated shadowing;
  - Nakagami fading, with optional Doppler coherence.
- Cumulative interference and a windowed-SINR NIST error model.
- EDCA channel access with carrier sense and back-off, half-duplex and hidden terminals, and C-V2X semi-persistent scheduling.
- Congestion control: SAE J2945/1 rate and power control, and ETSI reactive and adaptive DCC.
- WSMP and ETSI GeoNetworking with multi-hop forwarding.
- Generation of BSM, CAM, DENM, CPM, VAM, SPaT, MAP, SRM, SSM and WSA messages.
- Roadside units with SPaT/MAP, relay and CRL roles, WSA service announcements and a choice of backhaul.
- A packet-level cellular model with TCP and TLS for backend traffic.

**Intersections and safety applications**
- **SPaT and MAP** (SAE J2735, UPER) come from the real signal controller of each equipped intersection. They are signed and broadcast at the CTI 4501 rates: SPaT at 10 Hz and MAP at 1 Hz.
- **On-vehicle applications:**
  - forward collision warning (FCW), emergency electronic brake light (EEBL), intersection movement assist (IMA), left-turn assist (LTA), blind-spot and lane-change warning (BSW) and pedestrian collision warning (PCW);
  - red-light violation warning (RLVW);
  - green-light optimal speed advisory (GLOSA), with a setting for how many drivers follow it.
- **Signal priority:** vehicles request it with SRM and get NTCIP 1211 green extension or early green; SSM reports the state of each request.

**Security and the SCMS**
- **Signed messages:** real IEEE 1609.2 messages (COER encoding, ECDSA P-256) with implicit (ECQV) and explicit certificates.
- **Receiver validation:** the chain to an approved root, validity, region, permissions and revocation.
- **The US SCMS as deployed:**
  - **Governance and providers:** governance with 5 electors and 3-of-5 trust lists on P-384. Several providers each have their own root, ICA, ECA, RA, ACA, linkage authorities, misbehaviour authority and CRL generator. Each provider can be placed at a named site and given a capacity per role.
  - **Certificate supply:** the real IEEE 1609.2.1 messages over their HTTP framing, butterfly key provisioning and the real one-week certificate calendar. Enrolment, pseudonym, identification and RSU application certificates are all modelled, as are identified regions and peer-to-peer certificate distribution.
  - **Revocation:** linkage-based revocation, with the misbehaviour authority (MA) exchanging chain identifiers with both linkage authorities. Linkage and hash-based CRLs are issued and installed.
- **Misbehaviour:**
  - on-vehicle detection, with the SCMS Manager's US BSM checks as the default detector under a US deployment;
  - US misbehaviour reports (SM-MBR);
  - a declarative MA policy with eight presets, the CV-pilot manual review being the default;
  - an optional Python policy hook, and offline replay of a run's reports through a whole family of policies.
- **What-if options**, marked as not the US system:
  - the European CCMS;
  - CA hierarchies up to five ICA levels deep;
  - threshold authorities (FROST, CGGMP21, Umbrella);
  - hybrid and post-quantum signatures (ML-DSA, Falcon, SLH-DSA).
- **Attackers:** fake positions, replay, jamming, Sybil and compromised RSUs.

**Measurement**
- Metrics with plain definitions and health bands:
  - delivery ratio, latency broken into 11 stages, channel busy ratio, age of information and overhead by layer;
  - security processing, certificate supply and revocation latency;
  - detection quality and linkability;
  - traffic flow and safety indicators.
- Runs can be compared with a noise test, swept one setting at a time, and checked against an expectation table of how each setting should move the metrics.

## Status

The engine runs end to end: traffic, radio, network, security and the SCMS backend in one process, with the Studio and the agent on top. Its own suites validate it (see [Validation](#validation)), and it is cross-checked against SUMO, Veins, Artery, OpenCV2X and published results.

The SCMS is being rebuilt to match the real US deployment wherever the deployment can be observed:
- **Done:** the cryptography, formats, protocols, receiver validation, multi-provider deployment, real calendar, menus for every SCMS setting, and the redesigned certificate and metrics views in the Studio.
- **In progress:** the completion rounds, which close the remaining rule gaps. The plan and its status are in [`09-completion-plan.md`](docs/design/scms-real/09-completion-plan.md); the specs are in [`10-completion-specs.md`](docs/design/scms-real/10-completion-specs.md).
- **SAE J2945/1:** the vehicle side has been checked requirement by requirement against SAE J2945/1 (2020). The deviations found and their fixes are in [`11-j2945-1-conformance.md`](docs/design/scms-real/11-j2945-1-conformance.md).

Known limits are recorded rather than hidden:
- [`BUILD-STATUS.md`](docs/design/BUILD-STATUS.md) lists what each round left open.
- [`06-validation-summary.md`](docs/design/scms-real/06-validation-summary.md) states how close the SCMS is to the deployment, rule by rule.
- Each validation report lists its failing rows.

## Quick start

**Requirements:**
- Rust 1.98.1 (pinned in [`rust-toolchain.toml`](rust-toolchain.toml));
- Node 22 or newer, with pnpm 10;
- optionally [`just`](https://just.systems);
- Python 3.12, only for the validation oracle, the Python package and the SUMO tools.

**1. Optional: add model keys for the built-in agent.** Create `.env` in the repository root (it is git-ignored):

```bash
OPENAI_API_KEY=...          # or ANTHROPIC_API_KEY=...
V2XW_AGENT_PROVIDER=openai  # openai or anthropic
OPENAI_MODEL=...            # optional model override
```

The engine reads these at start-up and never prints them. Without keys, everything works except the agent.

**2. Start the engine** with [`run-sim.sh`](run-sim.sh). It builds the release binary the first time and listens on port 8787:

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

The Studio connects to `http://127.0.0.1:8787` unless `VWP_ENGINE` says otherwise, and `VWP_STUDIO_PORT` moves the Studio itself ([`vite.config.ts`](ui/apps/studio/vite.config.ts)).

To restart cleanly: `pkill -f v2xw-server; ./run-sim.sh`.

## Using the Studio

- **Header:** run, pause, restart, playback speed, scenario and city, Quick settings, Settings, Metrics, Agent, the light and dark themes, and a menu for the other panels (Certificate system, Compare two runs, Runs and recordings, Run details, Commands).
- **3D scene:**
  - Map, Chase, Driver's view and Free cameras;
  - layers for radio links, messages, signals and certificate states;
  - click-to-inspect for any vehicle, radio link or roadside unit.
- **Settings:**
  - grouped in the sections a researcher expects: Run, Traffic, Radio & Channel, Medium Access, Congestion Control, Messages, Networking, Security & Certificates, Backend & Cellular, Attackers, Output;
  - Basic settings first, with Advanced behind a toggle; the panel is searchable;
  - each setting has a plain explanation, a cited default and a recommended range;
  - every SCMS and security setting is a menu, a switch or a number with its unit, never a bare text box. Values that are accepted only to be refused show as disabled options with the reason.
- **Certificate system:**
  - the trust topology (CTL, roots, ICAs, ECA, RA, ACA, MA, linkage authorities, CRL generator), with what each entity does;
  - a vehicle's certificate life as a timeline, and a card for any one certificate with its chain up to the root;
  - each misbehaviour case told as a story: report, MA decision, CRL, then devices refusing the revoked certificate;
  - a readable decoder for any signed message, with the raw hex behind a toggle.
- **Metrics:**
  - every metric with its chart, health band and definition;
  - the SCMS metrics are grouped by the question each answers: are devices getting certificates, is revocation working and how fast, is misbehaviour being caught, and what security costs the radio and the CPU;
  - a run summary.
- **Compare:** "Compare with previous run" or any two saved runs: which settings changed and how every metric moved.
- **Agent:** type a request such as "Midtown Manhattan, 400 vehicles, DSRC, US SCMS, 5% Sybil attackers, run 2 minutes". The agent stages the settings for you to run, watches the run, flags unhealthy metrics and answers questions from the real numbers.

## Command line

The `v2xw` binary (`cargo build --release -p v2xw-cli`) runs everything headless. Its commands are defined in [`cli.rs`](crates/v2xw-cli/src/cli.rs):

| Command | What it does |
|---|---|
| `v2xw run <scenario> [--city <id>]` | Run a scenario and write its recording, metrics and manifest |
| `v2xw validate <scenario>` | Check a scenario without running it |
| `v2xw serve <scenario>` | Serve a run to the Studio, with the agent endpoints |
| `v2xw cities` | List the city library |
| `v2xw compare <runA> <runB>` | Settings that differ and every metric's change, with a noise test |
| `v2xw sensitivity <spec>` | One-at-a-time same-seed sweeps; `--catalogue` sweeps every setting, `--expect` checks the expectation table |
| `v2xw compare-signatures <scenario>` | Signing overhead of each signature scheme against ECDSA P-256 |
| `v2xw ma-replay <run>` | Replay a run's recorded misbehaviour reports through a family of MA decision rules or policy presets: honest devices blocked and the attacker's time to act, per policy |
| `v2xw check-traffic <run>` | Traffic invariants: stalls on green, overlaps, jumps, braking bounds, red entries |
| `v2xw experiment run\|status\|resume <scenario>` | Expand a scenario's experiment block into runs and aggregate them |
| `v2xw agent "<prompt>"` | The agent builds a scenario, runs it and writes a report |
| `v2xw energy <run>` | Fuel, emissions and EV energy from a recording |
| `v2xw export-sumo <scenario>` | Write the network and demand in SUMO formats |
| `v2xw import-osm <extract>` | Import an OpenStreetMap extract |
| `v2xw info <recording>` | Print a recording's manifest and channels |
| `v2xw resume <run>` | Continue a checkpointed run |

## Python

The [`python/`](python/) package drives the engine from Python ([`python/README.md`](python/README.md)). With it you can:
- load and run a scenario, read any metric as an Arrow table and open the recording;
- write a missing model in Python and have the engine call it, including an MA decision policy (`v2xw.run(..., ma_policy=obj)`).

## Scenarios and cities

[`scenarios/`](scenarios/) holds about 50 ready-made scenarios, covering:
- traffic: Manhattan and Portland, rush hour and peak load, pedestrians, highway congestion;
- radio: DSRC, LTE-V2X and NR-V2X comparisons;
- the SCMS: the credential lifecycle on the real calendar, revocation latency, pseudonym strategies, attacks, threshold authorities, post-quantum signatures and the European CCMS.

Any scenario can run on any of the 64 cities with `--city`. City maps are © OpenStreetMap contributors (ODbL 1.0).

A scenario is one YAML file holding the world, traffic, radio, network, messages, security, nodes, attackers, metrics and seed. Every field is validated when it loads, and a combination in which a setting would have no effect is refused with a plain message. [`configuration-reference.md`](docs/guides/configuration-reference.md) lists every setting with its default, range and source; [`config-inventory-scms.md`](docs/guides/config-inventory-scms.md) covers the SCMS settings in depth.

## How it works

One discrete-event kernel ([`v2xw-core`](crates/v2xw-core/)) holds every future event in a single queue ordered by (time, priority, sequence) and jumps from one event to the next. Time is counted in nanoseconds, and the engine never reads the wall clock.

- **Traffic** is one recurring event that moves every vehicle and pedestrian every 100 ms.
- **Radio events** happen at their exact microsecond times in between. Each device generates and signs its messages, waits for the channel and transmits, and every receiver decides whether it decoded the frame.
- **The SCMS backend** ([`v2xw-proto`](crates/v2xw-proto/)) runs on its own event kernel, advanced in step with the vehicles. Its messages travel over the modelled cellular and backhaul links.
- **An actor is a body that moves; a node is a radio device.** Nodes only know what their GPS and their received messages tell them. Metrics compare that belief with the ground truth.
- **Every random number** comes from a stream keyed by (seed, purpose, entity), so results do not depend on thread count or order. Parallel work inside an event is merged back in ID order.
- **Every record** passes through one path that feeds both the recording and the metric providers, so metrics come only from recorded events.

The full architecture is in [`02-architecture.md`](docs/design/02-architecture.md), and the decisions behind it are in [`docs/adr/`](docs/adr/).

## Validation

- **Foundations:** a two-vehicle run in which every metric was computed by hand ([`foundations.md`](docs/validation/foundations.md)).
- **Network and traffic suites:** each layer is held to its reference and each traffic model to its paper and field data, and every check is proved able to fail ([`v2xw-netval`](crates/v2xw-netval/), [`v2xw-trafficval`](crates/v2xw-trafficval/)).
- **Cross-validation:** traffic against SUMO on the same networks, and radio against Veins, OpenCV2X, Artery and published curves ([`cross-validation.md`](docs/validation/cross-validation.md), [`radio-fidelity.md`](docs/validation/radio-fidelity.md)).
- **Sensitivity:** a sweep of every setting, plus an expectation table of more than 150 rows, each with its source, that checks settings move metrics the way theory says ([`sensitivity-expectations.md`](docs/validation/sensitivity-expectations.md)).
- **Security:**
  - official IEEE 1609.2a, CAMP and NIST test vectors;
  - an independent Python oracle written without reading the Rust ([`tools/oracle/`](tools/oracle/));
  - the J2735 SPaT and MAP encoders, byte-checked against an ASN.1 compiler built from the public modules;
  - cross-checks against the real trust lists and certificate chains that SCMS Manager publishes ([`vectors-provenance.md`](docs/design/scms-real/vectors-provenance.md)).
- **Real-world SCMS cross-check:** each SCMS rule is graded against the deployment ([`04-validation-report.md`](docs/design/scms-real/04-validation-report.md), [`06-validation-summary.md`](docs/design/scms-real/06-validation-summary.md)).
- **Determinism:** the same scenario and seed give byte-identical runs on 1 and 8 threads.

All reports are indexed in [`docs/validation/README.md`](docs/validation/README.md).

## Repository layout

| Path | Contents |
|---|---|
| [`crates/v2xw-core`](crates/v2xw-core/) | Event kernel: clock, event queue, keyed RNG streams, registry, manifest |
| [`crates/v2xw-engine`](crates/v2xw-engine/) | Scenario schema and loader, the main loop, wiring, the SCMS bridge, MA policies, sensitivity tools |
| [`crates/v2xw-world`](crates/v2xw-world/) | Map model, OpenStreetMap and SUMO importers, procedural grids, spatial indices |
| [`crates/v2xw-mobility`](crates/v2xw-mobility/) | Traffic: car-following, lane changing, junctions, signals, pedestrians, GNSS, meso tier |
| [`crates/v2xw-demand`](crates/v2xw-demand/), [`v2xw-demand-cli`](crates/v2xw-demand-cli/) | Demand and route choice: OD matrices, traffic assignment, routes from counts, GTFS |
| [`crates/v2xw-radio`](crates/v2xw-radio/) | Propagation, fading, shadowing, PHY, MAC, congestion control, sidelink, cellular |
| [`crates/v2xw-net`](crates/v2xw-net/) | WSMP, GeoNetworking and BTP, fragmentation, byte accounting |
| [`crates/v2xw-msg`](crates/v2xw-msg/) | ASN.1 encoders (J2735, ETSI, IEEE 1609.2, 1609.2.1 and 1609.3), message generators and size models |
| [`crates/v2xw-sec`](crates/v2xw-sec/) | IEEE 1609.2 security: certificates, chains, ECIES, CCM, butterfly keys, linkage, CRL and CTL, cost tables |
| [`crates/v2xw-node`](crates/v2xw-node/) | Device runtimes for OBUs, RSUs and pedestrian devices: queues, signer and verifier, stores, safety applications, P2PCD |
| [`crates/v2xw-proto`](crates/v2xw-proto/) | Protocol host: the SCMS and CCMS entities, flows, governance, deployment, sites and capacity, threshold authorities |
| [`crates/v2xw-threat`](crates/v2xw-threat/) | Attackers, jammers, misbehaviour detectors, SM-MBR reports, the misbehaviour authority and its policies, privacy observer |
| [`crates/v2xw-metrics`](crates/v2xw-metrics/) | Metric providers and their definitions |
| [`crates/v2xw-record`](crates/v2xw-record/) | Recording (MCAP), Parquet and dataset exporters |
| [`crates/v2xw-energy`](crates/v2xw-energy/) | Emission, fuel and EV energy models |
| [`crates/v2xw-experiment`](crates/v2xw-experiment/) | Parameter sweeps, replications and aggregation |
| [`crates/v2xw-interop`](crates/v2xw-interop/) | SUMO TraCI co-simulation, route import and export, cross-validation |
| [`crates/v2xw-server`](crates/v2xw-server/) | JSON-RPC control and the WebSocket stream the Studio uses ([`vwp-v1.md`](docs/protocol/vwp-v1.md)) |
| [`crates/v2xw-copilot`](crates/v2xw-copilot/) | The built-in agent and its evaluation suite |
| [`crates/v2xw-cli`](crates/v2xw-cli/) | The `v2xw` and `v2xw-server` binaries |
| [`crates/v2xw-netval`](crates/v2xw-netval/), [`v2xw-trafficval`](crates/v2xw-trafficval/) | Network and traffic validation suites |
| [`crates/v2xw-py`](crates/v2xw-py/), [`v2xw-wasm`](crates/v2xw-wasm/) | Python bindings and the WebAssembly replay reader |
| [`ui/`](ui/) | pnpm workspace: [`apps/studio`](ui/apps/studio/) (the Studio), [`packages/viewer`](ui/packages/viewer/) (3D), [`packages/protocol`](ui/packages/protocol/), [`packages/mock-server`](ui/packages/mock-server/) |
| [`python/`](python/) | The `v2xw` Python package, its examples and tests |
| [`scenarios/`](scenarios/) | Ready-made scenarios |
| [`worlds/`](worlds/) | The city library and imported map caches |
| [`crates/v2xw-node/profiles/hardware/`](crates/v2xw-node/profiles/hardware/) | OBU, RSU, pedestrian-device and backend hardware profiles, each number cited |
| [`examples/`](examples/), [`plugins/`](plugins/) | Worked examples of extending the engine with your own models |
| [`tools/oracle/`](tools/oracle/) | The independent Python oracle for the security code |
| [`third_party/`](third_party/) | The standards' ASN.1 modules, with provenance |
| [`docs/`](docs/) | Design, guides, validation reports, model cards, decision records |
| [`legacy/`](legacy/) | The original Python reference, kept for its conformance vectors |

## Development

The [`justfile`](justfile) holds the common tasks:

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
- **Citations:** every constant cites its source or is labelled as an assumption ([`constants.md`](docs/design/scms-real/constants.md) for the SCMS).
- **Failing checks:** every test is shown to fail when its fault is injected.
- **Settings:** a new option is a validated, published scenario field that actually changes the run, and it gets a menu, labels and units in the Studio.
- **Goldens:** the default path's recorded digests change only through an explicit re-bless that names its cause.

## Documentation

| Where | What |
|---|---|
| [`docs/design/`](docs/design/) | Architecture, models, protocols, measurement and the [build status](docs/design/BUILD-STATUS.md) |
| [`docs/design/scms-real/`](docs/design/scms-real/) | The SCMS realism work (see below) |
| [`docs/guides/`](docs/guides/) | [Configuration reference](docs/guides/configuration-reference.md), parameter inventories ([traffic](docs/guides/config-inventory-traffic.md), [network](docs/guides/config-inventory-network.md), [SCMS](docs/guides/config-inventory-scms.md)), [comparing runs](docs/guides/compare-runs.md), [signature schemes](docs/guides/signature-schemes.md) |
| [`docs/validation/`](docs/validation/) | Every validation, sensitivity and cross-validation report |
| [`docs/models/`](docs/models/) | Model cards for the driving models and driver heterogeneity |
| [`docs/benchmark/`](docs/benchmark/), [`docs/calibration/`](docs/calibration/) | The SCMS benchmark and the traffic calibration data |
| [`docs/protocol/`](docs/protocol/) | The engine-to-Studio wire protocol and its vectors |
| [`docs/redteam/`](docs/redteam/) | The PKI privacy review |
| [`docs/adr/`](docs/adr/) | Architecture decision records |
| [`docs/site/`](docs/site/) | Generator for a browsable site built from the model cards and schema |

The SCMS documents, in reading order:

| Document | What it covers |
|---|---|
| [`00-gap-analysis-and-plan.md`](docs/design/scms-real/00-gap-analysis-and-plan.md) | Where the engine differed from the real US SCMS, and the build plan |
| [`01-deployment-and-governance.md`](docs/design/scms-real/01-deployment-and-governance.md) | The US SCMS today: providers, governance, trust lists |
| [`02-cryptography-and-formats.md`](docs/design/scms-real/02-cryptography-and-formats.md) | Certificates, keys, butterfly expansion, linkage values, CRL formats |
| [`03-protocols-and-operations.md`](docs/design/scms-real/03-protocols-and-operations.md) | IEEE 1609.2.1 flows and the real operating parameters |
| [`04-validation-report.md`](docs/design/scms-real/04-validation-report.md) | The real-world cross-check, rule by rule |
| [`05-scale-and-regression.md`](docs/design/scms-real/05-scale-and-regression.md) | Scale, regression and test time |
| [`06-conformance-run.md`](docs/design/scms-real/06-conformance-run.md), [`06-validation-summary.md`](docs/design/scms-real/06-validation-summary.md) | The end-to-end conformance run, and how close the engine is to the deployment |
| [`07-config-testing-report.md`](docs/design/scms-real/07-config-testing-report.md) | Every SCMS setting run and checked |
| [`08-settings-ux-and-gaps.md`](docs/design/scms-real/08-settings-ux-and-gaps.md) | A menu for every SCMS setting; server sites, capacity, MA policies and deeper hierarchies |
| [`09-completion-plan.md`](docs/design/scms-real/09-completion-plan.md), [`10-completion-specs.md`](docs/design/scms-real/10-completion-specs.md) | The completion rounds and their specs |
| [`11-j2945-1-conformance.md`](docs/design/scms-real/11-j2945-1-conformance.md) | The SAE J2945/1 conformance check and its fixes |
| [`constants.md`](docs/design/scms-real/constants.md), [`vectors-provenance.md`](docs/design/scms-real/vectors-provenance.md) | Every SCMS constant with its source, and where each test vector came from |

## Licensing

Code is Apache-2.0 ([`LICENSE`](LICENSE)), and generated datasets are CC-BY-4.0. GPL tools (Veins, Artery, F2MD, ns-3) are read as references and re-implemented, never linked. SUMO is optional and runs out of process. City maps are © OpenStreetMap contributors under ODbL 1.0. Published SCMS Manager artefacts are fetched for tests, not redistributed. To cite the simulator, see [`CITATION.cff`](CITATION.cff).
