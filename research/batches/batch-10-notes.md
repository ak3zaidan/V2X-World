# Batch 10 notes (378 papers)

Basis: 6 high-relevance papers summarised from full text; the rest from abstracts (36 had no abstract, title only). Most of this batch is adjacent background (network digital twins, wireless foundation models, LLMs for network management, ML performance prediction); only stated limitations found in full texts are listed. Abstract-only summaries record "not stated in the abstract".

## 1. Stated limitations and future work
- qin2025urbanv2x: data richness and evaluation completeness to be extended.
- qin2025urbanv2x: collaborative SLAM methods not yet validated on the dataset.
- qin2025urbanv2x: annotations incomplete.
- muktar2026reproducible: mobility not realistic yet; future work on SUMO-based mobility.
- muktar2026reproducible: richer wireless channel models needed.
- muktar2026reproducible: larger multi-seed datasets needed.
- muktar2026reproducible: learning-based UAV control policies to prevent fragmentation not yet done.
- xu2026cost: validated on one urban scenario only; needs more topologies, demand patterns and communication settings.
- xu2026cost: adaptive configuration across protocol stacks and application workloads not yet supported.
- xu2026cost: data and scripts not publicly archived.
- solida2026demystifying: only the Veins default configuration was compared.
- solida2026demystifying: propagation model needs refining for the measurement environment.
- solida2026demystifying: Veins parameters need calibration against real data (Veins overestimates RSSI and receives about 18% fewer messages than the field).
- anjum2026mint: short-range deviations such as RSRP upper-bound exceedance; simulation-only data.
- luz2026yes: only CAN; automotive Ethernet (AVTP) planned.
- luz2026yes: automatic labelling for IDS datasets planned.

## 2. Strong overlap
None. No paper in this batch combines V2X traffic, radio and SCMS/PKI security in one simulation, or closed-loop revocation. Closest partial overlaps:
- anjum2026mint: SUMO + Simu5G dataset with synchronized per-vehicle mobility and PHY ground truth at 100 ms (overlaps with joint traffic/radio ground truth, but has no PKI).
- silva2026b5gcybertestv2x: CARLA + Simu5G closed loop in which network performance drives safety outcomes (overlaps with direction 1's network-to-safety loop, but has no misbehaviour or revocation).
- xu2026cost: systematic configuration and cost/statistical-power model for SUMO/Veins co-simulation (overlaps with our configuration space and LLM configuration agent).
- solida2026demystifying: Veins checked against living-lab data (affects our "Veins-matched" fidelity claim).
- ma2026twin: noise-coupled factual/counterfactual twin rollouts in learned world models (overlaps with direction 3's shared-noise counterfactual idea).
- li2024chatsumo, ye2025sumo, jeong2025speak: LLM agents that configure SUMO (an LLM simulation-configuration agent is not novel by itself).
- sana2025reasoning, yi2025automated: LLM and automated cross-layer root-cause analysis in 5G (not V2X or PKI).
