# Quantum Safe Public Key Infrastructure: Hybrid Classical PQC Certificate Chains and Migration Framework for Enterprise TLS
Pavan Navandar · International Journal of Engineering &amp; Extended Technologies Research 2024 · https://doi.org/10.15662/ijeetr.2024.0604014 · relevance: low · basis: abstract

**Problem:** The imminent threat of cryptographically relevant quantum computers (CRQCs) to RSA and elliptic curve cryptography — combined with the 'harvest now, decrypt later' attack paradigm — necessitates an urgent enterprise PKI migration to post quantum cryptographic (PQC) algorithms.
**Approach:** This paper presents a comprehensive hybrid PKI framework enabling phased, backward compatible migration from classical to post quantum certificate chains. We provide Algorithm 2 (HybridCertIssue) specifying the complete certificate generation and verification protocol, and a migration roadmap structured across five phases from 2021 to 2035+.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The framework embeds dual signatures (ECDSA + CRYSTALS Dilithium3) within X.509v3 certificates following the emerging RFC 9480 composite certificate standard. Experimental evaluation on a 10,000 node enterprise PKI demonstrates: TLS handshake overhead of +4.6 KB, latency increase of +38ms (LAN p50), and Dilithium3 signing throughput of 800 ops/sec on FIPS 140 3 HSMs.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
