# SafeCoop: Unravelling Full Stack Safety in Agentic Collaborative Driving
Xiangbo Gao, Tzu-Hsiang Lin, Ruojing Song, Yuheng Wu, Kuan-Ru Huang, Zicheng Jin et al. · arXiv 2025 · https://arxiv.org/abs/2510.18123 · relevance: medium · basis: abstract

**Problem:** Collaborative driving systems leverage vehicle-to-everything (V2X) communication across multiple agents to enhance driving safety and efficiency.
**Approach:** Traditional V2X systems take raw sensor data, neural features, or perception results as communication media, which face persistent challenges, including high bandwidth demands, semantic loss, and interoperability issues. Recent advances investigate natural language as a promising medium, which can provide semantic richness, decision-level reasoning, and human-machine interoperability at significantly lower bandwidth.
**Tools and scale:** Mentions: CARLA; scale not stated in the abstract.
**Results:** We systematically evaluate SafeCoop in closed-loop CARLA simulation across 32 critical scenarios, achieving 69.15% driving score improvement under malicious attacks and up to 67.32% F1 score for malicious detection.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection or trust (direction 1 area); V2X simulation/datasets (direction 3 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
