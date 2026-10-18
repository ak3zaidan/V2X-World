# Security Analysis and Experimental Evaluation of CAN Protocol Vulnerabilities in Mobile Robotic Systems
M. A. Khalifa, A. Azar, Walid El-Shafai · International Conference on Computer Aided Design 2026 · https://doi.org/10.1109/iccad69956.2026.11643151 · relevance: medium · basis: abstract

**Problem:** The Controller Area Network (CAN) protocol is widely deployed in mobile robots despite well-documented security weaknesses that stem from its original design assumptions of trusted and isolated environments. We demonstrate three practical attack vectors: (1) priority inversion achieving 4,248.9 pkt/s denial of service (94.3% bus utilization), (2) command injection with 90% motion override, and (3) replay attacks ...
**Approach:** This paper presents the first comprehensive security analysis of the AgileX Tracer mobile robot's CAN 2.0B implementation, focusing on the safety implications of these weaknesses in human-populated operational settings. We further discuss three defense mechanisms with increasing security guarantees: AUTOSAR SecOC, a steganographic authentication scheme that leverages unused payload bytes for zero-overhead ...
**Tools and scale:** This paper presents the first comprehensive security analysis of the AgileX Tracer mobile robot's CAN 2.0B implementation, focusing on the safety implications of these weaknesses in human-populated operational settings.
**Results:** We demonstrate three practical attack vectors: (1) priority inversion achieving 4,248.9 pkt/s denial of service (94.3% bus utilization), (2) command injection with 90% motion override, and (3) replay attacks with 100% acceptance rate. The results expose critical gaps in the security posture of mobile robots and underscore the need for authentication and long-term crypto-graphic resilience in safety-critical robotic ...
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
