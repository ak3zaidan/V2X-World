# Uncertainty-Aware Hybrid Inference with On-Device Small and Remote Large Language Models
Seungeun Oh, Jinhyuk Kim, Jihong Park et al. · arXiv 2024 · https://arxiv.org/abs/2412.12687 · relevance: low · basis: abstract

**Problem:** This paper studies a hybrid language model (HLM) architecture that integrates a small language model (SLM) operating on a mobile device with a large language model (LLM) hosted at the base station (BS) of a wireless network.
**Approach:** While this approach ensures alignment between the vocabulary distributions of the SLM and LLM, it suffers from low token throughput due to uplink transmission and the computation costs of running both language models. To address this, we propose a novel HLM structure coined Uncertainty-aware opportunistic HLM (U-HLM), wherein the SLM locally measures its output uncertainty and skips both uplink transmissions and LLM operations for tokens that are likely to be accepted.
**Tools and scale:** Not stated in the abstract.
**Results:** Simulations show that U-HLM reduces uplink transmissions and LLM computations by 45.93%, while achieving up to 97.54% of the LLM's inference accuracy and 2.54$\times$ faster token throughput than HLM without skipping.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
