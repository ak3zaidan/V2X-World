# "PQ-V2X Original Paper Benchmark: A Quantum-Resilient V2X IDS Dataset"
Shimaa Abdelnaby AbdelHakeem, Hyung Won Kim · IEEE DataPort 2026 · https://doi.org/10.21227/830s-zh96 · relevance: high · basis: abstract

**Problem:** No dataset for IDS under classical, post-quantum and hybrid attack conditions.
**Approach:** PQ-V2X benchmark: 5,000,000 records, 55 columns, 24 classes (1 normal, 23 attack) with mobility, channel, crypto, protocol attributes and provenance. (summarised from the abstract only)
**Tools and scale:** About 10.054% of records from cryptographic execution traces; rest from simulation and synthetic pipeline.
**Results:** 3,006,500 normal, 894,700 PQ attack, 615,100 hybrid/quantum-inspired, 483,700 classical attack records.
**Limitations and future work (as stated by the authors):**
- Mostly synthetic data (stated composition).
**Overlap with our engine:** partial — a labelled V2X security dataset with crypto attributes, adjacent to direction 3's benchmark.
**Novelty implications:** A PQ-flavoured V2X IDS dataset exists; it lacks counterfactual per-unit ground truth and traffic-safety outcomes, which remain open.
