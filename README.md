<p align="center">
  <img alt="V2X World logo" src=".github/assets/logo/v2x-world.svg" width="112">
</p>

<h1 align="center">V2X World</h1>

<p align="center">
  <b>A connected-vehicle simulator with a real US SCMS.</b> Road traffic, the radio channel, the network,
  IEEE 1609.2 security and the US Security Credential Management System, in one deterministic engine.
</p>

<p align="center">
  <a href="https://github.com/ak3zaidan/V2X-World/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ak3zaidan/V2X-World/ci.yml?branch=main&label=CI&logo=github"></a>
  <a href="https://github.com/ak3zaidan/V2X-World/actions/workflows/docs.yml"><img alt="Docs" src="https://img.shields.io/github/actions/workflow/status/ak3zaidan/V2X-World/docs.yml?branch=main&label=docs&logo=materialformkdocs"></a>
  <a href="LICENSE"><img alt="Code licence: Apache-2.0" src="https://img.shields.io/badge/code-Apache--2.0-blue"></a>
  <a href="https://creativecommons.org/licenses/by/4.0/"><img alt="Data licence: CC BY 4.0" src="https://img.shields.io/badge/data-CC%20BY%204.0-lightgrey"></a>
  <a href="rust-toolchain.toml"><img alt="Rust 1.98.1" src="https://img.shields.io/badge/rust-1.98.1-orange?logo=rust"></a>
  <a href="https://arxiv.org/abs/XXXX.XXXXX"><img alt="Paper (coming soon)" src="https://img.shields.io/badge/paper-coming%20soon-b31b1b?logo=arxiv"></a>
  <a href="https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini"><picture><source media="(prefers-color-scheme: dark)" srcset="https://huggingface.co/datasets/huggingface/badges/resolve/main/dataset-on-hf-sm-dark.svg"><img alt="Dataset on Hugging Face" src="https://huggingface.co/datasets/huggingface/badges/resolve/main/dataset-on-hf-sm.svg"></picture></a>
</p>

<p align="center">
  <a href="https://ak3zaidan.github.io/V2X-World/"><img alt="Website" src=".github/assets/readme/btn-website.svg" width="128"></a>
  <a href="https://github.com/ak3zaidan/V2X-World/releases"><img alt="Download" src=".github/assets/readme/btn-download.svg" width="128"></a>
  <a href="https://ak3zaidan.github.io/V2X-World/getting-started/introduction/"><img alt="Docs" src=".github/assets/readme/btn-docs.svg" width="128"></a>
  <a href="https://arxiv.org/abs/XXXX.XXXXX"><img alt="Paper" src=".github/assets/readme/btn-paper.svg" width="128"></a>
  <a href="https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini"><img alt="Dataset" src=".github/assets/readme/btn-dataset.svg" width="128"></a>
  <a href="https://ak3zaidan.github.io/V2X-World/try/"><img alt="Try online" src=".github/assets/readme/btn-try.svg" width="128"></a>
</p>

https://github.com/user-attachments/assets/527e9bd5-5d25-4480-9bf7-0818f529f154

## Overview

V2X World is a deterministic, discrete-event simulator of connected-vehicle (V2X)
systems, written in Rust. One run models the road traffic on a real city map, the radio
channel and the network on top of it, the messages and safety applications, the IEEE 1609.2
security envelope, and the credential system that issues and revokes certificates: the US
SCMS as deployed, or the European ETSI ITS PKI. Attackers, on-vehicle misbehaviour detection
and the misbehaviour authority run in the same loop. A browser Studio shows the run in 2D and
3D, and a Python package drives the engine from scripts and notebooks.

It is built for research on V2X security and privacy: what credential management costs in
bytes, queueing and latency; how long a misbehaving vehicle stays believed; what a roadside
eavesdropper learns across pseudonym changes; what post-quantum signatures cost on the air.
Every model states its equations, its defaults and the source of each default, and every
run is byte-identical for the same scenario, seed and build, on any supported platform and
any number of threads.

## Key features

| Area | What is modelled |
|---|---|
| **Traffic** | Cars, trucks, buses, motorcycles, cyclists and pedestrians on 64 OpenStreetMap city maps ([`worlds/cities.yaml`](worlds/cities.yaml)), SUMO `net.xml` networks or procedural grids. IDM, EIDM, Krauss, Gipps, Wiedemann 99, ACC and CACC; MOBIL and LC2013 lane changes; fixed-time and actuated signals; a mesoscopic tier for city scale; optional SUMO co-simulation over TraCI. |
| **Radio and network** | IEEE 802.11p (DSRC), LTE-V2X and NR-V2X sidelink, and the cellular link. 3GPP TR 37.885 path loss, building obstruction, shadowing and Nakagami fading; cumulative interference; SAE J2945/1 and ETSI DCC congestion control; IEEE 1609.3 WSMP and ETSI GeoNetworking/BTP. |
| **Messages and applications** | SAE J2735 BSM, SPaT, MAP and PSM (UPER); ETSI CAM, DENM, VAM and CPM; SRM, SSM and WSA. FCW, EEBL, IMA, LTA, BSW, PCW, RLVW and GLOSA on the vehicle; signal priority at equipped intersections. |
| **Security (IEEE 1609.2)** | Real signed and verified messages: COER encoding, ECDSA P-256, implicit (ECQV) and explicit certificates, chain, validity, region, permission and revocation checks, with per-device processing times. |
| **US SCMS** | Governance with electors and certificate trust lists, several providers with their own root, ICA, ECA, RA, ACA, linkage authorities and misbehaviour authority, IEEE 1609.2.1 flows, butterfly keys, the weekly pseudonym calendar, linkage-based revocation and CRLs. What-if options, marked as such: the ETSI TS 102 941 PKI, deeper CA hierarchies, threshold authorities, hybrid and post-quantum signatures. |
| **Threats and misbehaviour detection** | Fake positions, replay, Sybil identities, jamming, compromised roadside units and a passive privacy observer. On-vehicle detectors (VeReMi/F2MD style, the SCMS Manager report profile, ETSI TS 103 759), misbehaviour reports, and configurable misbehaviour-authority policies with offline replay. |
| **Studio (2D and 3D)** | A browser interface on one Three.js scene: run controls, cameras, overlays for radio links, messages, signals and certificate states, click-to-inspect, searchable settings with plain explanations, cited defaults and units, live metrics, run comparison, certificate-system views and an optional built-in assistant. |
| **Recording and measurement** | MCAP recordings that replay byte for byte; metrics with plain definitions, sample counts and confidence intervals; Parquet, Arrow, JSONL, VeReMi and SUMO exports; parameter sweeps, same-seed comparison and sensitivity analysis. |
| **Determinism** | One event queue in integer nanoseconds, random streams keyed by (seed, purpose, entity), no wall clock. Results do not depend on thread count, and CI fails unless Linux, macOS and Windows produce the same digests. |
| **Python** | The `v2xw` package: load and run scenarios, read metrics as Arrow tables, open recordings, and write car-following models, detectors or misbehaviour-authority policies in Python. |

Each model has a model card with its equations, parameters, assumptions and validation:
see [Models](https://ak3zaidan.github.io/V2X-World/models/) in the documentation.

## Recommended system

| Component | Requirement |
|---|---|
| **Operating system** | Linux, macOS and Windows: CI builds and tests on `ubuntu-latest`, `macos-latest` and `windows-latest` ([`ci.yml`](.github/workflows/ci.yml)). The helper scripts ([`run-sim.sh`](run-sim.sh), [`worlds/fetch.sh`](worlds/fetch.sh)) are Bash; on Windows, run the cargo commands they wrap. |
| **Rust** | 1.98.1, pinned in [`rust-toolchain.toml`](rust-toolchain.toml). Install [rustup](https://rustup.rs); it installs the pinned compiler, `rustfmt`, `clippy` and the `wasm32-unknown-unknown` target on first use. A C compiler is also needed (the recording's zstd codec is C). |
| **Studio** | Node 22 or newer and pnpm 10 ([`ui/package.json`](ui/package.json) pins `pnpm@10.15.0`; `corepack` can provide it). |
| **Python bindings** | Python 3.9 or newer and [maturin](https://www.maturin.rs). |
| **Optional** | [`just`](https://just.systems) for the task recipes; [Eclipse SUMO](https://eclipse.dev/sumo/) for co-simulation and cross-validation. |
| **Memory** | 16 GB or more is recommended for building. A parallel release build of the whole workspace can exhaust 8 GB; on an 8 GB machine build one job at a time (`-j 1`, as `just build-cli` and `run-sim.sh` do). At run time a full-scale Manhattan run peaked at about 1.9 GB of engine memory, roughly 1.35 MB per simulated node, so memory grows with the fleet. |
| **Disk** | A release build takes a few GB under `target/`, and debug and test builds of the workspace take several GB more; keep 10 GB or more free. City extracts are a few MB to tens of MB each, and recordings grow with run length and record level. |

## Quick start

Build the command-line tool and run a scenario on a procedural road network, with no
download:

```sh
git clone https://github.com/ak3zaidan/V2X-World.git
cd V2X-World

cargo build --release -p v2xw-cli          # rustup installs the pinned toolchain first
./target/release/v2xw validate scenarios/highway-congestion.yaml
./target/release/v2xw run scenarios/highway-congestion.yaml --out runs/highway
./target/release/v2xw info runs/highway/recording.mcap
```

Run on a real city. Each city's OpenStreetMap extract is downloaded once into
`worlds/cache/` (git-ignored):

```sh
./target/release/v2xw cities               # the city library
worlds/fetch.sh manhattan                  # writes worlds/cache/manhattan.osm.xml
./target/release/v2xw run scenarios/midtown-normal.yaml --out runs/midtown

worlds/fetch.sh boston                     # the same scenario on another city
./target/release/v2xw run scenarios/midtown-normal.yaml --city boston --out runs/boston
./target/release/v2xw compare runs/midtown runs/boston
```

Open the Studio: the engine in one terminal, the page in another.

```sh
./run-sim.sh                               # the engine on :8787 (built on first use)
pnpm --dir ui install
pnpm --dir ui --filter @vwp/studio dev     # then open http://127.0.0.1:5173
```

> [!NOTE]
> `run-sim.sh` serves `scenarios/manhattan-5min.yaml` unless you name another scenario, so
> fetch the Manhattan extract first (`worlds/fetch.sh manhattan`). For browser work without a map, serve a synthetic stream
> with `./target/release/v2xw-server --actors 200 --port 8787`.

> [!TIP]
> The built-in assistant is optional. To use it, put `V2XW_AGENT_PROVIDER` (`openai` or
> `anthropic`) and `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` in a `.env` file in the
> repository root; it is git-ignored, and the engine never prints the keys. Everything else
> works without them.

With [`just`](justfile): `just run <scenario>`, `just validate <scenario>`,
`just serve <scenario>`, `just studio`, and `just ci` for the checks CI runs. Build the
Python package with `cd python && maturin develop` inside a virtual environment
([`python/README.md`](python/README.md)).

## Documentation

The documentation site is at **https://ak3zaidan.github.io/V2X-World/**. It is
built from [`docs/`](docs/README.md) in this repository.

| Page | What it covers |
|---|---|
| [Introduction](https://ak3zaidan.github.io/V2X-World/getting-started/introduction/) | What the simulator is for and how the parts fit together |
| [Install and build](https://ak3zaidan.github.io/V2X-World/getting-started/) | Prerequisites, builds, the `v2xw` command and the server |
| [Your first simulation](https://ak3zaidan.github.io/V2X-World/getting-started/first-simulation/) | A first run from scenario to metrics |
| [Studio tour](https://ak3zaidan.github.io/V2X-World/getting-started/studio-tour/) | The browser interface, panel by panel |
| [Configuration reference](https://ak3zaidan.github.io/V2X-World/configuration/reference/) | Every scenario setting with its default, range, unit and source |
| [SCMS and credentials](https://ak3zaidan.github.io/V2X-World/models/scms/) | The credential-management model and how it follows the US deployment |
| [Extending](https://ak3zaidan.github.io/V2X-World/extending/) | Adding your own models, detectors, metrics and settings |
| [API](https://ak3zaidan.github.io/V2X-World/api/) | The Rust API and the Python API |

## Dataset

<p>
  <a href="https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini"><picture><source media="(prefers-color-scheme: dark)" srcset="https://huggingface.co/datasets/huggingface/badges/resolve/main/dataset-on-hf-lg-dark.svg"><img alt="V2X-SCMS-MBD on Hugging Face" src="https://huggingface.co/datasets/huggingface/badges/resolve/main/dataset-on-hf-lg.svg"></picture></a>
</p>

[**V2X-SCMS-MBD**](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini) is the dataset built
with this simulator, for misbehaviour detection and misbehaviour-authority research. Its
schema publishes each run as two observer views of the same traffic plus the answers:

- **local**: what each vehicle's on-board unit sees (received messages, checks that fired,
  reports it filed);
- **global**: what the misbehaviour authority sees (decrypted reports, decisions, CRL
  entries);
- **truth**: the ground truth, kept out of the observer files (who really sent what, which
  attack, what happened next).

Tables are Apache Parquet, splits are by run rather than by message, and every run can be
regenerated from its scenario file and seed: the simulator's `dataset` exporter writes one
run, and `tools/dataset/` assembles, checks and publishes a release. The full schema is in
[`docs/dataset/schema.md`](docs/dataset/schema.md).

> [!NOTE]
> The published release, v1.1 (`v2x-scms-mbd-mini`), is a one-run sample: 60 vehicles for
> 75 s on a street grid with the US SCMS in the loop, ten attacks and faulty honest devices.
> The eleven-run full release is planned. The dataset is free for anyone to use, share and
> adapt under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/); please cite it.

## Paper and citation

A paper describing the simulator is in preparation; the link and the entry below will be
filled in when it is available.

```bibtex
@misc{zaidan_v2xworld_paper,
  title         = {TODO: paper title},
  author        = {Zaidan, Ahmed},
  year          = {TODO},
  eprint        = {XXXX.XXXXX},
  archivePrefix = {arXiv},
  url           = {https://arxiv.org/abs/XXXX.XXXXX}
}
```

To cite the software, use [`CITATION.cff`](CITATION.cff) (GitHub's "Cite this repository"
button reads it) or:

```bibtex
@software{zaidan_v2xworld,
  title   = {{V2X World}},
  author  = {Zaidan, Ahmed},
  version = {0.1.0},
  year    = {2026},
  license = {Apache-2.0},
  url     = {https://github.com/ak3zaidan/V2X-World}
}
```

## Repository layout

| Path | Contents |
|---|---|
| [`crates/`](crates/) | The Rust workspace: the event kernel, the model crates, the CLI and server, the Python and WebAssembly bindings ([`docs/crates.md`](docs/crates.md)) |
| [`ui/`](ui/) | The TypeScript workspace: the Studio, the protocol client, the Three.js viewer, a mock server |
| [`python/`](python/) | The `v2xw` Python package, built with maturin from `crates/v2xw-py` |
| [`scenarios/`](scenarios/) | Ready-made and study scenarios (YAML, schema `v2xw/scenario/1`) |
| [`worlds/`](worlds/) | The city library, extract download scripts, SUMO networks and terrain |
| [`examples/`](examples/) | Worked plug-ins, one per extension interface |
| [`tests/`](tests/) | The conformance kit and committed fixtures |
| [`tools/`](tools/) | Validation and conformance tools, including an independent Python oracle for the security code |
| [`third_party/`](third_party/) | Committed ASN.1 modules with their licences and provenance |
| [`docs/`](docs/) | The documentation sources ([`docs/README.md`](docs/README.md)) |

## Contributing

Bug reports and questions are welcome on the
[issue tracker](https://github.com/ak3zaidan/V2X-World/issues). For a larger change,
open an issue first to agree the approach. Before sending a pull request, run `just ci`
(formatting, clippy with warnings denied, and the test suite, as CI does). The codebase
keeps a few rules: results must stay deterministic (keyed random streams, ordered
collections, no wall clock), every constant cites its source or is labelled an assumption,
and every new setting is a validated scenario field with a menu, labels and units in the
Studio. [`docs/testing.md`](docs/testing.md) and [`docs/extending.md`](docs/extending.md)
explain the tests and the extension points, and the
[contributing guide](https://ak3zaidan.github.io/V2X-World/dev/contributing/) has the
details.

## Licence

The source code is licensed under the [Apache License 2.0](LICENSE). The
[V2X-SCMS-MBD dataset](https://huggingface.co/datasets/akzaidan/v2x-scms-mbd-mini) is licensed under
[Creative Commons Attribution 4.0 (CC BY 4.0)](https://creativecommons.org/licenses/by/4.0/):
anyone may use, share and adapt it, for any purpose, with credit. Third-party material in
the repository keeps its own terms:

| Material | Where | Terms |
|---|---|---|
| ETSI ITS ASN.1 modules (CDD, CAM, DENM, VAM, CPM, TS 103 097) | [`third_party/asn1/etsi/`](third_party/asn1/etsi/PROVENANCE.md) | BSD-3-Clause, © ETSI; a `LICENSE.txt` sits beside each module set |
| IEEE 1609.2, 1609.2.1 and 1609.3 ASN.1 modules (with the ISO 17419 identifiers module) | [`third_party/asn1/etsi/`](third_party/asn1/etsi/PROVENANCE.md), [`third_party/asn1/ieee1609.3/`](third_party/asn1/ieee1609.3/PROVENANCE.md), `tools/oracle/asn1/` | IEEE copyright, taken from the ETSI forge and the Certification Operating Council mirrors. No licence is stated upstream; they are committed as the public interoperability definitions, and anyone redistributing them should confirm the terms with IEEE |
| OpenStreetMap map fixture | [`tests/fixtures/midtown-6block.osm.xml`](tests/fixtures/midtown-6block.osm.xml) | © OpenStreetMap contributors, [ODbL 1.0](https://www.openstreetmap.org/copyright) |
| City extracts | `worlds/cache/`, downloaded by [`worlds/fetch.sh`](worlds/fetch.sh), not committed | © OpenStreetMap contributors, [ODbL 1.0](https://www.openstreetmap.org/copyright) |
| LuST scenario excerpts | [`worlds/sumo/`](worlds/sumo/lust-veremi-sub.PROVENANCE.md) (`lust-veremi-sub.net.xml`, `lust-sample.rou.xml`) | MIT, © 2016 Lara Codeca, VehicularLab; the network also carries the OpenStreetMap ODbL attribution |
| Terrain cut for Portland | [`worlds/dem/portland-downtown.asc`](worlds/dem/portland-downtown.md) | From the AWS Terrain Tiles (SRTM or USGS 3DEP): "terrain data courtesy of the U.S. Geological Survey" |
| Calibration statistics | [`data/calibration/`](data/calibration/) | Derived statistics only, fitted on public USDOT ITS connected-vehicle pilot data (data.transportation.gov); each file names its source queries |
| SCMS Manager published trust lists and certificates | [`third_party/scms-artefacts/`](third_party/scms-artefacts/PROVENANCE.md) (provenance only; the files are not committed) | © SCMS Manager LLC, all rights reserved; fetched for tests only, by `tools/scms-artefacts/fetch.py` |

> [!NOTE]
> The SAE J2735 ASN.1 modules are not redistributed: their embedded licence forbids it.
> The J2735 codecs in [`crates/v2xw-msg`](crates/v2xw-msg/) are written by hand and checked
> against an ASN.1 compiler run on a copy you supply yourself (`third_party/asn1/j2735/` is
> git-ignored).

Rust crates and npm packages fetched at build time keep their own licences
([`Cargo.lock`](Cargo.lock), [`ui/pnpm-lock.yaml`](ui/pnpm-lock.yaml)). Eclipse SUMO is
optional, not bundled, and runs as a separate process.

## Acknowledgements

The engine implements, or models, published specifications: IEEE 1609.2, 1609.2.1, 1609.3
and 1609.4; IEEE 802.11p; SAE J2735 and J2945/1; the ETSI ITS specifications for CAM, DENM,
VAM, CPM, GeoNetworking, decentralised congestion control, security (TS 103 097,
TS 102 941) and misbehaviour detection (TS 103 759); 3GPP TR 37.885; NTCIP 1211 and
CTI 4501. It uses map data from OpenStreetMap contributors, public connected-vehicle pilot
data from the US Department of Transportation, and the LuST scenario, and it is
cross-checked against SUMO and published results. Naming a standard, an organisation or a
project here does not imply that it endorses this simulator.
