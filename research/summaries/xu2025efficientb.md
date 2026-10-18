# Efficient and Secure Post-Quantum Certificateless Signcryption With Linkability for IoMT
Shi-Yuan Xu, Xue Chen, Yu Guo, Siu-Ming Yiu et al. · IEEE Transactions on Information Forensics and Security 2025 · https://doi.org/10.1109/tifs.2024.3520007 · relevance: low · basis: abstract

**Problem:** Nevertheless, the sensitive data involved in IoMT raises concerns regarding user validation and data privacy.
**Approach:** Therefore, research focusing on designing an efficient post-quantum CLSC scheme is still far-reaching. In this work, we propose PQ-CLSCL, a novel post-quantum CLSC scheme with linkability for IoMT.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We also formalize and prove the security of our design, including indistinguishability against chosen-ciphertext attacks (IND-CCA2), existential unforgeability against chosen-message attacks (EU-CMA), and linkability. Finally, through comprehensive performance evaluation, our computation overhead is just 5% of other existing schemes.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
