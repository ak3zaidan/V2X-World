# HCASS: Hybrid Certificateless Aggregate Signcryption Scheme for Post-Quantum Migration of ICS
Dingkang Fang, Jing Zhang, Dongmei Chen, anon anon · venue not stated 2026 · https://doi.org/10.2139/ssrn.7331262 · relevance: low · basis: abstract

**Problem:** As the backbone of critical national infrastructure, Industrial Control Systems (ICS) operate with decades-long lifecycles. Coupled with the integration of the Industrial Internet of Things (IIoT), this makes their immediate migration to post-quantum cryptography essential to counter the fundamental threat posed by the rapid advancement of quantum computing.
**Approach:** However, lattice-based primitives often present challenges in meeting the sub-millisecond latency requirements of deterministic control loops. We propose HCASS, a Hybrid Certificateless Aggregate Signcryption Scheme for the post-quantum migration of ICS devices.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** During the key generation phase, we design a diagonal trapdoor structure that reduces the complexity of preimage sampling from O(k^2) to O(k), where k is the gadget dimension parameter determined by the modulus q. To ensure real-time responsiveness, HCASS utilizes an offline/online decomposition strategy, shifting 90.9% of the computation to a precomputation stage and achieving an online signcryption latency of ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
