# A Security Benchmarking Approach for Cooperative Driving Automation (CDA) Applications
Mateen Malik, Behrooz Sangchoolie, Johan Karlsson · Mobile Networks and Applications 2025 · https://doi.org/10.1007/s11036-025-02460-x · relevance: high · basis: full text

**Problem:** Lack of repeatable, comparable security benchmarks for cooperative driving automation (CDA).
**Approach:** Reference model for security benchmarks: tests exposing a system under test to specific attacks while recording responses; benchmark for CACC algorithms against barrage jamming.
**Tools and scale:** System simulation (Plexe-type CACC algorithms: Ploeg, Flatbed, Consensus, PATH-type) with barrage jamming varying noise power.
**Results:** Severe outcomes vary by algorithm, e.g., one controller 1475 (41.3%) and Flatbed 1764 (49.3%) severe outcomes vs Ploeg 9 (0.3%) and Consensus 0; consensus needs larger spacing; combining V2V, sensing and fallback is best.
**Limitations and future work (as stated by the authors):**
- Consensus reduces traffic density and relies on faultable sensors
- benchmark covers jamming against CACC only.
**Overlap with our engine:** strong — strong: a simulation-based, repeatable security benchmark of cooperative driving applications with recorded safety outcomes — the same framing as our benchmark direction (direction 3) and safety-under-attack measurement.
**Novelty implications:** We cannot claim the first simulation-based V2X security benchmark for cooperative driving; open: PKI-layer attacks, per-message ground truth, counterfactual pairs, city scale.
