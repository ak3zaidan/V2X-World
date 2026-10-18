# Analysis and Improvement of a Conditional Privacy-Preserving Authentication Scheme With Double-Insurance in VANETs
Jiaming Wen, Lu Bai, Hou-Zhen Wang, Jin-Hui Liu et al. · IEEE Transactions on Vehicular Technology 2024 · https://doi.org/10.1109/tvt.2024.3426097 · relevance: low · basis: abstract

**Problem:** One key aspect of them is the Conditional Privacy-Preserving Authentication (CPPA) scheme, which addresses challenges of identity authentication and message integrity during communications.
**Approach:** To tackle this issue, Xiong et al. introduced a CPPA scheme with double-insurance, claiming its security against adaptively chosen message attacks. However, this paper reveals that their scheme is universally forgeable, allowing anyone to create valid signatures on any message, rendering it inadequate for CPPA.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Besides, we prove the security under the assumptions of Discrete Logarithms (DL) and Computational Diffie-Hellman (CDH), and conduct experiments and realistic evaluations. The results demonstrate its superiority, and establish its practicality in real-world scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
