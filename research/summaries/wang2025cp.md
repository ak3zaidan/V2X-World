# CP-FREEZER: Latency Attacks against Vehicular Cooperative Perception
Chenyi Wang, Ruoyu Song, Raymond Muller, Jean-Philippe Monteuuis, Z. Berkay Celik, Jonathan Petit et al. · Proceedings of the AAAI Conference on Artificial Intelligence 2025 · https://doi.org/10.1609/aaai.v40i2.37082 · relevance: high · basis: full text

**Problem:** Cooperative perception robustness to timeliness/availability attacks is unknown.
**Approach:** CP-FREEZER latency attack injecting adversarial V2V perturbations to maximise CP computation time, handling non-differentiable preprocessing and asynchronous inputs.
**Tools and scale:** OPV2V benchmark, several SOTA CP algorithms and hardware; real-world vehicle testbed.
**Results:** End-to-end CP latency increased over 90x; per-frame >3 s with 100% success.
**Limitations and future work (as stated by the authors):**
- Future: query-based black-box attacks
- need for defences.
**Overlap with our engine:** partial — availability attack via computational load, conceptually related to verification-load attacks in runner-up NR sidelink direction.
**Novelty implications:** Computation-latency attacks on receivers shown for CP; signature-verification-load attacks are a different layer.
