# A Hybrid Post-Quantum Cryptographic Framework Integrating Kyber-512 and ASCON for Secure IoT Communications
Liyth H. Mahdi, Alharith A. Abdullah · Engineering, Technology &amp; Applied Science Research 2025 · https://doi.org/10.48084/etasr.12471 · relevance: low · basis: abstract

**Problem:** The growing dependence on the Internet of Things (IoT) across different sectors requires stringent security solutions to ensure data confidentiality and integrity.
**Approach:** This article presents a hybrid cryptographic framework that combines Kyber-512, a post-quantum key encapsulation method, with ASCON, a lightweight authenticated encryption method, to improve IoT security. The proposed method includes key generation, encryption, secure data transmission, decryption with minimal computational load, and the ability to exhibit good security attributes.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Performance analysis through extensive experimentation reveals that the proposed framework is efficient in terms of encryption time (43 ms), has minimal memory usage (2.56 KB), and provides optimal CPU usage (21.62%) and power savings (20.76 W), while ensuring high ciphertext entropy (7.90). The incorporation of Kyber-512 offers post-quantum security, reducing the threat of quantum-based attacks while being practically usable for devices with resource constraints.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
