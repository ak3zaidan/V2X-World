# WDMoE: Wireless Distributed Mixture of Experts for Large Language Models
Nan Xue, Yaping Sun, Zhiyong Chen et al. · arXiv 2024 · https://arxiv.org/abs/2411.06681 · relevance: low · basis: abstract

**Problem:** Large Language Models (LLMs) have achieved significant success in various natural language processing tasks, but the role of wireless networks in supporting LLMs has not been thoroughly explored.
**Approach:** In this paper, we propose a wireless distributed Mixture of Experts (WDMoE) architecture to enable collaborative deployment of LLMs across edge servers at the base station (BS) and mobile devices in wireless networks. Accordingly, we develop a performance metric for WDMoE-based LLMs, which accounts for both model capability and latency.
**Tools and scale:** Not stated in the abstract.
**Results:** To minimize the latency while maintaining accuracy, we jointly optimize expert selection and bandwidth allocation based on the performance metric. Both theoretical simulations and practical hardware experiments demonstrate that the proposed method can significantly reduce the latency without compromising LLM performance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
