# Shaping a Quantum-Resistant Future: Strategies for Post-Quantum PKI
Grazia D’Onghia, Diana Gratiela Berbecaru, Antonio Lioy · 2024 IEEE Symposium on Computers and Communications (ISCC) 2024 · https://doi.org/10.1109/iscc61673.2024.10733624 · relevance: low · basis: abstract

**Problem:** Its application typically requires support via a public-key certificate, which is a signed data structure and must therefore face twice the quantum challenge: for the certified keys and for the signature itself.
**Approach:** We present the latest developments in selecting robust Post-Quantum algorithms and investigate their applicability in the Public Key Infrastructure context. Additionally, we explore transitioning Certificate Revocation List and Online Certificate Status Protocol to support quantum-resistant algorithms.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our contribution entails defining requirements for a secure transition to a quantum-resistant Public Key Infrastructure, with a focus on adaptations for the X.509 certificate format.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
