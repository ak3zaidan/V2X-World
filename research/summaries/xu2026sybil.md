# Sybil-TraceGuard: Traceability-enhanced Sybil Guardian for Connected and Autonomous Vehicles Using Dynamic Semi-supervised GNN
Qian Xu, Jiaxun Zhang, Chengyue Wang, Zhenning Li · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2609.19791 · relevance: high · basis: full text

**Problem:** Sybil attackers exploit pseudonym switching; fragmented identities must be linked to the source.
**Approach:** Sybil-TraceGuard: semi-supervised spatio-temporal GNN (stream pre-screening, dynamic graph, GAT encoder, multi-scale audit) with Mean-Teacher training.
**Tools and scale:** LuST-style Luxembourg segment (Route d'Esch 1,500 m x 400 m), chronological splits, cross-road and cross-density tests.
**Results:** Strong attribution performance (exact numbers in paper).
**Limitations and future work (as stated by the authors):**
- Extend to real-world V2X traces and more complex Sybil behaviour under heterogeneous mobility and communication.
**Overlap with our engine:** partial — linking pseudonyms to source attackers relates to linkage-based revocation and cross-layer linkability.
**Novelty implications:** ML-based pseudonym-to-attacker attribution exists; contrast with cryptographic LA-based linkage is open.
