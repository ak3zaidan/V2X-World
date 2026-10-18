# Feasibility of State Space Models for Network Traffic Generation
Andrew Chu, Xi Jiang, Shi-Nan Liu, A. Bhagoji et al. · NAIC 2024 · https://doi.org/10.1145/3672198.3673792 · relevance: medium · basis: abstract

**Problem:** Many problems in computer networking rely on parsing collections of network traces (e.g., traffic prioritization, intrusion detection). Unfortunately, the availability and utility of these collections is limited due to privacy concerns, data staleness, and low representativeness.
**Approach:** While methods for generating data to augment collections exist, they often fall short in replicating the quality of real-world traffic In this paper, we i) survey the evolution of traffic simulators and generators and ii) propose the use of state space models, specifically Mamba, for packet-level, synthetic network trace generation by modeling it as an unsupervised sequence generation problem.
**Tools and scale:** Mentioned in abstract: real-world. (From the abstract only.)
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (network simulation methodology relevant to learned surrogates); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
