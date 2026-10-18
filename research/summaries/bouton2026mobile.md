# Mobile Network Control with a World Model
Maxime Bouton, Ioanna Mitsioni, Simon Lindståhl, Jaeseong Jeong · 2026 IEEE/IFIP Network Operations and Management Symposium (NOMS) 2026 · https://doi.org/10.1109/noms69089.2026.11668257 · relevance: high · basis: full text (extracted sections)

**Problem:** Mobile network control needs adaptable strategies.
**Approach:** Ensemble world model learned from historical data; CEM receding-horizon controller using uncertainty; objective changeable without retraining.
**Tools and scale:** Simulated closed-loop cell sleep control; real network data.
**Results:** Pareto-dominates rule-based and RL baselines; counterfactual actions evaluated on real data.
**Limitations and future work (as stated by the authors):**
- Open-loop emulation on real data rather than live deployment.
**Overlap with our engine:** partial - world model with counterfactual action evaluation for wireless (direction 3).
**Novelty implications:** World-model control of mobile networks exists; V2X world models with security state remain open.
