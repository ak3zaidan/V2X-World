# Implementing a Distributed Certificate Authority Using Elliptic Curve Cryptography for Big Data Environment
Omar Salah F. Shareef, Ali Makki Sagheer · 2020 2nd Annual International Conference on Information and Sciences (AiCIS) 2020 · https://doi.org/10.1109/aicis51645.2020.00031 · relevance: low · basis: abstract

**Problem:** Improving security is necessary to secure the exchange of confidential documents and protection against unauthorized accesses in the big data environment.
**Approach:** Based on Elliptic Curve Cryptography (ECC), a Distributed Certificate Authority (DCA) scheme has been designed and implemented which can efficiently improve the authentication of big data in a distributed environment. The proposed scheme is demonstrated on a big dataset of social networks.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Each group has its own revocation and signature list instead of each node, and this way reduces system overhead and avoids consuming much time when verifying the nodes without a centralized server. The experiment shows that the proposed scheme is capable of consumes less memory and reduces time consumption.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
