# What If We Had Used a Different App? Reliable Counterfactual KPI Analysis in Wireless Systems
Qiu-Shuo Hou, Sangwoo Park, Matteo Zecchin, Yun-Long Cai, Guan-Ding Yu, Osvaldo Simeone · IEEE Transactions on Cognitive Communications and Networking 2024 · https://doi.org/10.1109/tccn.2025.3536793 · relevance: high · basis: full text (extracted sections)

**Problem:** In O-RAN, the KPI of an unselected app cannot be observed, so individual-level counterfactual KPI analysis is hard.
**Approach:** Conformal-prediction-based counterfactual KPI estimation with reliable error bars despite covariate shift between logged and test data.
**Tools and scale:** MAC-layer and PHY-layer app experiments.
**Results:** Demonstrated merits for both app types (numbers not extracted).
**Limitations and future work (as stated by the authors):**
- Future: app selection probability known only implicitly and must be learned.
- Generalise to distributed systems and temporal variability.
- Incorporate digital twins to augment logged data.
**Overlap with our engine:** partial - individual-level counterfactual KPI estimation in wireless (direction 3); our engine can supply exact counterfactual ground truth for such estimators.
**Novelty implications:** Cannot claim first counterfactual KPI analysis in wireless. Open: V2X counterfactuals with exact ground truth, temporal variability, and digital-twin augmentation (the authors' own future work).
