# A performance evaluation framework for post-quantum TLS
José A. Montenegro, Ruben Rios, Javier López-Cerezo · Future generations computer systems 2025 · https://doi.org/10.1016/j.future.2025.108062 · relevance: low · basis: abstract

**Problem:** Quantum computers pose a significant threat to widely used cryptographic schemes, making it crucial to urgently shift to post-quantum protocols that ensure the long-term security of communications.
**Approach:** This paper presents a framework for facilitating the transition to a quantum-resistant web by enabling the evaluation of the impact of integrating post-quantum and hybrid cryptographic primitives in the TLS protocol. The proposed framework is used to analyze different TLS configurations involving classic, hybrid and post-quantum primitives, with a focus on those selected by NIST.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
