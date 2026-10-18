# Poster: Revocation in VANETs Based on k-ary Huffman Trees
Francisco Martín-Fernández, Pino Caballero-Gil, Cándido Caballero-Gil · SmartObjects@MobiCom 2022 · https://doi.org/10.1145/2797044.2797051 · relevance: high · basis: full text (abstract, conclusion/future-work sections)

**Problem:** One of the biggest problems of vehicular ad-hoc networks is revocation.
**Approach:** The efficient management of such issue has become one of the major paradigms in this area of research. A solution proposed here is based on the use of authenticated data structures like revocation trees to replace the classical and inefficient certificate revocation lists.
**Tools and scale:** Not stated in the available text unless given above (topic: CRL/revocation).
**Results:** In particular, the idea of this paper is to propose the use of k-ary hash trees, Huffman coding and a duplex version of the SHA-3 hash function, to optimize insertions and searches in the revocation structure. Therefore, the proposal described here improves both the insertion of new revoked certificates in the revocation structure and the search of revoked certificates in the revocation structure.
**Limitations and future work (as stated by the authors):**
- "in the future. This is possible because the hash tree allows authentication for access con- trol but network communication is assumed to be encrypted with session keys."
**Overlap with our engine:** partial — revocation/CRL distribution, which our SCMS models (CRL, RSU broadcast)
**Novelty implications:** Does not affect novelty claims of the candidate directions beyond background in CRL/revocation.
