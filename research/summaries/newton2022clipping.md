# Clipping for Faster Wireless Network Simulation in ns-3
Benjamin Newton, Michael Scoggin, Anand Ganti, Uzoma Onunkwo, Vincent Hietala · Proceedings of the 2022 Workshop on ns-3 2022 · https://doi.org/10.1145/3532577.3532578 · relevance: low · basis: abstract

**Problem:** In this work we present the concept of ‘clipping’, scheduling receive events for wireless transmissions only on receivers within some distance of the transmitter.
**Approach:** Combined with spatial indexing, this technique enables faster simulation of large-scale wireless networks containing tens of thousands or even hundreds of thousands of wireless nodes. We detail our additions and changes to ns-3 to implement this feature, demonstrate how it yields a 2 × speedup for a complex 5G scenario with minimal impact on simulation fidelity, and show how under special circumstances a speedup of over 40 × is achievable while producing identical results.
**Tools and scale:** Mentioned in abstract: 5G, ns-3, simulation.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — touches network/traffic simulation but not the joint traffic/radio/SCMS closed loop.
**Novelty implications:** Nothing in our candidate directions is ruled out; useful only as background on network/traffic simulation.
