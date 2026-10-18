# Leveraging MoE-based Large Language Model for Zero-Shot Multi-Task Semantic Communication
Sin-Yu Huang, Renjie Liao, Vincent W. S. Wong · arXiv 2025 · https://arxiv.org/abs/2503.15722 · relevance: low · basis: abstract

**Problem:** Multi-task semantic communication (SC) can reduce the computational resources in wireless systems since retraining is not required when switching between tasks.
**Approach:** However, existing approaches typically rely on task-specific embeddings to identify the intended task, necessitating retraining the entire model when given a new task. We incorporate a mixture-of-experts (MoE) architecture in the FLAN model and propose MoE-FLAN-SC architecture for multi-task SC systems.
**Tools and scale:** Not stated in the abstract.
**Results:** Inspired by the superior zero-shot capabilities of large language models (LLMs), we leverage pre-trained instruction-tuned LLMs, referred to as fine-tuned language net (FLAN), to improve the generalization capability. Our proposed MoE-FLAN-SC architecture can further improve the performance of FLAN-T5 model without increasing the computational cost.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
