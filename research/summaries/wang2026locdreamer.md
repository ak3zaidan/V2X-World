# LocDreamer: World Model-Based Learning for Joint Indoor Tracking and Anchor Scheduling
Geng Wang, Zhouyou Gu, Shenghong Li, Peng Cheng et al. · AAAI-26 Workshop Machine Learning for Wireless Communication and Networks (ML4Wireless), 2026 2026 · https://arxiv.org/abs/2602.08204 · relevance: medium · basis: abstract

**Problem:** Accurate, resource-efficient localization and tracking enables numerous location-aware services in next-generation wireless networks. However, existing machine learning-based methods often require large labeled datasets while overlooking spectrum and energy efficiencies.
**Approach:** To fill this gap, we propose LocDreamer, a world model (WM)-based framework for joint target tracking and scheduling of localization anchors.
**Tools and scale:** Experiments on a real-world indoor dataset demonstrate that LocDreamer substantially improves data efficiency and generalization, outperforming conventional Bayesian filter with random scheduling by 37% in tracking accuracy, and achieving 86% of the accuracy of same model trained directly on real ...
**Results:** These measurements enable imagination-driven training of both the tracking model and the reinforcement learning (RL)-based anchor scheduler that activates only the most informative anchors, which significantly reduce energy and signaling costs while preserving high tracking ... Experiments on a real-world indoor dataset demonstrate that LocDreamer substantially improves data efficiency and generalization, outperforming conventional Bayesian filter with random scheduling by 37% in tracking accuracy, and achieving 86% of the accuracy of same model ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
