# Challenges of DNS in the Post-Quantum Era: Improving Security with Post-Quantum TLS
Abdullah Aydeger, Sanzida Hoque, Engin Zeydan · Híradástechnika/Infocommunications journal 2025 · https://doi.org/10.36244/icj.2025.3.2 · relevance: low · basis: abstract

**Problem:** The Domain Name System (DNS), an important component of the Internet infrastructure, is vulnerable to various attacks that can jeopardize the security and privacy of Internet communications.
**Approach:** In this paper, we propose a comprehensive framework for DNS over Post-Quantum TLS (DoPQT) to address this challenge. We introduce a hybrid key exchange mechanism and post-quantum authentication procedures to protect the confidentiality, integrity, and authenticity of DNS traffic.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** While DNS over TLS (DoT) is widely used to improve DNS security, the advent of quantum computing poses a significant threat to the underlying cryptographic algorithms used in TLS. DoPQT has the potential to offer comparable performance to existing solutions while demonstrating superior quantum resistance.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
