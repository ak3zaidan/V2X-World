# HyGra: Accelerating Network-State Simulation for LLM Training in DCNs via Adaptive Packet-Flow Granularity
Wenyi Wang, Zheng Wu, Yanmeng Wang, Haolin Mao et al. · arXiv 2026 · https://arxiv.org/abs/2603.12671 · relevance: medium · basis: abstract

**Problem:** In recent years, large language models (LLMs) have driven substantial intelligent transformation across diverse industries. Commercial LLM training is typically performed over data center networks (DCNs) comprising hundreds to thousands of GPUs, with multiple devices collocated per node.
**Approach:** In this paper, we develop \texttt{HyGra}, a hybrid-granularity network-state simulator that exploits intrinsic network dynamics in LLM training to adaptively switch simulation granularity.
**Tools and scale:** Network-state simulators therefore play a crucial role by enabling cost-effective evaluation of network configurations and parallelization strategies through faithful emulation of DCN dynamics during LLM training.
**Results:** Experiments based representative commercial LLM workloads, including ChatGPT, DeepSeek, and Qwen, show that \texttt{HyGra} achieves up to 15.4$\times$ speedup under single parallelization strategy and 7.8$\times$ under hybrid parallelization strategies while maintaining high ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — LLM agents for network/security operations; our engine has an LLM agent that configures and analyses runs, but this work is not V2X and not tied to a deterministic simulator with ground truth.
**Novelty implications:** LLM agents for SOC/NetOps/RCA are established; cross-layer V2X root-cause analysis against simulator ground truth is not covered here.
