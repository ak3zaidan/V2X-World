# Large Language Models Can Perform Automatic Modulation Classification via Discretized Self-supervised Candidate Retrieval
Mohammad Rostami, Atik Faysal, Reihaneh Gh. Roshan et al. · arXiv 2025 · https://arxiv.org/abs/2510.00316 · relevance: low · basis: abstract

**Problem:** Identifying wireless modulation schemes is essential for cognitive radio, but standard supervised models often degrade under distribution shift, and training domain-specific wireless foundation models from scratch is computationally prohibitive.
**Approach:** We introduce DiSC-AMC, a framework that reformulates Automatic Modulation Classification (AMC) as an LLM reasoning task by combining aggressive feature discretization with nearest-neighbor retrieval over self-supervised embeddings.
**Tools and scale:** Not stated in the abstract.
**Results:** By mapping continuous features to coarse symbolic tokens, DiSC-AMC aligns abstract signal patterns with LLM reasoning capabilities and reduces prompt length by over $50$\%. On a 10-class benchmark, a fine-tuned 7B-parameter LLM using DiSC-AMC achieves $83.0$\% in-distribution accuracy ($-10$\,to\,$+10$\,dB) and $82.50$\% out-of-distribution (OOD) accuracy ($-11$\,to\,$-15$\,dB), outperforming supervised baselines.
**Limitations and future work (as stated by the authors):**
- "Finally, we identify a fundamental limitation of this pipeline."
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
