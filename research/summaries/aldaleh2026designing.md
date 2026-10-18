# Designing a Quantum-Resistant PKI Framework For National ID Systems
Aseel F. Aldaleh, G. B. Satrya, Mahamed G. H. Omran · 2026 IEEE International Conference on Industry 4.0, Artificial Intelligence, and Communications Technology (IAICT) 2026 · https://doi.org/10.1109/iaict71158.2026.11620873 · relevance: low · basis: abstract

**Problem:** CRYSTALS-Dilithium signs 3.1× faster than RSA-3072 on simulated secure-element hardware—quantum resistance need not sacrifice smart-card performance. National identity systems depend on RSA and ECC certificates embedded in citizen smart cards, yet Shor’s algorithm will break both primitives once large-scale quantum computers arrive.
**Approach:** We design a hybrid migration path that pairs classical algorithms with NIST-standardised lattice-based replacements—Dilithium for signatures and Kyber for key encapsulation—inside a four-phase rollout suited to government PKI.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** CRYSTALS-Dilithium signs 3.1× faster than RSA-3072 on simulated secure-element hardware—quantum resistance need not sacrifice smart-card performance. Simulation results, calibrated against Open Quantum Safe benchmarks, show that every tested post-quantum operation completes within 5 ms on a 48 MHz secure element with 256 KB RAM.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
