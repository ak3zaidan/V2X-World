# VeReMi: A Dataset for Comparable Evaluation of Misbehavior Detection in VANETs
Rens W. van der Heijden, Thomas Lukaseder, Frank Kargl · Lecture Notes of the Institute for Computer Sciences, Social Informatics and Telecommunications Engineering 2018 · https://doi.org/10.1007/978-3-030-01701-9_18 · relevance: high · basis: full text

**Problem:** No reference dataset for comparable misbehaviour detection evaluation.
**Approach:** VeReMi: public, extensible dataset generated with Veins/SUMO (LuST) of BSM-like beacons with position-falsification attacks, plus metric discussion; evaluates ART, SAW, SSC, DMV detectors via the Maat framework.
**Tools and scale:** Veins + SUMO (Luxembourg scenario); 802.11p.
**Results:** Acceptance range threshold and simple speed check detect different attacks and are complementary; precision-recall recommended over other metrics.
**Limitations and future work (as stated by the authors):**
- Metrics need more study from a security perspective; PR curves do not give the full picture.
- A dataset has an inherent limitation: attacker and benign deviations are unknowable in advance (e.g. urban GPS errors, adaptive attackers).
**Overlap with our engine:** strong — a simulation-generated MBD benchmark with attack ground truth, which our engine and direction 3's benchmark also provide.
**Novelty implications:** Simulation-generated MBD datasets exist (VeReMi, and extensions); our benchmark must add closed-loop reaction, PKI/radio joint ground truth and per-unit counterfactuals to be novel.
