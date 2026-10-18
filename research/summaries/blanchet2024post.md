# Post-Quantum Sound CryptoVerif and Verification of Hybrid TLS and SSH Key-Exchanges
Bruno Blanchet, Charlie Jacomme · 2024 IEEE 37th Computer Security Foundations Symposium (CSF) 2024 · https://doi.org/10.1109/csf61375.2024.00050 · relevance: low · basis: abstract

**Problem:** A core issue is that we need to update the existing tools used to verify security protocols, as classical security proofs do not always carry over to quantum attackers.
**Approach:** In this work, we prove the post-quantum soundness of the CryptoVerif prover, a tool used to semi-automatically obtain computational security guarantees over cryptographic constructions. We also had to validate the soundness of all its proof techniques, the so-called game transformations.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
