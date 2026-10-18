# Veritas: Answering Causal Queries from Video Streaming Traces
Chandan Bothra, Jianfei Gao, Sanjay Rao, Bruno Ribeiro · Proceedings of the ACM SIGCOMM 2023 Conference 2022 · https://doi.org/10.1145/3603269.3604828 · relevance: high · basis: full text (extracted sections)

**Problem:** Answering what-if (causal) questions from logs of a deployed video-streaming system, where latent network conditions confound outcomes.
**Approach:** Veritas: embedded Hidden Markov Model relating latent ground-truth bandwidth to observed download times, using TCP state as control variables; abduction for counterfactual and interventional queries without RCTs.
**Tools and scale:** Emulation testbed with bandwidth traces as ground truth; ABR video.
**Results:** Answers counterfactual (e.g., different buffer size) and interventional queries more accurately than a non-causal baseline, approaching an oracle (numbers not extracted).
**Limitations and future work (as stated by the authors):**
- Domain-specific to ABR video; relies on observable TCP state.
**Overlap with our engine:** partial - per-session counterfactual estimation in networking validated against emulated ground truth, the same evaluation logic as direction 3; not V2X.
**Novelty implications:** Counterfactual queries over network logs are established; V2X per-message counterfactual ground truth across traffic/radio/PKI is not covered.
