# PAMPOS: Causal Transformer-based Trajectory Prediction for Attack-Agnostic Misbehavior Detection in V2X Networks
Konstantinos Kalogiannis, Ahmed Mohamed Hussain, Panos Papadimitratos · arXiv (Cornell University) 2026 · https://doi.org/10.1145/3811880.3815104 · relevance: high · basis: full text (extracted sections)

**Problem:** Supervised MDSs fail on unseen falsification attacks.
**Approach:** PAMPOS: causal transformer-decoder trained on benign VeReMi++ trajectories; top-K normalised anomaly score localises falsified kinematic features.
**Tools and scale:** VeReMi++, 19 attack types, rush-hour and afternoon scenarios.
**Results:** AUC up to 0.98, F1 up to 0.95 for most attack categories; symmetric cross-scenario generalisation.
**Limitations and future work (as stated by the authors):**
- Constant position offset (A2) and eventual stop (A9) remain undetectable.
- Future: augmented scoring, full ablation, temporal frequency features for DoS, model compression for devices.
**Overlap with our engine:** partial - attack-agnostic MBD that could run inside our closed loop; offline evaluation.
**Novelty implications:** No effect on closed-loop claims.
