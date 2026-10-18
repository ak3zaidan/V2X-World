# LLM-Based Digital Twin Intelligence for Application-Aware Network Selection in 6G Heterogeneous Wireless Networks
Brahim Mefgouda, Anis Bara, Lina Bariah, Hang Zou et al. · arXiv 2026 · https://arxiv.org/abs/2606.12293 · relevance: medium · basis: abstract

**Problem:** Future 6G heterogeneous wireless networks (HWNs) are expected to support multiple radio access technologies (RATs), dynamic wireless environments, and applications with diverse quality-of-service (QoS) requirements. In such environments, network selection (NS) cannot rely only on instantaneous radio measurements or static ranking rules.
**Approach:** This paper proposes a large language model (LLM)-based digital twin (DT) framework for stable, application-aware RAT selection under candidate-set evolution.
**Tools and scale:** The constructed DT combines site-specific geometry, Sionna RT-based propagation descriptors, ns-3 packet-level QoS emulation, service context, candidate-RAT information, and decision memory.
**Results:** To improve decision stability, the framework further introduces history-aware adaptive normalization (HAAN) and DT-memory-driven retrieval-augmented in-context learning (RA--ICL). Numerical results show that the proposed framework reduces rank-reversal problem and unnecessary handover events, while improving service-aware QoS satisfaction compared with representative MADM-based NS baselines.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
