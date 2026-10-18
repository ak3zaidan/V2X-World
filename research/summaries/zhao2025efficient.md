# Efficient Split Federated Learning for Large Language Models over Communication Networks
Kai Zhao, Zhaohui Yang, Ye Hu et al. · arXiv 2025 · https://arxiv.org/abs/2504.14667 · relevance: low · basis: abstract

**Problem:** Fine-tuning pre-trained large language models (LLMs) in a distributed manner poses significant challenges on resource-constrained edge networks.
**Approach:** To address this challenge, we propose SflLLM, a novel framework that integrates split federated learning with parameter-efficient fine-tuning techniques. By leveraging model splitting and low-rank adaptation (LoRA), SflLLM reduces the computational burden on edge devices.
**Tools and scale:** Mentions: federated learning.
**Results:** Simulation results demonstrate that the proposed SflLLM framework achieves comparable model accuracy while significantly reducing client-side computational requirements. Furthermore, the proposed resource allocation scheme and adaptive LoRA rank selection strategy notably reduce the training latency compared to conventional approaches.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
