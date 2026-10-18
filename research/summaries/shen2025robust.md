# A Robust and Efficient Threshold Signature Scheme Based on ECDSA
Dongsu Shen, Yangning Zhou, Benteng Deng, Shujuan Tian et al. · 2025 IEEE International Conference on Blockchain (Blockchain) 2025 · https://doi.org/10.1109/blockchain67634.2025.00011 · relevance: low · basis: abstract

**Problem:** Threshold signatures serve as a cryptographic primitive fundamental to blockchain security and reliability. However, a significant limitation of existing ECDSA threshold signature protocols is their lack of robustness during the signing phase.
**Approach:** To address this critical issue, we present a novel robust threshold ECDSA signature scheme that guarantees uninterrupted operation even in the presence of adversarial participants.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** Security analysis demonstrates that our scheme achieves active security under the Strong RSA and Decisional Diffie-Hellman (DDH) assumptions. Compared to prior work, our solution not only ensures robustness but also reduces communication and computational overhead, making it particularly suitable for real-world blockchain deployments where both efficiency and fault tolerance are paramount.
**Limitations and future work (as stated by the authors):**
- "However, a significant limitation of existing ECDSA threshold signature protocols is their lack of robustness during the signing phase."
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
