# BackFlush: Knowledge-Free Backdoor Detection and Elimination with Watermark Preservation in Large Language Models
Jagadeesh Rachapudi, Ritali Vatsi, Pranav Singh, Praful Hambarde, Amit Shukla · arXiv 2026 · https://arxiv.org/abs/2605.12529 · relevance: medium · basis: abstract

**Problem:** In recent trends, one can observe Large Language Models (LLMs) are exposed to backdoor attacks where vicious triggers added during training or model editing to elicit harmful outputs on specific input patterns while maintaining clean performance on normal inputs.
**Approach:** Legitimate watermarks used as ownership signatures share similar mechanisms to backdoors, creating a critical challenge: detecting and eliminating unknown backdoors without compromising watermark integrity. Existing defenses require prior knowledge of triggers or their payloads, depend on clean reference models, or sacrifice model utility without preserving the watermark.
**Tools and scale:** Not stated in the abstract.
**Results:** Comprehensive evaluation across diverse trigger types over different architectures demonstrates BackFlush achieves approximately 1%Attack Success Rate (ASR), approximately 99% clean accuracy (CACC), and preserved watermarking capabilities in the realm where no existing method simultaneously provides these alongside maintaining model utility comparable to clean baselines.
**Limitations and future work (as stated by the authors):**
- To address these limitations we introduce BackFlush and its variants, a unified framework for backdoor detection and elimination while preserving watermarks.
**Overlap with our engine:** partial — addresses security overhead or cryptography (direction 2 area); LLM agents (runner-up area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
