# Attack Analysis and Enhanced Authentication Protocol Design for Vehicle Networks
Weizheng Wang, Qipeng Xie, Yongzhi Huang, Yong Ding et al. · IEEE Transactions on Dependable and Secure Computing 2025 · https://doi.org/10.1109/tdsc.2025.3593599 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad-hoc Networks (VANETs) face significant security and privacy challenges in modern intelligent transportation systems. This paper analyzes vulnerabilities in Al-Shareeda et al.'s vehicle authentication protocol (doi: 10.1109/TDSC.2025.3553868) and proposes an enhanced ECC-based scheme using short-lived pseudonymous certificates.
**Approach:** We identify two critical weaknesses in Al-Shareeda et al.'s protocol—a desynchronization attack causing potential denial-of-service and an identity linking attack compromising vehicle privacy.
**Tools and scale:** Not stated in abstract.
**Results:** Performance analysis shows our approach achieves an optimal security-efficiency balance with competitive communication overhead (4608 bits) and computation costs (5.02 ms) compared to state-of-the-art alternatives, while uniquely satisfying all twelve evaluated security properties.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — Pseudonymous-certificate authentication protocol with attack analysis; touches our pseudonym/PKI stack only at protocol-design level.
**Novelty implications:** Does not rule out our candidate directions; any claim about this specific attack/detection/trust mechanism must cite it. Left open: end-to-end detection-report-revocation evaluation with safety outcomes.
