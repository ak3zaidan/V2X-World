# Personalized Wireless Federated Learning for Large Language Models
Feibo Jiang, Li Dong, Siwei Tu et al. · arXiv 2024 · https://arxiv.org/abs/2404.13238 · relevance: low · basis: abstract

**Problem:** Large language models (LLMs) have driven profound transformations in wireless networks.
**Approach:** Building upon this foundation, we propose a Personalized Wireless Federated Fine-tuning (PWFF) framework. Subsequently, we develop two reward models and design a personalized loss function to fulfill the goal of personalized learning.
**Tools and scale:** Not stated in the abstract.
**Results:** Initially, we utilize the adapter and Low-Rank Adaptation (LoRA) techniques to decrease energy consumption, while employing global partial aggregation to reduce communication delay.
**Limitations and future work (as stated by the authors):**
- "Nevertheless, when integrated with LLMs, FL still struggles with several critical limitations, including large-scale and heterogeneous data, resource-intensive training, and substantial communication overhead."
- "Finally, we conduct a series of simulations to validate the performance of the proposed PWFF method and provide an in-depth discussion of the open issues."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
