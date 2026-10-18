# Benchmarking Post-Quantum TLS: Performance, Security, and Deployment Trade-offs of Hybrid Key Exchange and Authentication
Hridoy Roy · venue not stated 2026 · https://doi.org/10.2139/ssrn.7444042 · relevance: low · basis: abstract

**Problem:** Post-quantum migration in TLS introduces two distinct engineering problems: replacing classical key exchange with a quantum-resistant or hybrid mechanism, and replacing classical certificate authentication with post-quantum signatures. This study benchmarks those dimensions separately and jointly using TLS 1.3 configurations based on X25519, X25519MLKEM768, ECDSA P-256, and ...
**Approach:** This study benchmarks those dimensions separately and jointly using TLS 1.3 configurations based on X25519, X25519MLKEM768, ECDSA P-256, and ML-DSA-65. Median latency increased from 0.66595 ms for X25519/ECDSA to 0.87666 ms with hybrid X25519MLKEM768 key exchange (31.6%), 1.52414 ms with ML-DSA-65 authentication (128.9%), and 1.62282 ms when both mechanisms were combined (143.7%).
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The benchmark reports N=300 fresh handshakes per configuration and provides aggregate median and mean latency, p95 and p99 latency, server CPU time per successful handshake, TLS-record bytes through Finished, ClientHello size, and certificate DER size. The hybrid key exchange increased the ClientHello from 188 to 1,364 bytes (625.5%), while the measured ML-DSA-65 certificate was 5,521 bytes versus 396 bytes for ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
