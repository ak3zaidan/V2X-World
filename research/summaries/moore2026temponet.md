# TempoNet: Learning Realistic Communication and Timing Patterns for Network Traffic Simulation
Kristen Moore, Diksha Goel, Cody James Christopher, Zhen Wang et al. · arXiv 2026 · https://doi.org/10.1109/acsac67867.2025.00097 · relevance: medium · basis: abstract

**Problem:** Realistic network traffic simulation is critical for evaluating intrusion detection systems, stress-testing network protocols, and constructing high-fidelity environments for cybersecurity training. While attack traffic can often be layered into training environments using red-teaming or replay methods, generating authentic benign background traffic remains ...
**Approach:** This paper introduces TempoNet, a novel generative model that combines multi-task learning with multi-mark temporal point processes to jointly model inter-arrival times and all packet- and flow-header fields.
**Tools and scale:** Realistic network traffic simulation is critical for evaluating intrusion detection systems, stress-testing network protocols, and constructing high-fidelity environments for cybersecurity training.
**Results:** TempoNet captures fine-grained timing patterns and higher-order correlations such as host-pair behavior and seasonal trends, addressing key limitations of GAN-, LLM-, and Bayesian-based methods that fail to reproduce structured temporal variation. Furthermore, we show that intrusion detection models trained on TempoNet-generated background traffic perform comparably to those trained on real data, validating its utility for real-world security applications.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
