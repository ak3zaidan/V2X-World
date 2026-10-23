# Code and data for "What VeReMi Measures: Structural Artefacts in VANET Misbehaviour Detection Benchmarks"
Hannan Muzammil · Zenodo 2026 · https://doi.org/10.5281/zenodo.22397725 (the DOI 22397724 points to a deleted record and redirects here) · relevance: high · basis: full text (journal and conference PDFs in the package)

**Problem:** Whether VeReMi benchmarks measure what they claim; reported Sybil detection rates are 94–99.9%.
**Approach:** Measurement audit of the eight VeReMi Extension Sybil archives, plus probes of VeReMi 2018 and NextGen. MD5-verified sources, a leakage-controlled loader, 14 experiments, 21 verifiers and a claim checker in a container. One untrained RF-200 detector is run through a seven-stage protocol cascade (S0–S6); there are prevalence, jitter, intensity and beacon-rate sweeps.
**Tools and scale:** VeReMi (Zenodo 20081895), Extension (20090854), the derivative 14903687; a convenience corpus of 92 full texts.
**Results:**
- Pseudonyms encode the sender in 86.5–100% of identity pairs; a trivial decoder recovers it 87.6% of the time against 0.11% chance.
- No benign pseudonym rotation.
- Prevalence 30.0% of vehicles and 67.6–97.5% of identities.
- Ground truth lists up to 27.6× fewer identities than receivers observe.
- Message count alone reaches AUC > 0.99 on 6 of 8 archives.
- Timing-only linkage reaches AUC 1.000.
- Removing ground-truth-key aggregation is the largest effect (−0.060/−0.064).
- Average precision falls from 0.956 to 0.620 at 3% prevalence.
- VeReMi 2018 path-loss exponent 0.93; RSSI position verification AUC 0.530/0.499.
- NextGen: alias leak fixed, but attacker ids in a disjoint range (> 1,000,078), no rotation, count AUC 0.972–0.996.
- The Guven dataset is withdrawn under an authorship dispute.
**Limitations and future work (as stated by the author):**
- Sybil archives only; one classifier; two datasets of one lineage; optimistic intervals; jitter applied after the fact; prevalence floor of 3%/5%.
**Overlap with our engine:** strong — a ready-made audit (the cascade accepts any detector via a model flag) that will be run on our benchmark.
**Novelty implications:** "Artefact-free" cannot be claimed; pass this audit and publish its result. The full artefact list is in synthesis/check-datasets-2026.md §3.
