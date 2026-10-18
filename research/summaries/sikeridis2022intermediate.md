# Intermediate certificate suppression in post-quantum TLS: an approximate membership querying approach
Dimitrios Sikeridis, Sean Huntley, David E. Ott, M. Devetsikiotis · IACR Cryptology ePrint Archive 2022 · https://doi.org/10.1145/3555050.3569127 · relevance: low · basis: abstract

**Problem:** We showcase the extend of the PQ authentication overhead challenge in TLS, and evaluate the feasibility of AMQ filters for ICA suppression in terms of space and computational overhead.
**Approach:** Unfortunately, authentication algorithms based on the new post-quantum (PQ) cryptography create significant performance bottlenecks for TLS due to larger certificate chains which introduce additional packets and round-trips. In this paper, we propose a novel framework for Intermediate Certificate Authority (ICA) certificate suppression in TLS that reduces the authentication message size and prevents excessive round-trip delays.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Finally, we experimentally evaluate the potential gains form our approach and showcase a 70% reduction in exchanged ICA cert data that translates to 15--50 MB of savings in PQ TLS and for certain Web-based application scenarios.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
