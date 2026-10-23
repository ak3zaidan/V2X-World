# VeReMi Extension: A Dataset for Comparable Evaluation of Misbehavior Detection in VANETs
Joseph Kamel, Michael Wolf, Rens W. van der Heijden, Arnaud Kaiser et al. · ICC 2020 - 2020 IEEE International Conference on Communications (ICC) 2020 · https://doi.org/10.1109/icc40277.2020.9149132 · relevance: high · basis: full text (HAL hal-02492739)

**Problem:** MBD results are hard to compare; original VeReMi lacked sensor error models and attack variety.
**Approach:** F2MD on Veins/SUMO, LuST subset of 1.61 km² (peak 67.4 veh/km²). Sensor error on position, speed, acceleration and heading (§IV-B). **Position error is a near-constant per-vehicle bias: EP₀ = U([−5,5]) m, μ = (EP₀+EPₜ₋₁)/2, σ = 0.03·EP₀.** 19 malfunction/attack types, including four Sybil variants. Attackers are labelled only while actively attacking (§II). 30% penetration in every simulation (§IV-D).
**Tools and scale:** 39 subsets (07–09 h, 14–16 h, MixAll 0–24 h); JSON with `sender` and `senderPseudo`; ground-truth file joined by `messageID`; Zenodo 20090854 (CC BY 4.0).
**Results:** Plausibility checks plus a threshold fusion: F1 ranges from 0.51 (DataReplaySybil) to 0.9995 (DoSRandom); replay attacks lower precision by implicating the replayed genuine vehicles (Table III, §V).
**Limitations and future work (as stated by the authors):**
- Future work: a misbehaviour-reports dataset for global MA evaluation; tests on field deployments in France and Germany (§VI).
- Reaction "is still not well defined in the literature or the published standards" (§III-B).
**Overlap with our engine:** strong — the dominant MBD benchmark; open-loop; no RSSI, PKI or impact labels.
**Novelty implications:** Muzammil (muzammil2026code) measures pseudonyms that encode the sender, benign vehicles that never rotate, identity prevalence of 68–98% and count-only AUC above 0.99 on six of the eight Sybil archives. The smoothed position-error model makes the 1 m gpsDrift check unreachable for honest vehicles. That supports Paper 1, see synthesis/check-datasets-2026.md §4.
