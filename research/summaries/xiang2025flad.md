# FLAD: Federated Learning for LLM-based Autonomous Driving in Vehicle-Edge-Cloud Networks
Tianao Xiang, Mingjian Zhi, Yuanguo Bi et al. · arXiv 2025 · https://arxiv.org/abs/2511.09025 · relevance: low · basis: abstract

**Problem:** Large Language Models (LLMs) have impressive data fusion and reasoning capabilities for autonomous driving (AD).
**Approach:** We present Federated LLM-based Autonomous Driving (FLAD), an FL framework that leverages distributed multimodal sensory data across AVs in heterogeneous environment. FLAD has three key innovations: (1) a cloud-edge-vehicle collaborative architecture that reduces communication delay and preserving data privacy; (2) an intelligent parallelized collaborative training with a communication scheduling mechanism that optimizes training efficiency, leveraging end-devices otherwise having insufficient resources for model training; and (3) a knowledge distillation method that personalizes LLM according to heterogeneous edge data.
**Tools and scale:** Not stated in the abstract.
**Results:** In addition, we prototype FLAD in a testbed with NVIDIA Jetsons, overcoming practical implementation challenges including CPU/GPU memory sharing in resource-constrained devices, dynamic model partitions, and fault-tolerant training.Extensive experimental evaluation demonstrates that FLAD achieves superior end-to-end AD performance while efficiently utilizing distributed vehicular resources, opening up new possibilities for future collaborative AD model training and knowledge sharing.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Security/authentication scheme for vehicular networks; overlaps the 1609.2/SCMS layer in topic but not the US SCMS design (butterfly keys, LA/MA, linkage-based CRL) or simulation-integrated evaluation.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
