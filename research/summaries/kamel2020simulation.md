# Simulation Framework for Misbehavior Detection in Vehicular Networks
Joseph Kamel, Mohammad Raashid Ansari, Jonathan Petit, Arnaud Kaiser et al. · IEEE Transactions on Vehicular Technology 2020 · https://doi.org/10.1109/tvt.2020.2984878 · relevance: high · basis: full text

**Problem:** MBD research lacks a common simulation framework.
**Approach:** F2MD: open-source Veins-based MBD framework with attacks, plausibility/consistency detectors, misbehaviour report formats (base, beacon, evidence) and pseudonym change strategies.
**Tools and scale:** Veins/SUMO, 802.11p; open source on GitHub.
**Results:** Example scenarios demonstrate capabilities.
**Limitations and future work (as stated by the authors):**
- Future work: more pseudonym change strategies (silent periods), attacks and detectors.
- Future work: an intelligent Misbehavior Authority to evaluate global investigation, reporting policies and report format.
- Future work: colluding attackers and false report injection.
- Future work: add a PKI module.
**Overlap with our engine:** strong — in-simulation attacks, detection and misbehaviour reporting to an MA, overlapping our closed-loop MBD; their stated future work (MA, PKI) is what our engine has.
**Novelty implications:** Cannot claim the first open MBD simulation framework with reports; open (per their future work): MA decision-making, PKI revocation feedback and their effect, which direction 1 targets.
