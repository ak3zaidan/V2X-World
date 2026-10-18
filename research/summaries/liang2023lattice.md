# A Lattice-Based Certificateless Traceable Ring Signature Scheme
Junbin Liang, Jianye Huang, Qiong Huang, Liantao Lan et al. · Inf. 2023 · https://doi.org/10.3390/info14030160 · relevance: low · basis: abstract

**Problem:** Up until that point, traceable ring signatures have been secure based on the difficult problem of number-theoretic (discrete logarithms or RSA), but since the advent of quantum computers, traditional traceable ring signatures may no longer be secure.
**Approach:** Thus Feng proposed a lattice based TRS, which are resistant to attacks by quantum computers. To close this gap, a quantum-resistant certificateless TRS scheme was proposed in the study.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Additionally, a better zero-knowledge protocol is used to improve the computational efficiency of the scheme, and by reducing the soundness error of the zero-knowledge protocol, the number of runs of the zero-knowledge protocol is reduced, so that the communication overhead of the scheme is reduced.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
