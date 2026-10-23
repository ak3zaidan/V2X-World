# BSMAttacker: Simulation-to-Field Evaluation of Safety-Application-Directed V2X Misbehavior Detection
Junjie Wu, Benjamin C. M. Fung, Hanbo Yu, Natalia Stakhanova · Zenodo (CERN European Organization for Nuclear Research) 2026 · https://doi.org/10.5281/zenodo.20754022 · relevance: high · basis: released code and README (Zenodo 23089747, v1.0.1)

**Problem:** Sim-to-field generalisation of BSM MBD and its safety-application relevance.
**Approach:** BSMAttacker: library injecting eleven safety-application attacks into benign trajectories; nine detectors trained on VeReMi Extension tested on five real V2X datasets; FCW and IMA replay.
**Tools and scale:** VeReMi Extension; Montreal, SPMD Ann Arbor, AMCD Tysons, V2AIX Aachen, BME Budapest datasets.
**Results:** Results of v1.0.0 under revision (per release notes).
**Limitations and future work (as stated by the authors):**
- Results under revision; revised release in preparation.
**Overlap with our engine:** strong — safety-application-directed MBD evaluation (FCW/IMA) and benchmarking overlaps directions 1 and 3.
**Novelty implications:** Safety-app-directed MBD benchmarking exists; closed-loop revocation and counterfactual ground truth remain open.
**Reviewer note (check-datasets-2026):** It injects attacks after the fact (an `Is_Attack` column) into real benign trajectories; four context-aware attacks fire only inside a scenario gate. It reuses the VeReMi Extension noise model. All nine detectors train only on synthetic data and are tested on real corpora without retraining. It replays FCW, IMA and EEBL on real warning events, open-loop. It is the only 2025–26 release grounded in real data. For Paper 1, its corpora (SPMD Ann Arbor, AMCD Tysons, V2AIX Aachen) can be used to measure the honest-vehicle gpsDrift trigger rate on real fixes.
