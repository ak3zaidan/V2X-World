# Post-Quantum Cryptographic Migration for Consumer Industrial Control Systems
Anurag Bhattacharjee, Anjan Bandyopadhyay · venue not stated 2026 · https://doi.org/10.2139/ssrn.6715506 · relevance: low · basis: abstract

**Problem:** Consumer Industrial Control Systems (ICS) - spanning smart homes, building automation, and consumer manufacturing equipment - continue to rely on RSA and ECC-based protocols despite decade-long deployment lifecycles that extend well into the post-quantum era. The harvest-now-decrypt-later (HNDL) threat, wherein adversaries archive ciphertext today for decryption once ...
**Approach:** We present an adaptive hybrid protocol that combines ML-KEM (NIST FIPS 203) with classical ECDH, selecting parameter sets dynamically from per-device capability profiles. We further provide a practical migration roadmap comprising a device capability taxonomy, risk-prioritised deployment guidance, and vendor-actionable cost–benefit analysis.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Over 5,000 TLS handshakes conducted on representative consumer IoT hardware - Raspberry Pi gateways and ESP-class microcontrollers - our adaptive negotiation reduces handshake overhead by 23–31% compared with static hybrid baselines while preserving full backward compatibility with classical-only peers.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
