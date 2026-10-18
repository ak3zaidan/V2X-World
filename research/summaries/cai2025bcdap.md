# BCDAP-DGS: Dynamic Group Signature and Batch Cross-Domain Authentication Protocol for Intelligent Transportation
Chuan-Da Cai, Changgen Peng, You-Liang Tian, Wei-Jie Tan et al. · IEEE transactions on intelligent transportation systems (Print) 2025 · https://doi.org/10.1109/tits.2025.3593412 · relevance: low · basis: abstract

**Problem:** However, the highly dynamic and open nature of the IoV poses severe security challenges in cross-domain scenarios, mainly due to the lack of trust relationships between different domains, making it difficult to achieve efficient and secure cross-domain authentication(CDA).
**Approach:** This paper proposes a Batch CDA Protocol based on Dynamic Group Signatures (BCDAP-DGS) to address these issues. The proposed protocol incorporates non-interactive zero-knowledge (NIZK) proofs to achieve secure identity verification without requiring additional data exchange.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** In addition, an online/offline authentication framework is designed by incorporating vehicle location information to precompute related parameters, thereby significantly improving CDA efficiency. A formal security analysis is conducted under the random oracle model, demonstrating that the proposed protocol satisfies essential security properties, including anonymity, non-frameability, unforgeability, and traceability.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
