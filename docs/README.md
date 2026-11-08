# Documentation

Each page describes the code as it is and names the files it describes. Where a page and
the code disagree, the code (and its rustdoc) is authoritative.

These pages are also published as a website (https://ak3zaidan.github.io/V2X-World/),
whose home page is [index.md](index.md). The site adds tutorials and guides under
[getting-started/](getting-started/) (introduction, first simulation, Studio tour, the
command-line reference, Python) and [guide/](guide/) (worlds, demand, running and recording,
comparing runs, sweeps, sensitivity, exports, SUMO, the assistant), and two pages generated
from the engine: the configuration reference ([configuration/reference/](configuration/reference/))
and the [metric catalogue](metrics/catalogue.md).

## Reading order

For a first pass, read in this order:

1. [getting-started.md](getting-started.md): build, run, inspect, serve.
2. [architecture.md](architecture.md): the layers, the event loop, the determinism contract.
3. [crates.md](crates.md): what each crate owns and depends on.
4. [scenarios.md](scenarios.md): the one input file of a run.
5. The model page for your area, under [models/](#models).
6. [recording-and-metrics.md](recording-and-metrics.md): what a run leaves behind.
7. [extending.md](extending.md): where a new model, detector, metric, setting or scenario goes.

## Pages

| Page | Contents |
|---|---|
| [getting-started.md](getting-started.md) | prerequisites, build, checking the install, task-runner recipes |
| [architecture.md](architecture.md) | layering, data flow, the main loop, tiers, determinism, firewall |
| [crates.md](crates.md) | every workspace crate, its entry types and its dependencies |
| [scenarios.md](scenarios.md) | scenario format, loading pipeline, overlays, cities, sweeps, shipped scenarios |
| [configuration.md](configuration.md) | where settings are defined, validated, published; status classes |
| [recording-and-metrics.md](recording-and-metrics.md) | run outputs, MCAP channels, digests, metric families, statistics, exporters |
| [studio.md](studio.md) | the browser UI, its packages and how it talks to the engine |
| [testing.md](testing.md) | unit and integration tests, the conformance kit, validation suites, CI |
| [extending.md](extending.md) | adding models, detectors, metrics, settings, scenarios, signature schemes |
| [glossary.md](glossary.md) | acronyms and terms |

### Models

| Page | Contents |
|---|---|
| [models/traffic.md](models/traffic.md) | world import, car-following, lane change, intersections, signals, VRUs, demand, GNSS and clocks, energy |
| [models/network.md](models/network.md) | propagation, fading, obstacles, PHY, MAC, sidelink, DCC, cellular, network layer, fragmentation, messages |
| [models/security.md](models/security.md) | the 1609.2 envelope, crypto backends, signature schemes, node security, verification, P2PCD |
| [models/scms.md](models/scms.md) | the CAMP SCMS, the ETSI ITS PKI, threshold protocols, pseudonym change |
| [models/threats.md](models/threats.md) | attackers, jammers, detectors, misbehaviour authority, privacy observer |

### Reference

| Page | Contents |
|---|---|
| [protocol/vwp-v1.md](protocol/vwp-v1.md) | VWP v1: the engine-to-browser wire protocol and JSON-RPC control plane |
| [dataset/schema.md](dataset/schema.md) | the dataset release schema |
| [reference/configuration-reference.md](reference/configuration-reference.md) | scenario settings with paths, defaults, ranges and sources |
| [reference/config-inventory-traffic.md](reference/config-inventory-traffic.md) | traffic, world and run parameters |
| [reference/config-inventory-network.md](reference/config-inventory-network.md) | radio and network parameters |
| [reference/config-inventory-scms.md](reference/config-inventory-scms.md) | security and credential-system parameters |
| [reference/sensitivity-catalogue.md](reference/sensitivity-catalogue.md) | the catalogue-wide one-at-a-time sensitivity sweep |
| [reference/sensitivity-expectations.md](reference/sensitivity-expectations.md) | expected metric directions per setting, with their theory |
