# SPADE: SPaT Attack Detection from the Connected Vehicle's Perspective
James Di Novo, Hany Ragab, Sylvain P. Leblanc · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2609.02741 · relevance: high · basis: full text

**Problem:** No dataset for SPaT attack detection from the vehicle side.
**Approach:** Eclipse MOSAIC with runtime J2735 application-layer injection from a malicious RSU or peer: six attack classes plus benign. DoS is modelled as suppression with no network simulation. Camera confidence comes from a hand formula (eqs. 1–6). 36 configurations × 5 seeds = 180 runs, each **run once per class with layout, density and camera conditions held constant** (1,260 executions, §IV-B).
**Tools and scale:** About 1.89 M records with 40 features, 270,000 per class (forced balance); scenario-run split 70/15/15; CSV.
**Results:** Dataset and generator released; training baseline models is future work.
**Limitations and future work (as stated by the authors):**
- Closed-world label set; fixed signal timing.
- "vehicles are unable to operate on the falsified malicious messages … the decision … is simply logged", so proprioceptive data never differs between attack and benign (§VI-B).
- Plans for CARLA, hardware-in-the-loop and imbalance studies.
**Overlap with our engine:** strong — seeded, paired, labelled V2X attack runs, but receivers do not react; no PKI and no radio layer.
**Novelty implications:** The paired runs mean "same-seed attack/no-attack pairs" is **not** novel; per-unit counterfactual impact labels from reacting receivers still are. The one-class-per-run design with forced balance is an artefact to avoid. See synthesis/check-datasets-2026.md §1.5.
