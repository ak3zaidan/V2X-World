# A Zero-Knowledge Signature Framework for Efficient Post-Quantum Message Authentication in Cooperative Automated Driving
Takahito Yoshizawa, Aysajan Abidin, Edoardo Pena-Gonzalez, Bart Preneel A · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2610.11490 · relevance: high · basis: full text

**Problem:** Large PQC keys and signatures make certificate-based V2X authentication of high-frequency CAMs unscalable.
**Approach:** ZKS-PQC: replace full PQ public keys and signatures in messages with a compact zero-knowledge proof (Bulletproofs) while staying compatible with certificate trust (ETSI TS 103 097) and legacy ECDSA; supports mixed-fleet migration.
**Tools and scale:** liboqs and Bulletproofs; NIST PQC and additional-round candidates; one commercial OBU development platform.
**Results:** Measured message-size reductions (exact figures not in extracted text); notes possible practical migration constraints on commercial OBU hardware.
**Limitations and future work (as stated by the authors):**
- Experiments on a single vendor's development platform; cannot generalise to other OBUs.
- Hybrid mode (ECDSA + PQC in certificate and message) has significant size implications.
**Overlap with our engine:** strong — PQ/hybrid message authentication size cost and mixed-fleet migration for CAMs, which is direction 2.
**Novelty implications:** Size-reduction techniques for PQ V2X authentication exist; we cannot claim novelty for hybrid-mode message-size analysis alone; open: network and safety impact at scale of these variants.
