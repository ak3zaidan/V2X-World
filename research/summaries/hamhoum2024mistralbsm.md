# MistralBSM: Leveraging Mistral-7B for Vehicular Networks Misbehavior Detection
Wissal Hamhoum, Soumaya Cherkaoui · arXiv (Cornell University) 2024 · https://arxiv.org/abs/2407.18462 · relevance: high · basis: full text

**Problem:** LLMs for V2X misbehaviour detection.
**Approach:** Fine-tuned Mistral-7B (QLoRA, 0.012% params) on BSM sequences as edge detector with a larger cloud LLM validating; quantisation and sequence-length study.
**Tools and scale:** Selected attacks from VeReMi.
**Results:** 98% binary and 96% multi-class accuracy, beating LLAMA2-7B and RoBERTa.
**Limitations and future work (as stated by the authors):**
- Notes prior LLM MDS works risk training data leakage via message ID or pseudo-ID features.
**Overlap with our engine:** partial — LLM in the MBD loop (runner-up LLM direction) but as classifier.
**Novelty implications:** Cannot claim first LLM-based BSM MBD; LLM for cross-layer root-cause analysis of runs remains open.
