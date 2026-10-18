# Post-quantum Cryptography for Connected and Cooperative Automated Mobility: A Comprehensive Overview
Mohamed Saied Mohamed, Julie Godard, Víctor Jiménez, Adrien Jousse et al. · Lecture notes in mobility 2025 · https://doi.org/10.1007/978-3-032-06763-0_105 · relevance: high · basis: full text

**Problem:** CCAM applications rely on quantum-vulnerable public-key cryptography.
**Approach:** Overview of PQC status in automotive with performance tests of Kyber + Dilithium/Falcon over OQS OpenSSL TLS 1.3 between two Raspberry Pi 4 devices; use cases SOTA and V2X.
**Tools and scale:** Two Raspberry Pi 4 (4 cores, 8 GB); packets 1 KB, 1 MB, 30 MB.
**Results:** For 1 KB, request-to-receipt latency about 0.072-0.077 s with Dilithium and 0.092-0.100 s with Falcon; CPU 2.9-4.1%, RAM about 1.1-1.6 MB.
**Limitations and future work (as stated by the authors):**
- Notes PQC is still under research and standardisation; further integration recommendations in conclusion.
**Overlap with our engine:** partial — PQ TLS measurements relevant to our PQ provisioning over TLS.
**Novelty implications:** Small testbed; no network-scale or SCMS evaluation.
