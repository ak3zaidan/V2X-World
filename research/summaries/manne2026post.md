# Post-Quantum Cryptography Migration Framework for Real-Time Payment Gateways
Vimal Teja Manne · venue not stated 2026 · https://doi.org/10.36227/techrxiv.177004925.56741305/v1 · relevance: low · basis: abstract

**Problem:** Real time payment gateways handle authorization and clearing traffic for card and account based ecosystems under strict latency and availability targets. Most deployed gateways still rely on RSA and elliptic curve cryptography, which become vulnerable once large scale quantum computers can run Shor's algorithm.
**Approach:** This paper presents a Post Quantum Cryptography Migration Framework that gives an end to end architectural view of gateway, acquirer, and issuer communication paths. The framework introduces a crypto abstraction layer with pluggable algorithm providers, supports hybrid classical plus post quantum modes during transition, and uses policy driven selection of key encapsulation and signature schemes such as Kyber and ...
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The experiments show that, with careful configuration and connection management, post quantum protection can be integrated into real time payment processing with performance overhead kept within operational limits.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
