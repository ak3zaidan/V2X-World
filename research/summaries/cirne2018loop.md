# loop - A Trace-based Emulator for Vehicular Ad Hoc Networks
Pedro Cirne, André Zúquete, Susana Sargento · Proceedings of 8th International Conference on Simulation and Modeling Methodologies, Technologies and Applications 2018 · https://doi.org/10.5220/0006854503910402 · relevance: high · basis: abstract

**Problem:** Need non-intrusive, deterministic comparison of VANET protocol variants on realistic data.
**Approach:** loop trace-based emulator using real vehicle locations and reception events; built to evaluate adding security features to the VANET routing control plane.
**Tools and scale:** Real traces of a city VANET with hundreds of vehicles.
**Results:** 24-hour periods evaluated in 6 to 30 minutes.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — deterministic replay to compare security variants (counterfactual-like), but trace-driven without closed-loop mobility.
**Novelty implications:** Deterministic comparison of security variants exists in trace replay; our same-seed closed-loop counterfactuals remain distinct.
