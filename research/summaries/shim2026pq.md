# PQ-Derived Schnorr: Post-Quantum Hybrid Signature Scheme for Bitcoin-Compatible Networks
Sophia Shim, Caleb Lee · venue not stated 2026 · https://doi.org/10.2139/ssrn.6492738 · relevance: low · basis: abstract

**Problem:** We present PQ-Derived Schnorr, a novel hybrid signature scheme that combines postquantum security with classical Bitcoin compatibility. Our approach leverages the CRYSTALS-Dilithium lattice-based signature scheme as the primary security foundation while maintaining full compatibility with existing Schnorr signature verification systems.
**Approach:** This design ensures quantum resistance while preserving seamless integration with Bitcoin's Taproot and other Schnorr-based systems.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** While the total signature size is approximately 4.4 KB, the scheme is structurally optimized so that only a 64-byte Schnorr-compatible segment is required for verification on-chain in blockchain environments such as Bitcoin. Performance analysis shows signing times of 0.02-0.04 seconds, making it practical for real-world deployment.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
