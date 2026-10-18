# Simulating Malicious Attacks on VANETs for Connected and Autonomous Vehicle Cybersecurity: A Machine Learning Dataset
Safras Iqbal, Peter Ball, Muhammad H Kamarudin, Andrew Bradley · International Symposium on Communication Systems, Networks and Digital Signal Processing (CSNDSP) 2022 · https://doi.org/10.1109/csndsp54353.2022.9908023 · relevance: high · basis: full text

**Problem:** Need data on how VANET attacks affect CAV behaviour to train ML detectors.
**Approach:** Eclipse MOSAIC models two road scenarios with V2V/V2I messaging; a custom module injects replay (emergency-vehicle impersonation) and bogus information attacks; an open dataset is released.
**Tools and scale:** Eclipse MOSAIC (with SUMO traffic); two small scenarios with a handful of vehicles; detection is post-simulation; no PKI.
**Results:** Qualitative: replay attacker impersonating an emergency vehicle makes victims stop and passes through; bogus info changes victims' speeds; dataset published.
**Limitations and future work (as stated by the authors):**
- Evaluation workflow is non-interactive since detection is post-simulation (noted as gap in prior work and their own pipeline)
- future: ML detection and mitigation on the dataset.
**Overlap with our engine:** partial — attacks executed in a traffic+V2X co-simulation with observable driving impact; small scale, no closed-loop detection or PKI.
**Novelty implications:** Cannot claim first simulation showing traffic impact of replay/bogus attacks; closed-loop detection-to-revocation remains open.
