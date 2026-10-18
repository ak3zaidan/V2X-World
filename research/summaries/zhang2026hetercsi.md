# HeterCSI: Channel-Adaptive Heterogeneous CSI Pretraining Framework for Generalized Wireless Foundation Models
Chenyu Zhang, Xinchen Lyu, Chenshan Ren, Shuhan Liu et al. · arXiv 2026 · https://arxiv.org/abs/2601.18200 · relevance: medium · basis: abstract

**Problem:** Wireless foundation models promise transformative capabilities for channel state information (CSI) processing across diverse 6G network applications, yet face fundamental challenges due to the inherent dual heterogeneity of CSI across both scale and scenario dimensions. However, current pretraining approaches either constrain inputs to fixed dimensions or isolate training by scale, limiting the generalization and scalability of wireless foundation models.
**Approach:** In this paper, we propose HeterCSI, a channel-adaptive pretraining framework that reconciles training efficiency with robust cross-scenario generalization via a new understanding of gradient dynamics in heterogeneous CSI pretraining. Specifically, we formulate heterogeneous CSI batch construction as a partitioning optimization problem that minimizes zero-padding overhead while preserving scenario diversity.
**Tools and scale:** Mentioned in abstract: 6G. (From the abstract only.)
**Results:** Our key insight reveals that CSI scale heterogeneity primarily causes destructive gradient interference, while scenario diversity actually promotes constructive gradient alignment when properly managed. Extensive experiments on 12 datasets demonstrate that HeterCSI establishes a generalized foundation model without scenario-specific finetuning, achieving superior average performance over full-shot baselines. Compared to the state-of-the-art zero-shot benchmark WiFo, it reduces NMSE by 7.19 dB, 4.08 dB, and 5.27 dB for CSI reconstruction, time-domain, and frequency-domain prediction, respectively.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (foundation/pretrained model for wireless signals/channels); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.
