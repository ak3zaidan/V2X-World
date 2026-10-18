# Adaptive thresholding and neural network-based misbehavior detection for incentivizing VANET data sharing
Waqwoya Mesfin Abebe ·  2022 · https://doi.org/10.31274/td-20240329-39 · relevance: high · basis: full text

**Problem:** Incentivising BSM data sharing in VANETs while resisting collusion and location falsification.
**Approach:** Nodes co-sign each other's BSMs into handshake queues uploaded to a lightweight blockchain for token rewards, with regional dynamic thresholds; a neural network trained on VeReMi detects position falsification.
**Tools and scale:** MSc thesis (Iowa State); VeReMi dataset; Java traffic simulator; no SCMS model.
**Results:** Reports recall/precision on VeReMi high-density scenarios and notes possible overfitting when GPS positions are used as features; queue-density simulations.
**Limitations and future work (as stated by the authors):**
- Thesis lists a future-work chapter (not extracted in detail); authors flag possible overfitting from using GPS position as a feature.
**Overlap with our engine:** partial — misbehaviour detection on VeReMi; no closed-loop revocation or PKI.
**Novelty implications:** Leaves open closed-loop detection to revocation; incentive schemes are outside our scope.
