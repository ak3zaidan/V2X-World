# NetLLM: Adapting Large Language Models for Networking
Duo Wu, Xianda Wang, Yaqi Qiao, Zhi Wang, Junchen Jiang, Shuguang Cui et al. · Proceedings of the ACM SIGCOMM 2024 Conference 2024 · https://doi.org/10.1145/3651890.3672268 · relevance: high · basis: full text (extracted sections)

**Problem:** DL networking algorithms need per-task hand-designed DNNs and generalise poorly.
**Approach:** NetLLM: adapts a pre-trained LLM to networking via a multimodal encoder, task-specific networking head and low-cost domain-adaptation fine-tuning, aiming at one model for many tasks.
**Tools and scale:** Three use cases: viewport prediction, adaptive bitrate streaming, cluster job scheduling (spark-sched-sim). No V2X.
**Results:** NetLLM-adapted LLM outperforms state-of-the-art task-specific algorithms on all three tasks, including better generalisation (numbers not extracted here).
**Limitations and future work (as stated by the authors):**
- Not stated in the extracted sections.
**Overlap with our engine:** partial - LLMs as foundation models for networking; our engine uses an LLM agent to configure and analyse runs, a different role.
**Novelty implications:** We cannot claim first use of LLMs for networking tasks. LLM agents operating a V2X simulator for cross-layer analysis are not addressed.
