# Agent-Assisted Side-Channel Attacks on Non-Prefix KV Cache in RAG
He Sun, Shinan Liu, Siyuan Ma, Junhao Li, Mingjun Xiao, Wenhao Jiang · arXiv 2026 · https://arxiv.org/abs/2606.21842 · relevance: medium · basis: abstract

**Problem:** Modern Large Language Model (LLM) serving engines increasingly rely on Retrieval-Augmented Generation (RAG) and non-prefix Key-Value (KV) cache fusion to accelerate long-context, multi-tenant inference.
**Approach:** While existing KV cache side-channel attacks require strict linear prefix alignment--rendering them ineffective against real-world RAG queries that contain unique, user-specific private prefixes--we uncover a critical class of structural vulnerabilities inherent to chunk-aware memory scheduling. We demonstrate that the deterministic micro-architectural mechanisms used to align and fuse disjoint memory chunks inadvertently leak a continuous "Step-Wave" timing signature.
**Tools and scale:** Mentions: real-world; scale not stated in the abstract.
**Results:** Extensive evaluations on production-grade frameworks (vLLM integrated with LMCache) demonstrate that SpliceLeak achieves up to a 100% extraction success rate in bounded-entropy scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses security overhead or cryptography (direction 2 area); pseudonym privacy/linkability (runner-up area); LLM agents (runner-up area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
