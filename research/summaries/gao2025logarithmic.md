# Logarithmic NTRU-Based Certificateless Ring Signature in E-Voting Applications
Wen Gao, Tianyou Fu, Simeng Ren, Shixuan Jin et al. · Electronics 2025 · https://doi.org/10.3390/electronics14071358 · relevance: low · basis: abstract

**Problem:** At the same time, we embed the NTRU small integer solution (NTRU-SIS) problem and provide a detailed proof of security under the random oracle model (ROM).
**Approach:** In this paper, we propose a certificateless ring signature based on the special structure of the number theory research unit (NTRU) lattice, which utilizes the Merkle tree and seed tree to split commitments and integrate them again to generate signatures. In the era of big data explosion, this feature enables the proposed scheme to maintain a comparatively short signature size even when the number of ring members N is very large.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** In efficiency, the Merkle tree makes the signature size logarithmically increase with the ring scale. When N=8, the signature size is 61.08 KB; when N increases to 512, the size is 65.02 KB.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
