# Network Impact of Post-Quantum Certificate Chain sizes on Time to First Byte in TLS Deployments
Matthew Chou, Phuong Cao · arXiv.org 2026 · https://arxiv.org/abs/2604.24869 · relevance: low · basis: abstract

**Problem:** Post-Quantum Cryptography (PQC) is a rapidly growing deployment challenge as cryptographically relevant quantum computers (CRQC) continue to advance, leaving traditional cryptographic algorithms used in X.509 vulnerable to attack. However, PQC introduces significant deployment challenges in real-world networks, with handshake sizes increasing from 5x to over 20x compared to ...
**Approach:** However, PQC introduces significant deployment challenges in real-world networks, with handshake sizes increasing from 5x to over 20x compared to classical algorithms. In this work, we evaluate the time to first byte (TTFB) under CDN-focused TLS conditions to characterize the latency cost of transitioning existing internet infrastructure to quantum-safe certificate schemes.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** We find that MTC allows for 2x-3x increase in supportable certificate chain size, whereas CDN-based optimizations yield more limited reductions, supporting up to approximately 1.6x certificate chain size increase.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
