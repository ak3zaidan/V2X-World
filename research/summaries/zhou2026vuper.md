# VUPER: Verified ASN.1 UPER Parser
Xiaotian Zhou, Kai Tu, Ali Ranjbar, Yilu Dong et al. · arXiv 2026 · https://arxiv.org/abs/2608.09094 · relevance: low · basis: abstract

**Problem:** ASN.1 is a widely used interface description language, and UPER (Unaligned Packed Encoding Rules) is one of its key encoding rules, particularly popular in security-critical domains such as cellular networks and vehicle-to-everything (V2X) communication.
**Approach:** To ensure the correctness and security of this foundational infrastructure, we present VUPER, a framework for generating verified ASN.1 UPER parsers. Finally, we develop a dynamic testing framework using the VUPER parser as a test oracle.
**Tools and scale:** Mentions: 5g (details not in abstract).
**Results:** VUPER uncovers 20 types of inconsistencies in popular parsers and demonstrates stricter compliance with ASN.1 UPER standards. Additionally, we demonstrate concrete attacks by exploiting these parser vulnerabilities.
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
