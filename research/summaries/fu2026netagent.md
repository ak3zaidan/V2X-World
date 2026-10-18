# NetAgent: Multi-Task Agentic Network Traffic Analysis Made Practical
Hao Fu, Dawn Song, Peng Gao · arXiv 2026 · https://arxiv.org/abs/2610.07386 · relevance: medium · basis: abstract

**Problem:** Network traffic analysis is central to network security, spanning tasks from intrusion detection to encrypted traffic classification. Existing approaches either train task-specific models that generalize poorly or rely on costly traffic foundation models that still struggle under distribution shift.
**Approach:** We present NetAgent, the first agentic framework for multi-task traffic analysis.
**Tools and scale:** These results reveal that existing methods owe much of their reported success to overfitting dataset-specific patterns and degrade sharply in realistic network environments, while NetAgent's agentic design remains accurate, generalizable, and robust.
**Results:** It introduces five key designs: (1) knowledge-augmented workflow planning that maps attack knowledge to traffic features to bridge the semantic gap; (2) a comprehensive tool action space with 150+ verified tools extracted from 50+ published systems; (3) a unified code execution ... Across 9 major benchmarks, NetAgent outperforms all baselines (23 single-task and 5 multi-task) on nearly all tasks and generalizes substantially better to unseen traffic distribution (90.04% F1 vs. 2.74% and 3.04% for the best single-task and multi-task baselines) and under ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
