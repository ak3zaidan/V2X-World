# Benchmarking NIST-Standardized Post-Quantum Digital Signatures (ML-DSA-44) for Secure IoT Authentication
MD. SOFIKUL ISLAM, Md Sofikul Islam · venue not stated 2026 · https://doi.org/10.22541/au.177031479.93526108/v1 · relevance: low · basis: abstract

**Problem:** As the Internet of Things (IoT) expands to encompass billions of devices in critical sectors like smart grids, healthcare, and industrial automation, ensuring the authenticity of data is paramount. Currently, digital signatures such as RSA and ECDSA (Elliptic Curve Digital Signature Algorithm) are the bedrock of IoT authentication.
**Approach:** This paper presents a comprehensive feasibility study of ML-DSA-44 (Dilithium2), the lightweight variant of the standard. Utilizing a cross-compiled ARM64 emulation environment (QEMU) to replicate modern IoT gateway architecture, we benchmarked the critical operations of Key Generation, Signing, and Verification.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Our empirical results reveal a verification latency of just 0.507 milliseconds and a signing latency of 1.869 milliseconds.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: post-quantum/hybrid cryptography outside V2X. It gives background for direction 2 (PQ migration of the SCMS) but does not model V2X channels, SCMS issuance, or safety impact.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
