# Evaluating Post-Quantum Cryptography in TLS-Based IoT Telemetry and OPC UA Industrial Communications
M. Iqbal, A. Pereda, Alessandra Pellegata, Doğancan Gemici et al. · International Conference on Transparent Optical Networks 2026 · https://doi.org/10.1109/icton71309.2026.11705248 · relevance: low · basis: abstract

**Problem:** This paper presents a system-level evaluation of post-quantum cryptography (PQC) in two representative communication scenarios: IoT telemetry over hybrid TLS 1.3 and industrial OPC UA SecureChannel with hybrid PQC protection. In the IoT use case, an ESP32-based client communicates with a wolfSSL gateway using hybrid TLS 1.3 with combined classical and post-quantum key exchange ...
**Approach:** In the OPC UA use case the asyncua stack is extended with Kyber768 and Dilithium3, retaining RSA-based X. 509 certificates for compatibility.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** In TLS, connection latency increases from 2.19 s to 4.08 s (+86.4%) and handshake size from 1.6 KB to 16.84 KB (+938.9%). In OPC UA, connection latency increases from 0.072 s to 0.133 s (+84%), handshake size from 10.63 KB to 24.84 KB (+133%), and peak memory usage from 488.6 KB to 528.4 KB (+8.1%).
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
