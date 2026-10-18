# RAT: Retrieval-Augmented Testing of Certificate Revocation List Parsers in TLS Implementations
Jin Liu, Xu Lu, Cong Tian, Chu Chen et al. · Proc. ACM Softw. Eng. 2026 · https://doi.org/10.1145/3808212 · relevance: low · basis: abstract

**Problem:** Transport Layer Security (TLS) implementations form the backbone of countless software systems, spanning web browsers, email clients, cloud services, and Internet of Things software, by enabling secure authentication and encrypted communication.
**Approach:** To address this gap, we introduce Retrieval-Augmented Testing (RAT), a framework powered by Large Language Models (LLMs), to systematically evaluate CRL parsers in mainstream TLS implementations such as OpenSSL, GnuTLS, and wolfSSL. Our work makes the following contributions: (1) Unlike prior work focusing on certificate validation, this is the first study to systematically assess CRL parsers; (2) We propose RAT, a novel testing framework that leverages an LLM to integrate insights from retrieved bug reports and RFC 5280, enabling automated test-case generation; and (3) We have implemented an open-source prototype of RAT and experiments uncovered 23 new bugs, features, enhancements, fixes in commits, and x509s, demonstrating RAT's potential to strengthen the reliability and security of CRL parsers in TLS implementations.
**Tools and scale:** Not stated in the abstract.
**Results:** By analyzing these normalized results, RAT detects discrepancies and uncovers latent risks in CRL parsers.
**Limitations and future work (as stated by the authors):**
- "Despite the CRL's critical role, CRL parsers, which decode CRL data for validation, remain overlooked in security research, exposing TLS-dependent software to potential threats."
**Overlap with our engine:** partial — shares topic terms (revocation, crl, certificate) with the engine, but the abstract gives no indication of an integrated traffic-radio-PKI simulation, closed-loop revocation, or counterfactual reruns.
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.
