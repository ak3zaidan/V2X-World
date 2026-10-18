# Radio Environment Mapping with World Models for Active Measurement Control: Should Networks Dream of Optimal Control?
Jernej Hribar, Ljupcho Milosheski, Ryoichi Shinkuma · arXiv 2026 · https://doi.org/10.1109/eucnc/6gsummit68295.2026.11577352 · relevance: medium · basis: abstract

**Problem:** Radio Environment Maps (REMs) have the potential to serve as an important enabler for intelligent modeling and control in emerging AI-native 6G networks. Despite significant progress, most REM construction methods remain passive, relying on interpolation or static uncertainty models and lacking an explicit mechanism to reason about how future measurements ...
**Approach:** In this paper, we formulate REM construction as a sequential decision-making problem and propose a world-model-inspired framework for active Received Signal Strength Indicator (RSSI) map reconstruction.
**Tools and scale:** Despite significant progress, most REM construction methods remain passive, relying on interpolation or static uncertainty models and lacking an explicit mechanism to reason about how future measurements will affect reconstruction quality under a limited measurement budget.
**Results:** Experimental results on real indoor RSSI data demonstrate that the proposed method significantly outperforms Gaussian Process-based interpolation in the few-shot regime, achieving up to a fivefold reduction in Root Mean Square Error (RMSE) with the same number of measurements. These results highlight the potential of world models as a powerful paradigm for sample-efficient radio environment mapping and intelligent model-based sensing in 6G and beyond networks.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — Learned wireless/PHY model (foundation or world model); no V2X security, PKI or closed-loop traffic.
**Novelty implications:** Shows learned radio surrogates are active work; a surrogate trained on joint traffic-radio-PKI ground truth with counterfactual labels is not addressed.
