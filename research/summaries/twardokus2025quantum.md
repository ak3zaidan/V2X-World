# Quantum-Resistant Safety Message Authentication for NextG C-V2X: A Cross-Layer Approach
Geoff Twardokus, Hanif Rahbari · IEEE Transactions on Vehicular Technology 2025 · https://doi.org/10.36227/techrxiv.175751197.77663474/v1 · relevance: high · basis: abstract

**Problem:** Elliptic-curve signatures on C-V2X safety messages are quantum-vulnerable, and post-quantum signatures/certificates do not fit LTE/NR-V2X message size, bandwidth and latency limits.
**Approach:** CHASM, a hybrid (classical + quantum-resistant) authentication protocol for C-V2X that keeps classical guarantees, fragments certificates and adds an adaptive modulation scheme (cross-layer) to carry the extra bytes. (summarised from the abstract only)
**Tools and scale:** 4G/5G C-V2X sidelink; details of the simulator and scale are truncated in the available abstract.
**Results:** The abstract states CHASM preserves V2V safety-service reliability while adding quantum resistance; specific numbers are not in the available abstract text.
**Limitations and future work (as stated by the authors):**
- Not stated in the available abstract.
**Overlap with our engine:** strong — hybrid PQ signatures on C-V2X BSMs with certificate fragmentation is part of our PQ/hybrid signature support and candidate direction 2 (safety cost of PQ migration).
**Novelty implications:** We cannot claim to be first with hybrid PQ+ECDSA BSM authentication over C-V2X or with certificate fragmentation for PQ; open: end-to-end SCMS-level PQ migration (provisioning, CRLs, linkage) measured against traffic-safety outcomes.
