# Search log: cluster `ai`

Cluster scope: learned surrogates, world models and foundation models for networks and vehicular systems; learned C-V2X/DSRC performance prediction and predictive QoS; counterfactual/causal simulation and causal benchmarks with interference; LLM agents for network simulation, configuration and security operations; driving world models where they touch V2X or counterfactual evaluation.

Run date: 2026-10-17. User-Agent `v2x-lit-review/1.0`, no email or name in any request.

## Source availability on the run date

- **OpenAlex**: every request returned HTTP 429 `Insufficient budget` (the free daily budget shared by the network IP was already exhausted, resets 00:00 UTC). The 18 planned OpenAlex queries were re-issued against Crossref and Semantic Scholar instead. OpenAlex should be re-queried later to fill gaps (theses, OA links).
- **DBLP**: all 20 queries were answered with an Anubis proof-of-work bot challenge (HTTP 202), on dblp.org, dblp.uni-trier.de and dblp.dagstuhl.de. Not bypassed; 0 results. DBLP coverage is largely subsumed by Crossref/Semantic Scholar for this cluster, but it should be retried.
- **Semantic Scholar**: unauthenticated search was heavily throttled (HTTP 429) by parallel review agents; only the queries listed below succeeded. The batch endpoint was used once to add abstracts, arXiv ids and OA links to existing records.
- **arXiv** and **Crossref**: fully available.
- Snowballing used Semantic Scholar references/citations where it responded, and otherwise OpenCitations (COCI v2 `references`/`citations`) with metadata resolved through Crossref.

Hit count = records returned by the query (capped at 100 for arXiv/S2, 60 for Crossref). Kept = records from that query that passed screening (high/medium/low) for this cluster; a record found by several queries counts for each.

## Database queries

| # | Source | Query | Hits | Kept |
|---|---|---|---|---|
| 1 | arXiv API | `all:"graph neural network" AND all:"network modeling" AND all:delay` | 9 | 4 |
| 2 | arXiv API | `ti:RouteNet` | 6 | 6 |
| 3 | arXiv API | `all:"network digital twin" AND (all:"graph neural" OR all:surrogate OR all:learned)` | 30 | 26 |
| 4 | arXiv API | `all:"data-driven network simulation" OR all:"learned network simulator"` | 4 | 3 |
| 5 | arXiv API | `(all:"network simulation" OR all:"network simulator") AND all:"deep learning" AND (all:surrogate OR all:approximate)` | 6 | 1 |
| 6 | arXiv API | `all:"network foundation model" OR all:"foundation model for network traffic"` | 15 | 9 |
| 7 | arXiv API | `all:"wireless foundation model" OR all:"large wireless model" OR all:"foundation model for wireless"` | 76 | 69 |
| 8 | arXiv API | `(all:V2X OR all:vehicular OR all:VANET) AND all:"foundation model"` | 20 | 10 |
| 9 | arXiv API | `(all:V2X OR all:"C-V2X" OR all:sidelink) AND (all:"machine learning" OR all:"deep learning") AND (all:"packet delivery" OR all:reliability OR all:"performance prediction")` | 42 | 14 |
| 10 | arXiv API | `all:"predictive quality of service" AND (all:vehicular OR all:V2X OR all:automotive)` | 12 | 10 |
| 11 | arXiv API | `(all:"QoS prediction" OR all:"throughput prediction" OR all:"latency prediction") AND (all:vehicular OR all:V2X OR all:"connected vehicles")` | 7 | 7 |
| 12 | arXiv API | `all:CausalSim OR (all:counterfactual AND all:"trace-driven simulation")` | 1 | 1 |
| 13 | arXiv API | `all:counterfactual AND (all:"network simulation" OR all:networking OR all:"video streaming") AND all:causal` | 100 | 11 |
| 14 | arXiv API | `all:"causal inference" AND (all:"wireless network" OR all:"computer network" OR all:vehicular)` | 3 | 3 |
| 15 | arXiv API | `all:benchmark AND all:causal AND all:interference AND (all:counterfactual OR all:"treatment effect")` | 8 | 2 |
| 16 | arXiv API | `all:"large language model" AND (all:"network simulation" OR all:ns-3 OR all:"network simulator")` | 35 | 10 |
| 17 | arXiv API | `all:"large language model" AND (all:"network configuration" OR all:"network management") AND all:agent` | 35 | 27 |
| 18 | arXiv API | `all:LLM AND (all:"security operations" OR all:SOC OR all:"incident response") AND all:benchmark` | 60 | 21 |
| 19 | arXiv API | `all:LLM AND (all:V2X OR all:vehicular OR all:"connected vehicles") AND (all:network OR all:communication)` | 35 | 22 |
| 20 | arXiv API | `all:"world model" AND (all:network OR all:wireless OR all:vehicular OR all:V2X)` | 100 | 37 |
| 21 | arXiv API | `all:"world model" AND all:driving AND (all:counterfactual OR all:V2X OR all:cooperative)` | 29 | 7 |
| 22 | arXiv API | `(all:"digital twin" AND (all:V2X OR all:vehicular)) AND (all:learning OR all:AI)` | 44 | 20 |
| 23 | arXiv API | `all:"generative AI" AND (all:"network simulation" OR all:"network digital twin")` | 6 | 4 |
| 24 | arXiv API | `(all:"packet-level" OR all:"flow-level") AND all:simulation AND (all:"neural network" OR all:"machine learning") AND (all:approximation OR all:acceleration)` | 5 | 0 |
| 25 | arXiv API | `(all:"intrusion detection" OR all:"misbehavior detection") AND (all:VANET OR all:V2X) AND all:"large language model"` | 0 | 0 |
| 26 | arXiv API | `all:"LLM" AND (all:"network troubleshooting" OR all:"root cause analysis") AND (all:network OR all:incident)` | 60 | 22 |
| 27 | arXiv API | `all:"agentic" AND (all:"6G" OR all:"network operations" OR all:"telecom")` | 100 | 46 |
| 28 | Semantic Scholar search | `m3 flow-level network performance estimation machine learning` | 100 | 8 |
| 29 | Semantic Scholar search | `netFound foundation model network security` | 100 | 15 |
| 30 | Semantic Scholar search | `MobiWorld world model mobile networks` | 100 | 9 |
| 31 | Semantic Scholar search | `GenOnet generative AI network` | 100 | 8 |
| 32 | Semantic Scholar search | `CyberSOCEval` | 1 | 1 |
| 33 | arXiv API | `ti:MimicNet OR ti:DeepQueueNet OR ti:netFound OR ti:CausalSim` | 2 | 2 |
| 34 | arXiv API | `all:"network calculus" AND all:"graph neural network"` | 1 | 0 |
| 35 | arXiv API | `all:"queueing" AND all:"neural network" AND (all:"network performance" OR all:"delay prediction") AND all:"graph"` | 1 | 1 |
| 36 | arXiv API | `(all:"C-ITS" OR all:"cooperative intelligent transport") AND (all:"machine learning" OR all:"deep learning")` | 10 | 2 |
| 37 | arXiv API | `(all:"connected vehicles" OR all:"connected and automated vehicles") AND all:"digital twin" AND all:network` | 11 | 3 |
| 38 | arXiv API | `(all:"NR-V2X" OR all:"LTE-V2X" OR all:"5G V2X") AND (all:"reinforcement learning" OR all:"deep learning") AND all:prediction` | 1 | 0 |
| 39 | arXiv API | `(all:"802.11p" OR all:DSRC) AND (all:"machine learning" OR all:"neural network")` | 13 | 5 |
| 40 | arXiv API | `all:"age of information" AND all:vehicular AND all:learning AND all:prediction` | 4 | 2 |
| 41 | arXiv API | `all:"LLM" AND (all:"autonomous driving" OR all:vehicles) AND (all:"cooperative" OR all:"V2X")` | 47 | 14 |
| 42 | arXiv API | `all:"LLM" AND all:"penetration testing" OR (all:LLM AND all:"cyber range")` | 100 | 23 |
| 43 | arXiv API | `all:"LLM" AND (all:"automotive security" OR all:"in-vehicle network" OR all:"CAN bus")` | 5 | 3 |
| 44 | arXiv API | `(all:"treatment effect" OR all:"causal effect") AND (all:"A/B test" OR all:"interference") AND (all:network OR all:marketplace) AND all:simulation` | 75 | 33 |
| 45 | arXiv API | `all:"causal discovery" AND (all:"network performance" OR all:telecom OR all:"root cause")` | 29 | 8 |
| 46 | arXiv API | `all:"what-if" AND (all:"network" OR all:vehicular) AND (all:"digital twin" OR all:learned)` | 100 | 19 |
| 47 | arXiv API | `all:"emulation" AND all:"network" AND all:"neural" AND all:"fidelity"` | 79 | 0 |
| 48 | arXiv API | `all:"simulation-to-reality" AND (all:wireless OR all:network OR all:vehicular)` | 89 | 7 |
| 49 | arXiv API | `all:"trace-driven" AND (all:"network emulation" OR all:"simulation") AND all:learning` | 44 | 2 |
| 50 | arXiv API | `all:"spatio-temporal" AND all:"mobile traffic" AND (all:"foundation model" OR all:"generative")` | 4 | 0 |
| 51 | arXiv API | `all:"world model" AND all:"traffic simulation" AND (all:generative OR all:learned)` | 7 | 6 |
| 52 | arXiv API | `all:"closed-loop" AND all:"driving simulation" AND all:"generative" AND all:"counterfactual"` | 1 | 0 |
| 53 | arXiv API | `all:"misbehavior detection" AND (all:VeReMi OR all:"F2MD") AND all:learning` | 5 | 2 |
| 54 | arXiv API | `(all:V2X OR all:vehicular) AND all:"surrogate model"` | 2 | 0 |
| 55 | arXiv API | `all:"neural ray tracing" OR all:"differentiable ray tracing" AND all:wireless` | 12 | 2 |
| 56 | arXiv API | `all:"beam prediction" AND (all:vehicular OR all:V2X) AND all:"deep learning"` | 5 | 1 |
| 57 | arXiv API | `all:"network digital twin" AND all:"vehicular"` | 5 | 5 |
| 58 | Semantic Scholar search | `deep learning model of 802.11p MAC performance` | 100 | 3 |
| 59 | arXiv API | `all:"self-supervised" AND all:"network traffic" AND (all:representation OR all:transformer OR all:pretrain)` | 12 | 2 |
| 60 | arXiv API | `(all:"channel prediction" OR all:"channel modeling") AND (all:V2V OR all:vehicular) AND (all:"deep learning" OR all:generative)` | 56 | 6 |
| 61 | arXiv API | `all:"radio map" AND (all:neural OR all:learned) AND (all:vehicular OR all:urban)` | 24 | 3 |
| 62 | arXiv API | `all:"generative model" AND (all:"network traffic" OR all:"packet traces") AND (all:synthesis OR all:generation)` | 25 | 5 |
| 63 | arXiv API | `all:"diffusion model" AND (all:wireless OR all:vehicular) AND (all:channel OR all:simulation)` | 100 | 5 |
| 64 | arXiv API | `all:"large language model" AND (all:"6G" OR all:telecom) AND (all:"digital twin" OR all:simulation)` | 57 | 38 |
| 65 | Crossref | `predictive quality of service V2X machine learning` | 60 | 5 |
| 66 | Crossref | `graph neural network network digital twin` | 60 | 9 |
| 67 | Crossref | `machine learning C-V2X performance prediction` | 60 | 4 |
| 68 | Crossref | `large language model vehicular network` | 60 | 18 |
| 69 | Crossref | `world model wireless network` | 60 | 1 |
| 70 | Crossref | `RouteNet graph neural network network modeling` | 60 | 11 |
| 71 | Crossref | `MimicNet fast performance estimates data center networks machine learning` | 60 | 2 |
| 72 | Crossref | `DeepQueueNet packet-level network simulation deep learning` | 60 | 6 |
| 73 | Crossref | `network digital twin graph neural network performance prediction` | 60 | 6 |
| 74 | Crossref | `learned surrogate model network simulator` | 60 | 3 |
| 75 | Crossref | `machine learning prediction C-V2X packet delivery ratio` | 60 | 10 |
| 76 | Crossref | `predictive quality of service vehicular communications` | 60 | 11 |
| 77 | Crossref | `deep learning DSRC performance prediction VANET` | 60 | 2 |
| 78 | Crossref | `counterfactual simulation causal inference networking trace-driven` | 60 | 1 |
| 79 | Crossref | `large language model network configuration agent` | 60 | 10 |
| 80 | Crossref | `large language model network simulation ns-3` | 60 | 7 |
| 81 | Crossref | `foundation model wireless communications` | 60 | 19 |
| 82 | Crossref | `world model vehicular network` | 60 | 6 |
| 83 | Crossref | `digital twin vehicular network artificial intelligence` | 60 | 5 |
| 84 | Crossref | `machine learning VANET simulation surrogate` | 60 | 4 |
| 85 | Crossref | `LLM security operations center benchmark` | 60 | 4 |
| 86 | Crossref | `network traffic foundation model pretraining` | 60 | 7 |
| 87 | Crossref | `causal machine learning wireless networks` | 60 | 3 |
| 88 | Crossref | `digital twin vehicular network learning` | 60 | 36 |
| 89 | Crossref | `machine learning DSRC VANET performance prediction` | 60 | 5 |
| 90 | Crossref | `large language model network management agent` | 60 | 3 |
| 91 | Crossref | `causal inference network performance counterfactual` | 60 | 0 |
| 92 | Crossref | `foundation model network traffic` | 60 | 9 |
| 93 | arXiv API | `ti:RouteNet OR abs:RouteNet` | 10 | 8 |
| 94 | arXiv API | `abs:MimicNet OR abs:DeepQueueNet OR abs:"m3" AND abs:"flow-level"` | 1 | 1 |
| 95 | arXiv API | `all:"flow-level simulation" AND all:"machine learning"` | 1 | 1 |
| 96 | arXiv API | `(all:"LLM" OR all:"large language model") AND all:"wireless" AND all:"simulation"` | 86 | 49 |
| 97 | arXiv API | `all:"large language model" AND all:"digital twin" AND all:network` | 58 | 23 |
| 98 | arXiv API | `all:"LLM" AND all:"network incident" OR (all:"LLM" AND all:"network operations" AND all:benchmark)` | 20 | 12 |
| 99 | arXiv API | `all:"LLM" AND all:"cybersecurity" AND all:benchmark AND (all:"threat intelligence" OR all:"log analysis" OR all:"SOC")` | 16 | 13 |
| 100 | arXiv API | `all:"LLM" AND (all:"intrusion detection" OR all:"anomaly detection") AND all:network` | 100 | 46 |
| 101 | arXiv API | `all:"generative AI" AND (all:vehicular OR all:V2X OR all:"connected vehicles")` | 19 | 10 |
| 102 | arXiv API | `all:"vehicular" AND all:"digital twin" AND (all:"graph neural" OR all:"deep learning")` | 2 | 0 |
| 103 | arXiv API | `all:"C-V2X" AND (all:"neural" OR all:"learning") AND (all:reliability OR all:latency OR all:"packet reception")` | 16 | 3 |
| 104 | arXiv API | `all:"sidelink" AND all:"machine learning" AND (all:prediction OR all:"performance")` | 5 | 2 |
| 105 | arXiv API | `all:"V2X" AND all:"QoS prediction"` | 3 | 3 |
| 106 | arXiv API | `all:"V2X" AND all:"dataset" AND (all:"machine learning" OR all:"benchmark") AND all:communication` | 47 | 14 |
| 107 | arXiv API | `all:"packet reception ratio" AND (all:learning OR all:prediction) AND (all:vehicular OR all:V2V)` | 1 | 0 |
| 108 | arXiv API | `all:"vehicular" AND all:"link quality" AND all:prediction AND all:learning` | 0 | 0 |
| 109 | arXiv API | `all:"counterfactual" AND (all:vehicular OR all:"autonomous driving" OR all:traffic) AND all:simulation` | 58 | 13 |
| 110 | arXiv API | `all:"counterfactual" AND all:"reinforcement learning" AND (all:network OR all:"congestion control" OR all:"adaptive bitrate")` | 47 | 4 |
| 111 | arXiv API | `all:"off-policy evaluation" AND (all:network OR all:wireless OR all:systems)` | 96 | 2 |
| 112 | arXiv API | `all:"world model" AND all:"autonomous driving" AND all:"multi-agent" AND all:generative` | 14 | 6 |
| 113 | arXiv API | `all:"neural simulator" AND (all:network OR all:traffic OR all:wireless)` | 54 | 1 |
| 114 | arXiv API | `all:"machine learning" AND (all:"ns-3" OR all:"OMNeT++") AND all:"accelerat"` | 0 | 0 |
| 115 | arXiv API | `all:"LLM" AND all:"vehicular" AND (all:"security" OR all:"misbehavior" OR all:"attack")` | 9 | 5 |
| 116 | arXiv API | `all:"large language model" AND all:"SUMO" OR (all:"large language model" AND all:"traffic simulation")` | 33 | 14 |
| 117 | arXiv API | `all:"LLM" AND all:"simulation" AND all:"configuration" AND (all:network OR all:"scenario generation")` | 59 | 15 |
| 118 | arXiv API | `all:"agent" AND all:"LLM" AND all:"telecom" AND all:"root cause"` | 5 | 4 |
| 119 | Crossref | `RouteNet graph neural network` | 60 | 7 |
| 120 | Crossref | `MimicNet data center network simulation` | 60 | 2 |
| 121 | Crossref | `DeepQueueNet` | 1 | 1 |
| 122 | Crossref | `CausalSim counterfactual` | 60 | 1 |
| 123 | Crossref | `netFound network foundation model` | 60 | 4 |
| 124 | Crossref | `predictive QoS sidelink` | 60 | 10 |
| 125 | Crossref | `deep learning C-V2X reliability prediction` | 60 | 8 |
| 126 | Crossref | `vehicular network performance prediction neural network` | 60 | 2 |
| 127 | Crossref | `LLM network operations benchmark` | 60 | 7 |
| 128 | Crossref | `wireless foundation model pretraining` | 60 | 8 |
| 129 | Crossref | `network digital twin machine learning vehicular` | 60 | 18 |
| 130 | Crossref | `LLM security operations center evaluation` | 60 | 6 |
| 131 | arXiv API | `all:"network performance" AND all:"graph neural network" AND (all:"digital twin" OR all:modeling)` | 26 | 6 |
| 132 | arXiv API | `all:"network traffic" AND all:"foundation model"` | 15 | 12 |
| 133 | arXiv API | `all:"large language model" AND all:"network traffic" AND (all:classification OR all:analysis)` | 34 | 16 |
| 134 | arXiv API | `all:"causal" AND all:"digital twin"` | 62 | 15 |
| 135 | arXiv API | `all:"interference" AND all:"causal inference" AND all:"simulation" AND all:benchmark` | 1 | 0 |
| 136 | arXiv API | `all:"world model" AND (all:"6G" OR all:"wireless network" OR all:"mobile network")` | 44 | 29 |
| 137 | Crossref | `MimicNet fast performance estimates for data center networks with machine learning` | 60 | 2 |
| 138 | Crossref | `DeepQueueNet scalable generalized network performance estimation packet-level visibility` | 60 | 3 |
| 139 | Crossref | `Parsimon scalable tail latency estimation data center networks` | 60 | 0 |
| 140 | Crossref | `ns3-ai fostering artificial intelligence algorithms for networking research` | 60 | 1 |
| 141 | arXiv API | `ti:MimicNet` | 0 | 0 |
| 142 | arXiv API | `ti:Parsimon OR ti:"ns3-ai" OR ti:"ns3 gym"` | 1 | 1 |
| 143 | arXiv API | `all:NIKA AND all:network AND (all:LLM OR all:agent)` | 5 | 3 |
| 144 | arXiv API | `all:"network incident" AND (all:LLM OR all:"language model") AND all:benchmark` | 1 | 1 |
| 145 | arXiv API | `all:"neural network simulator" OR all:"learned packet-level simulation" OR all:"data-driven packet-level"` | 100 | 0 |
| 146 | arXiv API | `(all:"V2X" OR all:"vehicular") AND all:"generative model" AND (all:"channel" OR all:"packet")` | 3 | 1 |
| 147 | arXiv API | `all:"network simulation" AND all:"large language model" AND all:agent` | 19 | 2 |
| 148 | arXiv API | `all:"what-if" AND all:"network performance" AND all:"graph neural network"` | 2 | 1 |
| 149 | Semantic Scholar search | `NetArena LLM agents network benchmark` | 100 | 35 |
| 150 | Semantic Scholar search | `predictive QoS V2X machine learning` | 100 | 32 |
| 151 | Semantic Scholar search | `vehicular network digital twin machine learning surrogate` | 100 | 41 |
| 152 | Semantic Scholar search | `RouteNet graph neural network network modeling` | 100 | 28 |

Distinct queries that returned: 152 (arXiv API 98, Semantic Scholar search 10, Crossref 44). Queries that failed after retries (arXiv/S2 throttling, OpenAlex budget, DBLP challenge) are not counted in the table; DBLP: 20 attempted, 0 answered; OpenAlex: 18 attempted, 0 answered.

## Snowballing

Round 1 seeds: every high-relevance record from the database queries. Round 2 seeds: high-relevance records first found in round 1. Kept = candidates that passed screening.

| Round | Method | Seed | Candidates | Kept |
|---|---|---|---|---|
| 1 | OpenCitations refs+cites -> Crossref metadata | 5G NR V2X Throughput Prediction Using Deep Hybrid Learning | 12 | 4 |
| 1 | OpenCitations refs+cites -> Crossref metadata | A Deep Learning Method for Automatic Reliability Prediction of CR-VANETs | 50 | 11 |
| 1 | OpenCitations refs+cites -> Crossref metadata | A Network Arena for Benchmarking AI Agents on Network Troubleshooting | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | A Novel Framework for QoS Prediction of V2X in 5G and B5G Networks: a Unified Approach with Explainable Artifi | 7 | 7 |
| 1 | OpenCitations refs+cites -> Crossref metadata | A Performance Analysis of Tree-Based and Deep Learning Models for Predicting Quality of Service in Vehicle-to- | 4 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | A Reinforcement Learning-Based PQoS Framework for V2X Networks in NS-3 | 13 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Active Few-shot Learning For RouteNet-Fermi | 9 | 4 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Advancing Network Digital Twin Framework for Generating Realistic Datasets | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | An AI-Enabled Decision Support System for Predictive QoS in Vehicular Networks | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Berlin V2X: A Machine Learning Dataset from Multiple Vehicles and Radio Access Technologies | 38 | 19 |
| 1 | OpenCitations refs+cites -> Crossref metadata | BiLQoS: Stacked Bi-LSTM for Operator-Agnostic QoS Prediction in C-V2X Smart Cities | 7 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Can Vehicles Foresee Communication Disruptions?: Feasibility of V2X Network Digital Twin | 9 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Clustered Predictive Quality of Service for Connected Vehicles Using Federated Learning | 14 | 5 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Clustering of Geographical Segments for Predictive Quality of Service of Connected Vehicles | 29 | 7 |
| 1 | OpenCitations refs+cites -> Crossref metadata | CyberSOCEval: Benchmarking LLMs Capabilities for Malware Analysis and Threat Intelligence Reasoning | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | DEMO: NetArena Adaptation for Next Waves of Network Benchmarks | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | DISTINQT: A Distributed Privacy Aware Learning Framework for QoS Prediction for Future Mobile and Wireless Net | 36 | 6 |
| 1 | OpenCitations refs+cites -> Crossref metadata | DRIVE: A Digital Network Oracle for Cooperative Intelligent Transportation Systems | 30 | 6 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Data-Driven Network Performance Prediction for B5G Networks: A Graph Neural Network Approach | 14 | 5 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Data-driven Network Simulation for Performance Analysis of Anticipatory Vehicular Communication Systems | 59 | 16 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Deep Learning Surrogate Models for Network Simulation | 3 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Deep Learning in Network-Level Performance Prediction Using Cross-Layer Information | 40 | 8 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Digital Twin Based Network Latency Prediction in Vehicular Networks | 14 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Digital Twin based Packet Reception Prediction for C-V2X Networks | 7 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Distributed Predictive QoS in Automotive Environments Under Concept Drift | 24 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Enhancing Vehicular Communication with Predictive Quality of Service: A Real-World Proof of Concept | 3 | 2 |
| 1 | OpenCitations refs+cites -> Crossref metadata | GenOnet: Generative Open xG Network Simulation with Multi-Agent LLM and ns-3 | 10 | 5 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Generative Network Performance Prediction with Network Digital Twin | 24 | 11 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Improving QoS Prediction in Urban V2X Networks by Leveraging Data from Leading Vehicles and Historical Trends | 16 | 12 |
| 1 | OpenCitations refs+cites -> Crossref metadata | In-Context Autonomous Network Incident Response: An End-to-End Large Language Model Agent Approach | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Learning Queuing Networks by Recurrent Neural Networks | 58 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Learning Without Forgetting: Predicting the Reliability of V2X Wireless Communication | 19 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Lens: A Foundation Model for Network Traffic in Cybersecurity | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | ML-Driven Prediction of QoS in C-V2X Scenarios | 9 | 7 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Machine Learning Aided NR-V2X Quality of Service Predictions | 23 | 12 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Machine Learning Benchmarking for V2X Link‑Quality Inference Across Communication Types | 12 | 6 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Machine Learning for QoS Prediction in Vehicular Communication: Challenges and Solution Approaches | 78 | 27 |
| 1 | OpenCitations refs+cites -> Crossref metadata | MobiWorld: World Models for Mobile Wireless Network | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Mobile Network Control with a World Model | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Multi-MNO Predictive-QoS for Vehicular Applications | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Multi-environment automotive QOS prediction using Machine Learning for Vehicle-to-Anything (V2X) communication | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | NR-V2X Quality of Service Prediction Through Machine Learning with Nested Cross-Validation Scheme | 19 | 9 |
| 1 | OpenCitations refs+cites -> Crossref metadata | NS+NDT: Smart integration of Network Simulation in Network Digital Twin, application to IoT networks | 54 | 20 |
| 1 | OpenCitations refs+cites -> Crossref metadata | NetAgentBench: A State-Centric Benchmark for Evaluating Agentic Network Configuration | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | NetLLMBench: A Benchmark Framework for Large Language Models in Network Configuration Tasks | 15 | 9 |
| 1 | OpenCitations refs+cites -> Crossref metadata | NordicDat: A Cross-Border Predictive QoS Dataset | 29 | 8 |
| 1 | OpenCitations refs+cites -> Crossref metadata | On Effectiveness of Graph Neural Network Architectures for Network Digital Twins (NDTs) | 10 | 10 |
| 1 | OpenCitations refs+cites -> Crossref metadata | PRATA: A Framework to Enable Predictive QoS in Vehicular Networks via Artificial Intelligence | 42 | 21 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Packet Delivery Ratio Prediction for V2V Based on Radio Environment Map Considering Hidden Terminal Problem | 11 | 1 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Performance Evaluation of Graph Neural Network-Based RouteNet Model with Attention Mechanism | 18 | 11 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Predicting CAM generation times through machine learning for cellular V2X communication | 6 | 5 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Predictive QoS for Tele-Operated Driving over 5G SA Networks: an Experimental Study | 14 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Predictive Quality of Service in Cellular Networks: Challenges, Framework, and Application in Vehicular Commun | 16 | 9 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Predictive Quality of Service in Vehicular Communication Systems Models, Datasets, and Open Challenges | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Predictive Quality of Service: Adaptation of Platoon Inter-Vehicle Distance to Packet Inter-Reception Time | 10 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | QT-Routenet: Improved GNN generalization to larger 5G networks by fine-tuning predictions from queueing theory | 5 | 4 |
| 1 | OpenCitations refs+cites -> Crossref metadata | QoS Predictability in V2X Communication with Machine Learning | 33 | 21 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RIS-Assisted Full Duplex MAC Protocol for NR-V2X with ML-Driven QoS Prediction | 15 | 4 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RouteNet-Erlang: A Graph Neural Network for Network Performance Evaluation | 72 | 33 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RouteNet-Fermi: Network Modeling with Graph Neural Networks | 122 | 55 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RouteNet-Gauss: Hardware-Enhanced Network Modeling with Machine Learning | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RouteNet-TGNN-A Temporal Graph Neural Network for Delay and Loss Forecasting in Dynamic Communication Networks | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | RouteNet: Leveraging Graph Neural Networks for network modeling and optimization in SDN | 328 | 111 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Routenet: Using Graph Neural Networks for SDN Network Modeling and Optimizations | 2 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | SIMCODE: A Benchmark for Natural Language to ns-3 Network Simulation Code Generation | 9 | 2 |
| 1 | OpenCitations refs+cites -> Crossref metadata | SRCON: A Data-Driven Network Performance Simulator for Real-World Wireless Networks | 33 | 5 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Student Research Track: Twin the Drive: An HGBR-based Machine Learning Model as a Vehicular Digital Twin for R | 16 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Towards a Playground to Democratize Experimentation and Benchmarking of AI Agents for Network Troubleshooting | 9 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Unveiling the potential of Graph Neural Networks for network modeling and optimization in SDN | 195 | 62 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Usage of a Graph Neural Network for Large-Scale Network Performance Evaluation | 17 | 2 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Using Neural Network to Model and Estimate PER in Vehicular Ad Hoc Network | 17 | 3 |
| 1 | OpenCitations refs+cites -> Crossref metadata | V2X Communication Reliability Improvement through QoS Prediction and Reservation Using Network Virtualization  | 0 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | Vehicle-to-Everything (V2X) Datasets for Machine Learning-Based Predictive Quality of Service | 18 | 4 |
| 1 | OpenCitations refs+cites -> Crossref metadata | m3: Accurate Flow-Level Performance Estimation using Machine Learning | 45 | 11 |
| 1 | OpenCitations refs+cites -> Crossref metadata | m4: A Learned Flow-level Network Simulator | 1 | 0 |
| 1 | OpenCitations refs+cites -> Crossref metadata | ns3-GenAI: Integrating Large Language Models with ns3 for AI-Native Network Simulations | 0 | 0 |
| 1 | Semantic Scholar references+citations | A Multi-Modal Simulation Framework to Enable Digital Twin-based V2X Communications in Dynamic Environments | 57 | 28 |
| 1 | Semantic Scholar references+citations | A Reinforcement Learning Framework for PQoS in a Teleoperated Driving Scenario | 33 | 10 |
| 1 | Semantic Scholar references+citations | AdaPTwin: Adaptive Multi-Fidelity Predictive Digital Twin for Proactive Radio Resource Management in Vehicular | 31 | 17 |
| 1 | Semantic Scholar references+citations | AgentSUMO: An Agentic Framework for Interactive Simulation Scenario Generation in SUMO via Large Language Mode | 42 | 13 |
| 1 | Semantic Scholar references+citations | Agentic-V2X: Small Language Model Agents for Deadline-Aware V2X Scheduling in 5G/6G Networks | 20 | 8 |
| 1 | Semantic Scholar references+citations | Artificial Intelligence in Vehicular Wireless Networks: A Case Study Using ns-3 | 45 | 14 |
| 1 | Semantic Scholar references+citations | Benchmarking LLM-Driven Network Configuration Repair | 47 | 8 |
| 1 | Semantic Scholar references+citations | Beyond Simulation: Benchmarking World Models for Planning and Causality in Autonomous Driving | 35 | 6 |
| 1 | Semantic Scholar references+citations | Bridging the Gap Between Simulated and Real Network Data Using Transfer Learning | 24 | 7 |
| 1 | Semantic Scholar references+citations | CTBench: Evaluating Troubleshooting Capabilities of AI Agents in Realistic Telecom Network Operations | 24 | 9 |
| 1 | Semantic Scholar references+citations | CausalSim: A Causal Framework for Unbiased Trace-Driven Simulation | 124 | 11 |
| 1 | Semantic Scholar references+citations | CounterScene: Counterfactual Causal Reasoning in Generative World Models for Safety-Critical Closed-Loop Evalu | 62 | 14 |
| 1 | Semantic Scholar references+citations | Demystifying Network Foundation Models | 66 | 24 |
| 1 | Semantic Scholar references+citations | Digital Twin for Networking: A Data-driven Performance Modeling Perspective | 121 | 61 |
| 1 | Semantic Scholar references+citations | Evaluating Agentic Configuration Repair for Computer Networks | 26 | 4 |
| 1 | Semantic Scholar references+citations | FaulT-Bench: Towards Benchmarking Network Troubleshooting LLM Agents under Unreliable User Tickets | 45 | 17 |
| 1 | Semantic Scholar references+citations | Frame Error Rate Prediction for Non-Stationary Wireless Vehicular Communication Links | 21 | 2 |
| 1 | Semantic Scholar references+citations | How Can Driving World Models Do Counterfactual Prediction? | 33 | 2 |
| 1 | Semantic Scholar references+citations | Improving the Real-Data Driven Network Evaluation Model for Digital Twin Networks | 23 | 14 |
| 1 | Semantic Scholar references+citations | Joint Clustering and Prediction of the Quality of Service in Vehicular Cellular Networks | 0 | 0 |
| 1 | Semantic Scholar references+citations | Large Wireless Model (LWM): A Foundation Model for Wireless Channels | 139 | 90 |
| 1 | Semantic Scholar references+citations | Learn to Augment Network Simulators Towards Digital Network Twins | 15 | 7 |
| 1 | Semantic Scholar references+citations | Learnable Digital Twin for Efficient Wireless Network Evaluation | 39 | 21 |
| 1 | Semantic Scholar references+citations | Learning a Network Digital Twin as a Hybrid System | 29 | 8 |
| 1 | Semantic Scholar references+citations | Lens: A Knowledge-Guided Foundation Model for Network Traffic | 75 | 43 |
| 1 | Semantic Scholar references+citations | M3Net: A Multi-Metric Mixture of Experts Network Digital Twin with Graph Neural Networks | 33 | 15 |
| 1 | Semantic Scholar references+citations | Multi-Agent Embodied Autonomous Driving (MAEAD): From V2X Information Exchange to Shared World Models | 0 | 0 |
| 1 | Semantic Scholar references+citations | NDT Factory: Synthesizing Verified Network Digital Twins from Semantic Models via Multi-Agent LLM | 28 | 7 |
| 1 | Semantic Scholar references+citations | NS3Learn: Transferring 5G NR Mode-2 Reception Realism from ns-3 to the Veins/SUMO Stack for Connected-Vehicle  | 29 | 9 |
| 1 | Semantic Scholar references+citations | NetArena: Dynamic Benchmarks for AI Agents in Network Automation | 93 | 15 |
| 1 | Semantic Scholar references+citations | NetConfArena: An Executable Benchmark for LLM Agents in Closed-Loop Network Configuration | 39 | 11 |
| 1 | Semantic Scholar references+citations | Network Digital Twin for Route Optimization in 5G/B5G Transport Slicing with What-If Analysis | 25 | 11 |
| 1 | Semantic Scholar references+citations | Network World Models as Environments for Algorithm Design on Complex Systems | 123 | 1 |
| 1 | Semantic Scholar references+citations | Predicting Networks Before They Happen: Experimentation on a Real-Time V2X Digital Twin | 26 | 16 |
| 1 | Semantic Scholar references+citations | QoS Evaluation and Prediction for C-V2X Communication in Commercially-Deployed LTE and Mobile Edge Networks | 49 | 22 |
| 1 | Semantic Scholar references+citations | QoS Prediction for 5G Connected and Automated Driving | 65 | 19 |
| 1 | Semantic Scholar references+citations | QoS prediction in radio vehicular environments via prior user information | 20 | 9 |
| 1 | Semantic Scholar references+citations | The Best of Both Worlds: Hybrid Data-Driven and Model-Based Vehicular Network Simulation | 24 | 10 |
| 1 | Semantic Scholar references+citations | Toward Composable Network Digital Twins: A Subgraph-Based Latency Prediction Study | 41 | 23 |
| 1 | Semantic Scholar references+citations | Towards Decentralized Predictive Quality of Service in Next-Generation Vehicular Networks | 33 | 5 |
| 1 | Semantic Scholar references+citations | Towards Trustworthy 6G Network Digital Twins: A Framework for Validating Counterfactual What-If Analysis in Ed | 23 | 16 |
| 1 | Semantic Scholar references+citations | V2X-LLM: Enhancing V2X Integration and Understanding in Connected Vehicle Corridors | 61 | 20 |
| 1 | Semantic Scholar references+citations | Veritas: Answering Causal Queries from Video Streaming Traces | 66 | 10 |
| 1 | Semantic Scholar references+citations | What-if Analysis Framework for Digital Twins in 6G Wireless Network Management | 36 | 22 |
| 1 | Semantic Scholar references+citations | White-Box Modeling of V2X Link Performance Using Stabilized Symbolic Regression | 43 | 9 |
| 1 | Semantic Scholar references+citations | WirelessOpsAgent: A Benchmark and Agent Design for Action Assurance in Wireless Networks | 31 | 9 |
| 1 | Semantic Scholar references+citations | World Model-Based Learning for Long-Term Age of Information Minimization in Vehicular Networks | 26 | 11 |
| 1 | Semantic Scholar references+citations | netFound: A Domain-specific Foundation Model for Network Security | 1 | 0 |
| 1 | Semantic Scholar references+citations | netFound: Principled Design for Network Foundation Models | 76 | 42 |
| 2 | OpenCitations refs+cites -> Crossref metadata | <i>xNet</i>: Modeling Network Performance With Graph Neural Networks | 31 | 5 |
| 2 | OpenCitations refs+cites -> Crossref metadata | A Comparison of AI-Based Throughput Prediction for Cellular Vehicle-To-Server Communication | 27 | 5 |
| 2 | OpenCitations refs+cites -> Crossref metadata | A Reinforcement Learning Framework for PQoS in a Teleoperated Driving Scenario | 24 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | A Statistical Learning Framework for QoS Prediction in V2X | 15 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | A federated learning approach to QoS forecasting in cellular vehicular communications: Approaches and empirica | 36 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | ADePT: Latency prediction for edge CDN traffic scheduling via causal inference | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | AI-driven, QoS prediction for V2X communications in beyond 5G systems | 35 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | AI4Mobile: Use Cases and Challenges of AI-based QoS Prediction for High-Mobility Scenarios | 51 | 11 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Addressing RouteNet scalability through input and output design | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Artificial Intelligence in Vehicular Wireless Networks: A Case Study Using ns-3 | 23 | 10 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Artificial Intelligence-Enabled 5G Network Performance Evaluation With Fine Granularity and High Accuracy | 44 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Beyond Mean: Spatio-Temporal Modeling of Queue Utilizations and Flow Latencies Using T-GNNs | 25 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Boosting Vehicle-to-Cloud Communication by Machine Learning-Enabled Context Prediction | 86 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | CausalTune: Causal Learning based Automated Cellular RAN Configuration Tuning Framework | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Challenging the generalization capabilities of Graph Neural Networks for network modeling | 39 | 13 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Comparative Analysis of Neural Network Models for Error Probability Prediction in Vehicular Communication | 15 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Continual Benchmarking of LLM-Based Systems on Networking Operations | 3 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | D2D-based QoS prediction analysis in beyond 5G V2X | 5 | 1 |
| 2 | OpenCitations refs+cites -> Crossref metadata | DITTO: DIgital Twins for Testing and Optimizing Wireless Decisions | 17 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | DT-CoVeSS: Advancing NextG C-V2X Security Evaluation through High-Fidelity Digital Twinning | 13 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Data-Driven Prediction Models for Wireless Network Configuration | 16 | 5 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Data-driven Predictive Latency for 5G: A Theoretical and Experimental Analysis Using Network Measurements | 18 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Deep Learning-based QoS Prediction with Innate Knowledge of the Radio Access Network | 14 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | DeepComNet: Performance evaluation of network topologies using graph-based deep learning | 34 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | DeepTMA: Predicting Effective Contention Models for Network Calculus using Graph Neural Networks | 63 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Digital Twin for Networking: A Data-driven Performance Modeling Perspective | 89 | 39 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Digital Twins for Maintaining QoS in Programmable Vehicular Networks | 31 | 18 |
| 2 | OpenCitations refs+cites -> Crossref metadata | EAGLE: Heterogeneous GNN-based Network Performance Analysis | 38 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Empirical Analysis of Client-Based Network Quality Prediction in Vehicular Multi-MNO Networks | 48 | 18 |
| 2 | OpenCitations refs+cites -> Crossref metadata | End-to-End Spatio-temporal Latency Prediction for Vehicular Applications | 11 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Estimating Quality-of-Service in Urban Vehicular Networks Through Machine Learning | 14 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Exploring the Limitations of Current Graph Neural Networks for Network Modeling | 17 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | FlowDT: A Flow-Aware Digital Twin for Computer Networks | 26 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Forecasting SDN End-to-End Latency Using Graph Neural Network | 27 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Frame Error Rate Prediction for Non-Stationary Wireless Vehicular Communication Links | 19 | 3 |
| 2 | OpenCitations refs+cites -> Crossref metadata | From simulation to deep learning: Survey on network performance modeling approaches | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | GNN-based End-to-end Delay Prediction in Software Defined Networking | 32 | 14 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Graph Neural Network for Real-Time Simulation of SDN-Enabled Communication | 13 | 3 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Graph Neural Network-based Delay Prediction Model Enhanced by Network Calculus | 21 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Graph-neural-network-based delay estimation for communication networks with heterogeneous scheduling policies | 10 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Grothendieck Graph Neural Network (GGNN): A Path-Based Framework For Network Modelling | 6 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Group-based link modeling for wireless digital twins: Towards accurate network performance prediction | 38 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | IGNNITION: Bridging the Gap between Graph Neural Networks and Networking Systems | 25 | 13 |
| 2 | OpenCitations refs+cites -> Crossref metadata | IRNet: INVANETs Performance Prediction via Spatio-Temporal Graph Attention Networks | 24 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Improving Network Delay Predictions Using GNNs | 43 | 16 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Improving the Real-Data Driven Network Evaluation Model for Digital Twin Networks | 13 | 5 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Informer-based QoS prediction for V2X communication: A method with verification using reality field test data | 20 | 10 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Intelligent performance inference: A graph neural network approach to modeling maximum achievable throughput i | 34 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Iridescence: Improving Configuration Tuning in the Presence of Confounders for 5G NSA Networks | 28 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | LSTM-based QoS prediction for 5G-enabled Connected and Automated Mobility applications | 31 | 20 |
| 2 | OpenCitations refs+cites -> Crossref metadata | LWM: A Pre-trained Wireless Foundation Model for Universal Feature Extraction | 14 | 1 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Latency Prediction for Delay-sensitive V2X Applications in Mobile Cloud/Edge Computing Systems | 29 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Learnable Digital Twin for Efficient Wireless Network Evaluation | 31 | 15 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Learning-Based Network Performance Estimators: The Next Frontier for Network Simulation | 14 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | MHNet: A Multi-Head GNN Architecture for Efficient Network Modeling | 43 | 10 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Machine Learning Based Context-Predictive Car-to-Cloud Communication Using Multi-Layer Connectivity Maps for U | 34 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Machine Learning Based Propagation Loss Module for Enabling Digital Twins of Wireless Networks in ns-3 | 14 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Machine Learning-Enabled Data Rate Prediction for 5G NSA Vehicle-to-Cloud Communications | 40 | 16 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Mechanisms for the Estimation of Prediction Intervals in Vehicular Communication Scenarios | 8 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Modeling TCP performance using graph neural networks | 27 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | NetConfEval: Can LLMs Facilitate Network Configuration? | 58 | 31 |
| 2 | OpenCitations refs+cites -> Crossref metadata | NetGen: A Network Digital Twin Generator | 7 | 5 |
| 2 | OpenCitations refs+cites -> Crossref metadata | NetLLM: Adapting Large Language Models for Networking | 145 | 61 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Network Digital Twin for Route Optimization in 5G/B5G Transport Slicing with What-If Analysis | 16 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Network-Level System Performance Prediction Using Deep Neural Networks with Cross-Layer Information | 4 | 1 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Neural Surrogate Model for Autonomous Driving Communications Based on Strategic Sampling | 3 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Neural language models for network configuration: Opportunities and reality check | 39 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Online QoS estimation for vehicular radio environments | 25 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Packet Inter-Reception Time Conditional Density Estimation Based on Surrounding Traffic Distribution | 24 | 7 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Packet Inter-Reception Time Modeling for High-Density Platooning in Varying Surrounding Traffic Density | 10 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Packet Loss Rate Prediction for Vehicular Networks with Regression Methods | 1 | 1 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Per Link Data-Driven Network Replication Towards Self-Adaptive Digital Twins | 21 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Position-Based Machine Learning Propagation Loss Model Enabling Fast Digital Twins of Wireless Networks in ns- | 12 | 8 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Predicting transmission success with Support Vector Machine in VANETs | 20 | 11 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Prediction of Communication Delays in Connected Vehicles and Platoons | 21 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Prediction of Packet Inter-Reception Time for Platooning using Conditional Exponential Distribution | 11 | 3 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Predictive QoS of wireless communication for autonomous driving vehicles by fine tuning | 15 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | QoS Evaluation and Prediction for C-V2X Communication in Commercially-Deployed LTE and Mobile Edge Networks | 32 | 15 |
| 2 | OpenCitations refs+cites -> Crossref metadata | QoS Prediction for 5G Connected and Automated Driving | 51 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Revolutionizing Wireless Modeling and Simulation with Network-Oriented LLMs | 10 | 2 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Sim2HW: Modeling Latency Offset Between Network Simulations and Hardware Measurements | 10 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Stationarity-Aware QoS Prediction for Platooning in V2X Networks | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | System Modeling and Performance Evaluation of Predictive QoS for Future Tele-Operated Driving | 24 | 11 |
| 2 | OpenCitations refs+cites -> Crossref metadata | The Best of Both Worlds: Hybrid Data-Driven and Model-Based Vehicular Network Simulation | 21 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | The graph neural networking challenge | 41 | 16 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Toward Generative 6G Simulation: An Experimental Multi-Agent LLM and ns-3 Integration | 15 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Toward Reproducing Network Research Results Using Large Language Models | 23 | 9 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Towards Automated RAN Configuration Tuning in Cellular Networks with Causal Learning | 0 | 0 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Towards Cooperative Data Rate Prediction for Future Mobile and Vehicular 6G Networks | 67 | 12 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Towards Data-Driven Simulation of End-to-End Network Performance Indicators | 21 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Towards Digital Network Twins: Can we Machine Learn Network Function Behaviors? | 18 | 3 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Towards more realistic network models based on Graph Neural Networks | 17 | 10 |
| 2 | OpenCitations refs+cites -> Crossref metadata | Veritas: Answering Causal Queries from Video Streaming Traces | 33 | 4 |
| 2 | OpenCitations refs+cites -> Crossref metadata | What If We Had Used a Different App? Reliable Counterfactual KPI Analysis in Wireless Systems | 49 | 11 |
| 2 | OpenCitations refs+cites -> Crossref metadata | What do LLMs need to Synthesize Correct Router Configurations? | 65 | 39 |
| 2 | OpenCitations refs+cites -> Crossref metadata | vChainNet: Accurate and Scalable End-to-End Slice Modeling for 5G and Beyond Networks | 19 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | vNetRunner: Per-VNF Slice Modeling for 5G and Beyond Networks | 15 | 6 |
| 2 | OpenCitations refs+cites -> Crossref metadata | xNet: Improving Expressiveness and Granularity for Network Modeling with Graph Neural Networks | 50 | 17 |
| 2 | Semantic Scholar references+citations | A benchmark for graph neural networks for computer network modeling | 0 | 0 |
| 2 | Semantic Scholar references+citations | CausalSim: A Causal Inference Framework for Unbiased Trace-Driven Simulation | 2 | 1 |
| 2 | Semantic Scholar references+citations | CausalSim: Toward a Causal Data-Driven Simulator for Network Protocols | 6 | 2 |
| 2 | Semantic Scholar references+citations | Confounding-Valid Conformal Inference for Counterfactual KPIs in Wireless Networks | 38 | 12 |
| 2 | Semantic Scholar references+citations | GLANCE: Graph-based Learnable Digital Twin for Communication Networks | 69 | 29 |
| 2 | Semantic Scholar references+citations | NeuralEmu: in situ Measurement-Driven, ML-based, High-Fidelity 5G Network Emulation | 52 | 4 |
| 2 | Semantic Scholar references+citations | Path-Link Graph Neural Network for IP Network Performance Prediction | 20 | 10 |
| 2 | Semantic Scholar references+citations | The Story of QoS Prediction in Vehicular Communication: From Radio Environment Statistics to Network-Access Th | 2 | 2 |

Snowball seeds: 231; candidates retrieved: 6767.

## Screening

Titles (and abstracts where needed) of every candidate were screened by hand against the cluster scope and the protocol inclusion rules, after a keyword pre-filter that tagged candidates as: learned network models/digital twins; wireless or network foundation/world models; learned V2X performance/pQoS prediction; counterfactual/causal; LLM/agentic for networks or security operations; driving world models; vehicular digital twins/GenAI. Generic work outside networking (e.g. GNNs for chemistry, generic driving world models with no V2X or counterfactual angle, generic LLM benchmarks) was excluded. Adjacent work (e.g. GNN traffic engineering, generic network digital twin architectures, network-traffic classification foundation models, conformal/causal methods used in wireless) was kept at low relevance.
