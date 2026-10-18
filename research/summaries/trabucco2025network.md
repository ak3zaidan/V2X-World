# Network-efficient authenticated pseudonym-based V2X communications with constant revocation costs
Mattia Trabucco, Giovanni Gambigliani Zoccoli, Mirco Marchetti, Luca Ferretti · IEEE International Symposium on Network Computing and Applications 2025 · https://doi.org/10.1109/nca67271.2025.00022 · relevance: high · basis: abstract

**Problem:** Linkage values in pseudonym certificates add per-message network overhead.
**Approach:** Provision vehicles with conditionally unlinkable pseudo-random key pairs via hierarchical deterministic homomorphic key derivation, removing linkage values while keeping constant-cost revocation. (summarised from the abstract only)
**Tools and scale:** Cost analysis versus explicit-certificate standards.
**Results:** Lower network overhead, no added computation for securing messages, same assumptions; higher but affordable key-management cost.
**Limitations and future work (as stated by the authors):**
- Higher computational cost for key management operations (stated).
**Overlap with our engine:** partial — alternative to SCMS butterfly keys and linkage-based revocation, both of which our engine implements.
**Novelty implications:** A drop-in comparison of this scheme against SCMS linkage revocation in our engine would be possible, but the scheme itself is prior work.
