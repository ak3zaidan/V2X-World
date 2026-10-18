# Great X: A Unified Multi-Modal Simulator Bridging the Sim2Real Gap for 6G
Yuan Gao, Kongwu Huang, Jun Jiang, Shiyi Mu et al. · arXiv 2025 · https://arxiv.org/abs/2507.08716 · relevance: medium · basis: abstract

**Problem:** Multi-platform simulators suffer timing misalignment and Sim2Real gap.
**Approach:** Great-X single-engine Unreal simulator with native ray tracing and deterministic fixed-step clock across CSI, RGB, depth, LiDAR, radar.
**Tools and scale:** Great-MCD: over three million synchronised samples; UAVs; urban and rural.
**Results:** CIRs consistent with (truncated).
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — single-engine deterministic multi-modal simulator with synchronised ground truth, like our design philosophy, but for sensing/radio not PKI/V2X protocols.
**Novelty implications:** Single-engine deterministic co-simulation is not unique; per-message joint traffic-radio-PKI ground truth remains ours.
