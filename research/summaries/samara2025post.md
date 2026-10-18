# Post-Quantum Tokenized Security for C-V2X
Ghassan Samara, Ibrahim Obeidat, Mais Haj Qasem, Raed Alazaidah et al. · International Arab Conference on Information Technology (ACIT) 2025 · https://doi.org/10.1109/acit68900.2025.11510686 · relevance: high · basis: abstract

**Problem:** Per-message signatures for C-V2X safety messages are costly, privacy-sensitive and quantum-vulnerable.
**Approach:** PQ-Rate: 1-RTT KEM handshake to derive AEAD session keys; unlinkable rate-limited anonymous tokens from RSU/MEC issuers via threshold VOPRF; ZK nullifiers enforce per-epoch budgets; Bloom-filter revocation digests broadcast by RSUs; threshold opening for accountability. (summarised from the abstract only)
**Tools and scale:** NS-3 with SUMO mobility; compared against pseudonym PKI and VLR group signatures; metrics include PDR, CBR, verifier throughput, revocation overhead, Sybil capacity.
**Results:** Sub-20 ms session setup; further improvements truncated in abstract.
**Limitations and future work (as stated by the authors):**
- Not stated in available abstract.
**Overlap with our engine:** strong — compares a PQ alternative against pseudonym PKI on PDR, CBR, revocation overhead and Sybil capacity in SUMO-coupled simulation, close to direction 2's safety/channel cost of PQ and our pseudonym/revocation model.
**Novelty implications:** PQ vs pseudonym-PKI comparisons on channel metrics exist; open: SCMS-faithful PQ migration with butterfly provisioning and linkage CRLs, and safety (not just PDR) outcomes.
