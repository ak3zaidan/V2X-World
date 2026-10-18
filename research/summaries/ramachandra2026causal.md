# When Does Causal Pretraining Beat Direct Small-Data Causal Learning? Towards a causal foundation model
Vikas Ramachandra · unknown venue 2026 · https://doi.org/10.21203/rs.3.rs-10549233/v1 · relevance: low · basis: abstract

**Problem:** Scientific causal inference is frequently data limited: a target study may contain only tens of randomized or quasi-experimental observations, while related domains contain much larger observational and interventional datasets.
**Approach:** We introduce SciCFM, an episodic causal pretraining framework that encodes support sets from heterogeneous structural causal models and adapts the learned mechanism representation to a target dose-response task.
**Tools and scale:** Not stated in the abstract.
**Results:** We investigate whether intervention-rich cross-task pretraining can provide a reusable causal prior that improves treatment-effect estimation in a new small-data domain. First, under changing hidden confounding, outcome-related selection, overlap restriction, measurement distortion, and nonlinear response mechanisms, causal pretraining reduced mean precision in estimation of heterogeneous effect (PEHE) by 42.8–53.8% relative to observational pretraining.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Vehicular simulation/dataset work; overlaps the engine's simulation scope in part, without the integrated SCMS/PKI and per-message ground truth.
**Novelty implications:** Low-relevance background; does not rule out any candidate direction (no SCMS linkage-based revocation, closed-loop MA, PQ SCMS migration, or counterfactual ground truth).
