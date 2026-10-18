# Search log: cluster `sim` (V2X simulators, co-simulation, digital twins, datasets)

Date of all queries: 2026-10-17. Hits = records returned (first page(s): OpenAlex up to 200, Semantic Scholar 100, arXiv 100, Crossref 100, DBLP 200).
Kept = returned records that met the inclusion criteria for this cluster (before cross-query deduplication).

## Source availability

- **OpenAlex**: unavailable for the whole session. Every call returned HTTP 429 with a daily-budget message (free credit exhausted; a key requires registering an email, which the privacy rule forbids). Logged as `n/a`.
- **DBLP**: unavailable. The API answered with an Anubis anti-bot challenge page (HTTP 202 HTML) instead of JSON. Logged as `n/a`.
- **Semantic Scholar**: heavily rate-limited (shared unauthenticated pool); calls retried with exponential backoff.
- **arXiv**: intermittent HTTP 429; retried with backoff. Queries use field syntax (`all:`, `ti:`).
- **Crossref**: fully available (`query.bibliographic`).
- **OpenCitations** (COCI/Index v2) was used for reference/citation snowballing by DOI, with Crossref for metadata, because Semantic Scholar's graph endpoints were rate-limited.

## Queries

| # | Source | Query | Hits | Kept |
|---|---|---|---|---|
| 1 | OpenAlex | `Veins vehicular network simulation framework` | 0 | 0 |
| 1 | SemanticScholar | `Veins vehicular network simulation framework` | 100 | 37 |
| 1 | arXiv | `all:Veins AND all:vehicular` | 22 | 4 |
| 1 | Crossref | `Veins vehicular network simulation framework` | 100 | 40 |
| 1 | DBLP | `Veins vehicular simulation` | 0 | 0 |
| 2 | OpenAlex | `Artery V2X simulation framework ETSI ITS-G5` | 0 | 0 |
| 2 | SemanticScholar | `Artery V2X simulation framework ETSI ITS-G5` | 51 | 18 |
| 2 | arXiv | `all:Artery AND all:V2X` | 1 | 1 |
| 2 | Crossref | `Artery V2X simulation framework ETSI ITS-G5` | 100 | 28 |
| 2 | DBLP | `Artery V2X simulation` | 0 | 0 |
| 3 | OpenAlex | `Vanetza ETSI C-ITS protocol stack` | 0 | 0 |
| 3 | SemanticScholar | `Vanetza ETSI C-ITS protocol stack` | 100 | 4 |
| 3 | arXiv | `all:Vanetza` | 0 | 0 |
| 3 | Crossref | `Vanetza ETSI C-ITS protocol stack` | 100 | 0 |
| 3 | DBLP | `Vanetza` | 0 | 0 |
| 4 | OpenAlex | `OpenCV2X LTE-V2X mode 4 simulation model` | n/a | 0 |
| 4 | SemanticScholar | `OpenCV2X LTE-V2X mode 4 simulation model` | 0 | 0 |
| 4 | arXiv | `all:"C-V2X" AND all:"mode 4" AND all:simulation` | 20 | 2 |
| 4 | Crossref | `OpenCV2X LTE-V2X mode 4 simulation model` | 100 | 11 |
| 4 | DBLP | `OpenCV2X` | n/a | 0 |
| 5 | OpenAlex | `SimuLTE Simu5G vehicular network simulation OMNeT++` | n/a | 0 |
| 5 | SemanticScholar | `SimuLTE Simu5G vehicular network simulation OMNeT++` | n/a | 0 |
| 5 | arXiv | `all:Simu5G OR all:SimuLTE` | 26 | 5 |
| 5 | Crossref | `SimuLTE Simu5G vehicular network simulation OMNeT++` | 100 | 19 |
| 5 | DBLP | `Simu5G` | n/a | 0 |
| 6 | OpenAlex | `ms-van3t ns-3 V2X simulation framework` | n/a | 0 |
| 6 | SemanticScholar | `ms-van3t ns-3 V2X simulation framework` | n/a | 0 |
| 6 | arXiv | `all:"ms-van3t" OR (all:ns-3 AND all:V2X AND all:framework)` | 8 | 5 |
| 6 | Crossref | `ms-van3t ns-3 V2X simulation framework` | 100 | 30 |
| 6 | DBLP | `ms-van3t` | n/a | 0 |
| 7 | OpenAlex | `Eclipse MOSAIC VSimRTI V2X co-simulation` | n/a | 0 |
| 7 | SemanticScholar | `Eclipse MOSAIC VSimRTI V2X co-simulation` | n/a | 0 |
| 7 | arXiv | `all:VSimRTI OR (all:MOSAIC AND all:V2X)` | 4 | 0 |
| 7 | Crossref | `Eclipse MOSAIC VSimRTI V2X co-simulation` | 100 | 38 |
| 7 | DBLP | `VSimRTI` | n/a | 0 |
| 8 | OpenAlex | `VENTOS vehicular network open simulator` | n/a | 0 |
| 8 | SemanticScholar | `VENTOS vehicular network open simulator` | n/a | 0 |
| 8 | arXiv | `all:VENTOS` | 13 | 0 |
| 8 | Crossref | `VENTOS vehicular network open simulator` | 100 | 27 |
| 8 | DBLP | `VENTOS vehicular` | n/a | 0 |
| 9 | OpenAlex | `VESNOS vehicular network security simulator` | n/a | 0 |
| 9 | SemanticScholar | `VESNOS vehicular network security simulator` | 100 | 10 |
| 9 | arXiv | `all:VANET AND all:security AND ti:simulator` | 2 | 2 |
| 9 | Crossref | `VESNOS vehicular network security simulator` | 100 | 19 |
| 9 | DBLP | `vehicular security simulator` | n/a | 0 |
| 10 | OpenAlex | `ns-3 NR-V2X sidelink module 5G-LENA simulation` | n/a | 0 |
| 10 | SemanticScholar | `ns-3 NR-V2X sidelink module 5G-LENA simulation` | n/a | 0 |
| 10 | arXiv | `all:"NR-V2X" AND all:ns-3` | 7 | 1 |
| 10 | Crossref | `ns-3 NR-V2X sidelink module 5G-LENA simulation` | 100 | 3 |
| 10 | DBLP | `NR V2X ns-3` | n/a | 0 |
| 11 | OpenAlex | `ns-3 WAVE IEEE 802.11p module simulation` | n/a | 0 |
| 11 | SemanticScholar | `ns-3 WAVE IEEE 802.11p module simulation` | n/a | 0 |
| 11 | arXiv | `all:802.11p AND all:ns-3` | 3 | 0 |
| 11 | Crossref | `ns-3 WAVE IEEE 802.11p module simulation` | 100 | 8 |
| 11 | DBLP | `WAVE ns-3 802.11p` | n/a | 0 |
| 12 | OpenAlex | `iTETRIS cooperative ITS simulation platform` | n/a | 0 |
| 12 | SemanticScholar | `iTETRIS cooperative ITS simulation platform` | n/a | 0 |
| 12 | arXiv | `all:iTETRIS` | 1 | 0 |
| 12 | Crossref | `iTETRIS cooperative ITS simulation platform` | 100 | 12 |
| 12 | DBLP | `iTETRIS` | n/a | 0 |
| 13 | OpenAlex | `CARLA network simulator co-simulation V2X` | n/a | 0 |
| 13 | SemanticScholar | `CARLA network simulator co-simulation V2X` | n/a | 0 |
| 13 | arXiv | `all:CARLA AND all:V2X` | 23 | 10 |
| 13 | Crossref | `CARLA network simulator co-simulation V2X` | 100 | 28 |
| 13 | DBLP | `CARLA co-simulation` | n/a | 0 |
| 14 | OpenAlex | `OpenCDA cooperative driving automation co-simulation` | n/a | 0 |
| 14 | SemanticScholar | `OpenCDA cooperative driving automation co-simulation` | n/a | 0 |
| 14 | arXiv | `all:OpenCDA OR (all:cooperative AND all:driving AND all:co-simulation)` | 33 | 9 |
| 14 | Crossref | `OpenCDA cooperative driving automation co-simulation` | 100 | 18 |
| 14 | DBLP | `OpenCDA` | n/a | 0 |
| 15 | OpenAlex | `VaN3Twin digital twin vehicular network ns-3` | n/a | 0 |
| 15 | SemanticScholar | `VaN3Twin digital twin vehicular network ns-3` | n/a | 0 |
| 15 | arXiv | `all:VaN3Twin OR (ti:"digital twin" AND all:vehicular AND all:network)` | 37 | 30 |
| 15 | Crossref | `VaN3Twin digital twin vehicular network ns-3` | 100 | 52 |
| 15 | DBLP | `VaN3Twin` | n/a | 0 |
| 16 | OpenAlex | `V2X digital twin simulation` | n/a | 0 |
| 16 | SemanticScholar | `V2X digital twin simulation` | n/a | 0 |
| 16 | arXiv | `ti:"digital twin" AND (all:V2X OR all:vehicular)` | 54 | 45 |
| 16 | Crossref | `V2X digital twin simulation` | 100 | 28 |
| 16 | DBLP | `digital twin V2X` | n/a | 0 |
| 17 | OpenAlex | `bidirectionally coupled network and road traffic simulation SUMO OMNeT++` | n/a | 0 |
| 17 | SemanticScholar | `bidirectionally coupled network and road traffic simulation SUMO OMNeT++` | 11 | 8 |
| 17 | arXiv | `all:SUMO AND all:"OMNeT++"` | 23 | 7 |
| 17 | Crossref | `bidirectionally coupled network and road traffic simulation SUMO OMNeT++` | 100 | 23 |
| 17 | DBLP | `SUMO OMNeT++ coupling` | n/a | 0 |
| 18 | OpenAlex | `SUMO ns-3 coupling vehicular network simulation` | n/a | 0 |
| 18 | SemanticScholar | `SUMO ns-3 coupling vehicular network simulation` | n/a | 0 |
| 18 | arXiv | `all:SUMO AND all:ns-3 AND all:vehicular` | 8 | 2 |
| 18 | Crossref | `SUMO ns-3 coupling vehicular network simulation` | 100 | 42 |
| 18 | DBLP | `SUMO ns-3` | n/a | 0 |
| 19 | OpenAlex | `V2X simulator security PKI certificates` | n/a | 0 |
| 19 | SemanticScholar | `V2X simulator security PKI certificates` | 42 | 0 |
| 19 | arXiv | `all:V2X AND all:PKI AND all:simulation` | 1 | 1 |
| 19 | Crossref | `V2X simulator security PKI certificates` | 100 | 1 |
| 19 | DBLP | `V2X simulation PKI` | n/a | 0 |
| 20 | OpenAlex | `pseudonym change simulation framework VANET privacy` | n/a | 0 |
| 20 | SemanticScholar | `pseudonym change simulation framework VANET privacy` | n/a | 0 |
| 20 | arXiv | `all:pseudonym AND all:simulation AND (all:VANET OR all:vehicular)` | 4 | 1 |
| 20 | Crossref | `pseudonym change simulation framework VANET privacy` | 100 | 5 |
| 20 | DBLP | `pseudonym simulation VANET` | n/a | 0 |
| 21 | OpenAlex | `misbehavior detection simulation framework VANET F2MD` | n/a | 0 |
| 21 | SemanticScholar | `misbehavior detection simulation framework VANET F2MD` | n/a | 0 |
| 21 | arXiv | `all:misbehavior AND all:detection AND all:framework AND (all:VANET OR all:V2X)` | 5 | 0 |
| 21 | Crossref | `misbehavior detection simulation framework VANET F2MD` | 100 | 4 |
| 21 | DBLP | `misbehavior detection framework VANET` | n/a | 0 |
| 22 | OpenAlex | `VeReMi dataset misbehavior vehicular` | n/a | 0 |
| 22 | SemanticScholar | `VeReMi dataset misbehavior vehicular` | n/a | 0 |
| 22 | arXiv | `all:VeReMi` | 12 | 1 |
| 22 | Crossref | `VeReMi dataset misbehavior vehicular` | 100 | 13 |
| 22 | DBLP | `VeReMi` | n/a | 0 |
| 23 | OpenAlex | `V2X dataset simulated vehicular communication messages` | n/a | 0 |
| 23 | SemanticScholar | `V2X dataset simulated vehicular communication messages` | n/a | 0 |
| 23 | arXiv | `ti:dataset AND (all:V2X OR all:VANET OR all:vehicular)` | 45 | 38 |
| 23 | Crossref | `V2X dataset simulated vehicular communication messages` | 100 | 12 |
| 23 | DBLP | `V2X dataset` | n/a | 0 |
| 24 | OpenAlex | `validation of V2X simulation against field measurements` | n/a | 0 |
| 24 | SemanticScholar | `validation of V2X simulation against field measurements` | n/a | 0 |
| 24 | arXiv | `all:V2X AND all:simulation AND all:validation AND all:measurements` | 17 | 4 |
| 24 | Crossref | `validation of V2X simulation against field measurements` | 100 | 13 |
| 24 | DBLP | `V2X simulation validation measurements` | n/a | 0 |
| 25 | OpenAlex | `IEEE 802.11p simulation model validation measurement channel` | n/a | 0 |
| 25 | SemanticScholar | `IEEE 802.11p simulation model validation measurement channel` | n/a | 0 |
| 25 | arXiv | `all:802.11p AND all:validation AND all:simulation` | 9 | 2 |
| 25 | Crossref | `IEEE 802.11p simulation model validation measurement channel` | 100 | 4 |
| 25 | DBLP | `802.11p simulation validation` | n/a | 0 |
| 26 | OpenAlex | `VANET simulators survey comparison` | n/a | 0 |
| 26 | SemanticScholar | `VANET simulators survey comparison` | n/a | 0 |
| 26 | arXiv | `ti:VANET AND ti:simulat` | 0 | 0 |
| 26 | Crossref | `VANET simulators survey comparison` | 100 | 9 |
| 26 | DBLP | `VANET simulators survey` | n/a | 0 |
| 27 | OpenAlex | `vehicular network simulation tools review` | n/a | 0 |
| 27 | SemanticScholar | `vehicular network simulation tools review` | n/a | 0 |
| 27 | arXiv | `ti:vehicular AND ti:simulat AND ti:network` | 0 | 0 |
| 27 | Crossref | `vehicular network simulation tools review` | 100 | 31 |
| 27 | DBLP | `vehicular network simulators` | n/a | 0 |
| 28 | OpenAlex | `C-ITS cooperative intelligent transport systems simulation framework` | n/a | 0 |
| 28 | SemanticScholar | `C-ITS cooperative intelligent transport systems simulation framework` | n/a | 0 |
| 28 | arXiv | `all:"C-ITS" AND all:simulation` | 25 | 4 |
| 28 | Crossref | `C-ITS cooperative intelligent transport systems simulation framework` | 100 | 1 |
| 28 | DBLP | `C-ITS simulation` | n/a | 0 |
| 29 | OpenAlex | `connected vehicles co-simulation network traffic` | n/a | 0 |
| 29 | SemanticScholar | `connected vehicles co-simulation network traffic` | n/a | 0 |
| 29 | arXiv | `all:co-simulation AND all:vehicles` | 100 | 24 |
| 29 | Crossref | `connected vehicles co-simulation network traffic` | 100 | 15 |
| 29 | DBLP | `connected vehicles co-simulation` | n/a | 0 |
| 30 | OpenAlex | `hardware-in-the-loop V2X testbed` | n/a | 0 |
| 30 | SemanticScholar | `hardware-in-the-loop V2X testbed` | n/a | 0 |
| 30 | arXiv | `all:V2X AND all:"hardware-in-the-loop"` | n/a | 0 |
| 30 | Crossref | `hardware-in-the-loop V2X testbed` | 100 | 13 |
| 30 | DBLP | `V2X hardware-in-the-loop` | n/a | 0 |
| 31 | OpenAlex | `V2X emulation testbed real-time` | n/a | 0 |
| 31 | SemanticScholar | `V2X emulation testbed real-time` | n/a | 0 |
| 31 | arXiv | `all:V2X AND all:emulation` | 12 | 7 |
| 31 | Crossref | `V2X emulation testbed real-time` | 100 | 6 |
| 31 | DBLP | `V2X emulation` | n/a | 0 |
| 32 | OpenAlex | `C-V2X system-level simulator` | n/a | 0 |
| 32 | SemanticScholar | `C-V2X system-level simulator` | n/a | 0 |
| 32 | arXiv | `all:"C-V2X" AND ti:simulat` | 0 | 0 |
| 32 | Crossref | `C-V2X system-level simulator` | 100 | 8 |
| 32 | DBLP | `C-V2X simulator` | n/a | 0 |
| 33 | OpenAlex | `NR V2X system level simulator sidelink` | n/a | 0 |
| 33 | SemanticScholar | `NR V2X system level simulator sidelink` | n/a | 0 |
| 33 | arXiv | `all:sidelink AND ti:simulat` | 0 | 0 |
| 33 | Crossref | `NR V2X system level simulator sidelink` | 100 | 5 |
| 33 | DBLP | `NR sidelink simulator` | n/a | 0 |
| 34 | OpenAlex | `LTE-V2X simulation ns-3 module` | n/a | 0 |
| 34 | SemanticScholar | `LTE-V2X simulation ns-3 module` | n/a | 0 |
| 34 | arXiv | `all:"LTE-V2X" AND all:simulation AND all:module` | 0 | 0 |
| 34 | Crossref | `LTE-V2X simulation ns-3 module` | 100 | 13 |
| 34 | DBLP | `LTE-V2X simulation` | n/a | 0 |
| 35 | OpenAlex | `reproducibility of network simulation studies` | n/a | 0 |
| 35 | SemanticScholar | `reproducibility of network simulation studies` | n/a | 0 |
| 35 | arXiv | `all:reproducibility AND all:"network simulation"` | 66 | 13 |
| 35 | Crossref | `reproducibility of network simulation studies` | 100 | 2 |
| 35 | DBLP | `reproducibility network simulation` | n/a | 0 |
| 36 | OpenAlex | `credibility of simulation studies network simulators` | n/a | 0 |
| 36 | SemanticScholar | `credibility of simulation studies network simulators` | n/a | 0 |
| 36 | arXiv | `all:credibility AND all:simulation AND all:network` | 100 | 0 |
| 36 | Crossref | `credibility of simulation studies network simulators` | 100 | 7 |
| 36 | DBLP | `credibility simulation network` | n/a | 0 |
| 37 | OpenAlex | `realistic vehicular mobility model simulation VANET` | n/a | 0 |
| 37 | SemanticScholar | `realistic vehicular mobility model simulation VANET` | n/a | 0 |
| 37 | arXiv | `all:mobility AND all:model AND all:VANET AND ti:simulat` | 0 | 0 |
| 37 | Crossref | `realistic vehicular mobility model simulation VANET` | 100 | 32 |
| 37 | DBLP | `vehicular mobility simulation VANET` | n/a | 0 |
| 38 | OpenAlex | `Plexe platooning simulator` | n/a | 0 |
| 38 | SemanticScholar | `Plexe platooning simulator` | n/a | 0 |
| 38 | arXiv | `all:Plexe OR (all:platooning AND ti:simulat)` | 94 | 0 |
| 38 | Crossref | `Plexe platooning simulator` | 100 | 15 |
| 38 | DBLP | `Plexe platooning` | n/a | 0 |
| 39 | OpenAlex | `cooperative perception V2X simulation framework` | n/a | 0 |
| 39 | SemanticScholar | `cooperative perception V2X simulation framework` | n/a | 0 |
| 39 | arXiv | `all:"cooperative perception" AND all:simulation AND all:V2X` | 23 | 7 |
| 39 | Crossref | `cooperative perception V2X simulation framework` | 100 | 23 |
| 39 | DBLP | `cooperative perception simulation` | n/a | 0 |
| 40 | OpenAlex | `connected automated vehicles simulation platform communication` | n/a | 0 |
| 40 | SemanticScholar | `connected automated vehicles simulation platform communication` | n/a | 0 |
| 40 | arXiv | `all:"connected and automated" AND ti:simulat` | n/a | 0 |
| 40 | Crossref | `connected automated vehicles simulation platform communication` | 100 | 16 |
| 40 | DBLP | `connected automated vehicles simulation platform` | n/a | 0 |
| 41 | OpenAlex | `large-scale vehicular network simulation scalability performance` | n/a | 0 |
| 41 | SemanticScholar | `large-scale vehicular network simulation scalability performance` | n/a | 0 |
| 41 | arXiv | `all:"large-scale" AND all:vehicular AND ti:simulat` | n/a | 0 |
| 41 | Crossref | `large-scale vehicular network simulation scalability performance` | 100 | 6 |
| 41 | DBLP | `large-scale vehicular simulation` | n/a | 0 |
| 42 | OpenAlex | `parallel distributed simulation vehicular networks` | n/a | 0 |
| 42 | SemanticScholar | `parallel distributed simulation vehicular networks` | n/a | 0 |
| 42 | arXiv | `all:parallel AND all:vehicular AND ti:simulat` | 0 | 0 |
| 42 | Crossref | `parallel distributed simulation vehicular networks` | 100 | 7 |
| 42 | DBLP | `parallel simulation vehicular` | n/a | 0 |
| 43 | OpenAlex | `VANET attack simulation framework injection` | n/a | 0 |
| 43 | SemanticScholar | `VANET attack simulation framework injection` | n/a | 0 |
| 43 | arXiv | `all:attack AND all:VANET AND ti:simulat` | 0 | 0 |
| 43 | Crossref | `VANET attack simulation framework injection` | 100 | 5 |
| 43 | DBLP | `VANET attack simulation` | n/a | 0 |
| 44 | OpenAlex | `IEEE 1609.2 security simulation V2X` | n/a | 0 |
| 44 | SemanticScholar | `IEEE 1609.2 security simulation V2X` | n/a | 0 |
| 44 | arXiv | `all:1609.2` | 7 | 0 |
| 44 | Crossref | `IEEE 1609.2 security simulation V2X` | 100 | 29 |
| 44 | DBLP | `1609.2 simulation` | n/a | 0 |
| 45 | OpenAlex | `TraCI traffic control interface SUMO coupling` | n/a | 0 |
| 45 | SemanticScholar | `TraCI traffic control interface SUMO coupling` | n/a | 0 |
| 45 | arXiv | `all:TraCI` | 100 | 0 |
| 45 | Crossref | `TraCI traffic control interface SUMO coupling` | 100 | 17 |
| 45 | DBLP | `TraCI` | n/a | 0 |
| 46 | OpenAlex | `game engine Unity V2X simulation` | n/a | 0 |
| 46 | SemanticScholar | `game engine Unity V2X simulation` | n/a | 0 |
| 46 | arXiv | `all:Unity AND all:V2X` | 2 | 1 |
| 46 | Crossref | `game engine Unity V2X simulation` | 100 | 3 |
| 46 | DBLP | `Unity V2X simulation` | n/a | 0 |
| 47 | OpenAlex | `driving simulator V2X communication integration` | n/a | 0 |
| 47 | SemanticScholar | `driving simulator V2X communication integration` | n/a | 0 |
| 47 | arXiv | `all:"driving simulator" AND all:V2X` | 9 | 1 |
| 47 | Crossref | `driving simulator V2X communication integration` | 100 | 17 |
| 47 | DBLP | `driving simulator V2X` | n/a | 0 |
| 48 | OpenAlex | `GeoNetworking simulation ETSI` | n/a | 0 |
| 48 | SemanticScholar | `GeoNetworking simulation ETSI` | n/a | 0 |
| 48 | arXiv | `all:GeoNetworking` | 4 | 0 |
| 48 | Crossref | `GeoNetworking simulation ETSI` | 100 | 2 |
| 48 | DBLP | `GeoNetworking` | n/a | 0 |
| 49 | OpenAlex | `collective perception message simulation` | n/a | 0 |
| 49 | SemanticScholar | `collective perception message simulation` | n/a | 0 |
| 49 | arXiv | `all:"collective perception"` | 63 | 3 |
| 49 | Crossref | `collective perception message simulation` | 100 | 2 |
| 49 | DBLP | `collective perception` | n/a | 0 |
| 50 | OpenAlex | `cooperative awareness message generation simulation ETSI` | n/a | 0 |
| 50 | SemanticScholar | `cooperative awareness message generation simulation ETSI` | n/a | 0 |
| 50 | arXiv | `all:"cooperative awareness" AND all:simulation` | 19 | 4 |
| 50 | Crossref | `cooperative awareness message generation simulation ETSI` | 100 | 2 |
| 50 | DBLP | `CAM generation` | n/a | 0 |
| 51 | OpenAlex | `ITS-G5 simulation` | n/a | 0 |
| 51 | SemanticScholar | `ITS-G5 simulation` | n/a | 0 |
| 51 | arXiv | `all:"ITS-G5"` | 29 | 5 |
| 51 | Crossref | `ITS-G5 simulation` | 100 | 5 |
| 51 | DBLP | `ITS-G5` | n/a | 0 |
| 52 | OpenAlex | `digital twin connected vehicles network` | n/a | 0 |
| 52 | SemanticScholar | `digital twin connected vehicles network` | n/a | 0 |
| 52 | arXiv | `ti:"digital twin" AND all:"connected vehicles"` | 13 | 12 |
| 52 | Crossref | `digital twin connected vehicles network` | 100 | 59 |
| 52 | DBLP | `digital twin connected vehicles` | n/a | 0 |
| 53 | OpenAlex | `network simulator 3 vehicular ad hoc` | n/a | 0 |
| 53 | SemanticScholar | `network simulator 3 vehicular ad hoc` | n/a | 0 |
| 53 | arXiv | `all:ns-3 AND all:VANET` | 10 | 3 |
| 53 | Crossref | `network simulator 3 vehicular ad hoc` | 100 | 5 |
| 53 | DBLP | `ns-3 VANET` | n/a | 0 |
| 54 | OpenAlex | `VANET simulation ns-2 MOVE traffic traces` | n/a | 0 |
| 54 | SemanticScholar | `VANET simulation ns-2 MOVE traffic traces` | n/a | 0 |
| 54 | arXiv | `all:VANET AND all:ns-2` | 15 | 2 |
| 54 | Crossref | `VANET simulation ns-2 MOVE traffic traces` | 100 | 16 |
| 54 | DBLP | `VANET ns-2` | n/a | 0 |
| 55 | OpenAlex | `basic safety message dataset` | n/a | 0 |
| 55 | SemanticScholar | `basic safety message dataset` | n/a | 0 |
| 55 | arXiv | `all:"basic safety message" AND all:dataset` | 4 | 0 |
| 55 | Crossref | `basic safety message dataset` | 100 | 0 |
| 55 | DBLP | `BSM dataset` | n/a | 0 |
| 56 | OpenAlex | `OMNeT++ INET vehicular simulation` | n/a | 0 |
| 56 | SemanticScholar | `OMNeT++ INET vehicular simulation` | n/a | 0 |
| 56 | arXiv | `all:"OMNeT++" AND all:vehicular` | 29 | 8 |
| 56 | Crossref | `OMNeT++ INET vehicular simulation` | 100 | 11 |
| 56 | DBLP | `OMNeT++ vehicular` | n/a | 0 |
| 57 | OpenAlex | `GEMV2 geometry-based vehicular channel model` | n/a | 0 |
| 57 | SemanticScholar | `GEMV2 geometry-based vehicular channel model` | n/a | 0 |
| 57 | arXiv | `all:GEMV2 OR (all:geometry AND all:vehicle AND all:"channel model" AND all:simulation)` | 22 | 1 |
| 57 | Crossref | `GEMV2 geometry-based vehicular channel model` | 100 | 3 |
| 57 | DBLP | `GEMV2` | n/a | 0 |
| 58 | OpenAlex | `WiLabV2Xsim LTEV2Vsim simulator` | n/a | 0 |
| 58 | SemanticScholar | `WiLabV2Xsim LTEV2Vsim simulator` | n/a | 0 |
| 58 | arXiv | `all:WiLabV2Xsim OR all:LTEV2Vsim` | 0 | 0 |
| 58 | Crossref | `WiLabV2Xsim LTEV2Vsim simulator` | 100 | 2 |
| 58 | DBLP | `LTEV2Vsim` | n/a | 0 |
| 59 | OpenAlex | `ray tracing digital twin vehicular channel Sionna` | n/a | 0 |
| 59 | SemanticScholar | `ray tracing digital twin vehicular channel Sionna` | n/a | 0 |
| 59 | arXiv | `all:"ray tracing" AND all:vehicular AND all:"digital twin"` | 6 | 5 |
| 59 | Crossref | `ray tracing digital twin vehicular channel Sionna` | 100 | 13 |
| 59 | DBLP | `ray tracing vehicular` | n/a | 0 |
| 60 | OpenAlex | `VANET simulator open source` | n/a | 0 |
| 60 | SemanticScholar | `VANET simulator open source` | n/a | 0 |
| 60 | arXiv | `ti:VANET AND ti:simulator` | 12 | 7 |
| 60 | Crossref | `VANET simulator open source` | 100 | 4 |
| 60 | DBLP | `VANET simulator` | n/a | 0 |
| 61 | OpenAlex | `V2X simulation framework` | n/a | 0 |
| 61 | SemanticScholar | `V2X simulation framework` | n/a | 0 |
| 61 | arXiv | `ti:V2X AND ti:simulation` | 11 | 7 |
| 61 | Crossref | `V2X simulation framework` | 100 | 36 |
| 61 | DBLP | `V2X simulation framework` | n/a | 0 |
| 62 | OpenAlex | `vehicular communication simulation co-simulation framework coupling` | n/a | 0 |
| 62 | SemanticScholar | `vehicular communication simulation co-simulation framework coupling` | n/a | 0 |
| 62 | arXiv | `ti:vehicular AND ti:co-simulation` | 0 | 0 |
| 62 | Crossref | `vehicular communication simulation co-simulation framework coupling` | 100 | 26 |
| 62 | DBLP | `vehicular co-simulation` | n/a | 0 |
| 63 | OpenAlex | `artificial intelligence V2X simulation reinforcement learning environment ns-3 gym` | n/a | 0 |
| 63 | SemanticScholar | `artificial intelligence V2X simulation reinforcement learning environment ns-3 gym` | n/a | 0 |
| 63 | arXiv | `all:ns3-gym OR (all:gym AND all:vehicular AND all:network)` | 4 | 0 |
| 63 | Crossref | `artificial intelligence V2X simulation reinforcement learning environment ns-3 gym` | 100 | 2 |
| 63 | DBLP | `ns3-gym vehicular` | n/a | 0 |
| x | Crossref | `VANET simulator comparison NS-2 NS-3 OMNeT++ SUMO` | 100 | 12 |
| x | Crossref | `vehicular ad hoc network simulation tools survey` | 100 | 19 |
| x | Crossref | `integrated simulation of vehicular networks traffic and communication` | 100 | 16 |
| x | Crossref | `inter-vehicle communication simulation framework` | 100 | 16 |
| x | Crossref | `car-to-car communication simulation environment` | 100 | 7 |
| x | Crossref | `cooperative ITS simulation platform evaluation` | 100 | 13 |
| x | Crossref | `C-V2X simulator open source` | 100 | 5 |
| x | Crossref | `LTE-V2X sidelink system level simulation platform` | 100 | 2 |
| x | Crossref | `5G V2X simulation framework OMNeT++` | 100 | 13 |
| x | Crossref | `NR V2X mode 2 simulation ns-3` | 100 | 8 |
| x | Crossref | `IEEE 802.11bd simulation` | 100 | 0 |
| x | Crossref | `802.11p PHY simulation model physical layer vehicular` | 100 | 6 |
| x | Crossref | `vehicular channel model simulation obstacle shadowing` | 100 | 8 |
| x | Crossref | `building shadowing model IEEE 802.11p simulation` | 100 | 2 |
| x | Crossref | `VANET emulator real-time` | 100 | 2 |
| x | Crossref | `V2X hardware in the loop simulation` | 100 | 10 |
| x | Crossref | `vehicle-in-the-loop V2X testing` | 100 | 14 |
| x | Crossref | `digital twin vehicular network simulation` | 100 | 47 |
| x | Crossref | `network digital twin V2X real-time` | 100 | 10 |
| x | Crossref | `connected vehicle digital twin communication` | 100 | 42 |
| x | Crossref | `V2X misbehavior simulation framework` | 100 | 28 |
| x | Crossref | `VANET security simulation framework attacks` | 100 | 6 |
| x | Crossref | `Sybil attack simulation VANET Veins` | 100 | 3 |
| x | Crossref | `pseudonym change strategy simulation evaluation` | 100 | 0 |
| x | Crossref | `certificate revocation VANET simulation` | 100 | 0 |
| x | Crossref | `security overhead V2X simulation verification delay` | 100 | 9 |
| x | Crossref | `V2X communication dataset real-world measurement` | 100 | 5 |
| x | Crossref | `CAM message dataset vehicular` | 100 | 0 |
| x | Crossref | `vehicular mobility traces dataset VANET` | 100 | 9 |
| x | Crossref | `realistic mobility traces vehicular network simulation` | 100 | 30 |
| x | Crossref | `TAPASCologne LuST scenario SUMO` | 100 | 18 |
| x | Crossref | `Luxembourg SUMO traffic scenario` | 100 | 19 |
| x | Crossref | `Monaco SUMO traffic scenario V2X` | 100 | 14 |
| x | Crossref | `co-simulation CARLA SUMO network` | 100 | 27 |
| x | Crossref | `autonomous driving simulator communication network co-simulation` | 100 | 23 |
| x | Crossref | `platooning simulation framework communication` | 100 | 10 |
| x | Crossref | `cooperative adaptive cruise control simulation V2V` | 100 | 2 |
| x | Crossref | `VANET simulation reproducibility` | 100 | 40 |
| x | Crossref | `credibility of VANET simulation studies` | 100 | 30 |
| x | Crossref | `ns-3 reproducible network simulation experiments` | 100 | 3 |
| x | Crossref | `OMNeT++ simulation credibility` | 100 | 7 |
| x | Crossref | `V2X application simulation evaluation safety` | 100 | 24 |
| x | Crossref | `large-scale city V2X simulation` | 100 | 3 |
| x | Crossref | `GPU accelerated network simulation vehicular` | 100 | 2 |
| x | Crossref | `network simulator machine learning surrogate vehicular` | 100 | 12 |
| x | Crossref | `V2X simulation reinforcement learning environment` | 100 | 9 |
| x | Crossref | `Eclipse MOSAIC simulation` | 100 | 1 |
| x | Crossref | `Veins INET Artery integration` | 100 | 0 |
| x | Crossref | `SimuLTE Veins LTE vehicular` | 100 | 3 |
| x | Crossref | `VSimRTI security` | 100 | 4 |
| x | Crossref | `SUMO traffic simulation software` | 100 | 21 |
| x | Crossref | `multi-scale V2X simulation` | 100 | 8 |
| x | Crossref | `V2X test track validation simulation` | 100 | 17 |
| x | Crossref | `ITS station emulation ETSI` | 100 | 2 |
| x | Crossref | `collective perception simulation Artery` | 100 | 3 |
| x | Crossref | `V2X simulation MATLAB toolbox` | 100 | 1 |
| x | Crossref | `vehicular network simulator Java` | 100 | 31 |
| x | arXiv | `all:Veins AND all:simulation` | 100 | 6 |
| x | arXiv | `ti:simulator AND all:vehicular` | 84 | 33 |
| x | arXiv | `ti:co-simulation` | 100 | 6 |
| x | arXiv | `all:V2X AND ti:testbed` | 4 | 4 |
| x | arXiv | `all:V2X AND ti:dataset` | 29 | 28 |
| x | arXiv | `all:vehicular AND ti:"digital twin"` | 45 | 36 |
| x | arXiv | `all:sidelink AND all:ns-3` | 4 | 2 |
| x | arXiv | `all:VANET AND all:OMNeT` | 14 | 2 |

## Snowballing

Method: for every high-relevance record with a DOI, its references and citing papers were pulled from OpenCitations Index v2 (`/references/doi:` and `/citations/doi:`); DOIs not already found were resolved in batches of 50 through Crossref (`filter=doi:...`) and passed through the same inclusion and relevance rules. Semantic Scholar's reference/citation endpoints were the intended source but returned HTTP 429 throughout. arXiv-only records (no DOI) could not be snowballed this way.

| Round | Seeds (high, with DOI) | New candidate DOIs | Kept |
|---|---|---|---|
| 1 (seeds from the keyword searches, first pass) | 278 | 5497 | 452 |
| 1 (seeds from the later keyword searches) | 83 | 1364 | 57 |
| 2 (seeds = high-relevance papers first found by snowballing) | 162 | 5291 | 230 |

Kept records carry `found_via: "snowball:<seed title> (references|citations, round N, OpenCitations)"`.

## Enrichment

After searching and snowballing, every kept record with a DOI or arXiv id was looked up through the Semantic Scholar batch endpoint (`/paper/batch`, 450 ids per call), which filled abstracts, open-access PDF links and citation counts for 1395 of 1523 records. Relevance was then re-assessed using the abstracts.

## Relevance assignment

Relevance was assigned with rules over title and abstract, then checked by hand on random samples: **high** = papers that build, extend, couple, validate or survey a vehicular network simulator, emulator, testbed, network digital twin, or V2X communication/misbehaviour dataset (including the named tools Veins, Artery, Vanetza, OpenCV2X, SimuLTE/Simu5G, ms-van3t, VSimRTI/MOSAIC, VENTOS, VESNOS, iTETRIS, OpenCDA, Plexe, F2MD, VeReMi, LTEV2Vsim/WiLabV2Xsim, GEMV2); **medium** = papers that use such tools for an application, apply a "digital twin" concept without a network model, or are cooperative-perception/driving datasets; **low** = traffic or driving simulators without a communication layer, generic VANET simulation studies, and general (non-vehicular) network-simulation credibility/reproducibility work. Some misclassification remains; summarisers should re-judge relevance from the full text.

## Known gaps

- OpenAlex and DBLP could not be queried (see above); Semantic Scholar keyword search succeeded for only a minority of queries. Crossref (120 queries) and arXiv (71 queries, 68 answered) carried the keyword search, so non-DOI grey literature is under-represented.
- Snowballing covered DOI-bearing seeds only (OpenCitations); arXiv-only seeds were not snowballed.
- Re-running the OpenAlex and DBLP rows when those services are available is recommended.
