# Leveraging Large Vision Model for Multi-UAV Co-perception in Low-Altitude Wireless Networks
Yunting Xu, Jiacheng Wang, Ruichen Zhang et al. · arXiv 2026 · https://arxiv.org/abs/2603.16927 · relevance: low · basis: abstract

**Problem:** Multi-uncrewed aerial vehicle (UAV) cooperative perception has emerged as a promising paradigm for diverse low-altitude economy applications, where complementary multi-view observations are leveraged to enhance perception performance via wireless communications.
**Approach:** To address these challenges, this paper proposes a communication-efficient cooperative perception framework, termed Base-Station-Helped UAV (BHU), which reduces communication overhead while enhancing perception performance. Furthermore, we develop a diffusion model-based deep reinforcement learning (DRL) algorithm to jointly select cooperative UAVs, sparsification ratios, and precoding matrices, achieving a balance between communication efficiency and perception utility.
**Tools and scale:** Mentions: reinforcement learning.
**Results:** Specifically, we employ a Top-K selection mechanism to identify the most informative pixels from UAV-captured RGB images, enabling sparsified visual transmission with reduced data volume and latency. Simulation results on the Air-Co-Pred dataset demonstrate that, compared with traditional CNN-based BEV fusion baselines, the proposed BHU framework improves perception performance by over 5% while reducing communication overhead by 85%, providing an effective solution for multi-UAV cooperative perception under resource-constrained wireless environments.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
