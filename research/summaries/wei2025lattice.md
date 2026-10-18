# Lattice-Based Certificateless Proxy Re-Signature for IoT: A Computation-and-Storage Optimized Post-Quantum Scheme
Zhan-Zhen Wei, Gongjian Lan, Hong Zhao, Zhao-Bin Li et al. · Italian National Conference on Sensors 2025 · https://doi.org/10.3390/s25154848 · relevance: low · basis: abstract

**Problem:** However, most existing proxy re-signature schemes, which are predominantly based on traditional public-key cryptosystems, face security vulnerabilities and certificate management bottlenecks.
**Approach:** We propose a novel and practical proxy re-signature scheme that utilizes the Dilithium algorithm. Performance evaluation and comparative analysis demonstrate that our proposed scheme significantly reduces the computational overhead compared to existing algebraic lattice-based schemes.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Furthermore, it substantially optimizes the signature storage size compared to mainstream NTRU-based solutions, achieving a 52.9% reduction in storage space. The considerable reduction in computational overhead positions our proposed scheme as a highly attractive option for resource-constrained environments, such as Internet of Things (IoT) terminals.
**Limitations and future work (as stated by the authors):**
- "Certificateless schemes effectively resolve both certificate management and key escrow problems but remain vulnerable to quantum computing threats."
- "To address these limitations, this paper constructs an efficient post-quantum certificateless proxy re-signature scheme based on algebraic lattices."
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
