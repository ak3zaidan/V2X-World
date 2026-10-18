# Q-Auth5G: A Low-Overhead Post-Quantum Authentication Protocol for 5G-V2X Systems
Shimaa Abdelnaby AbdelHakeem, Hyungwon Kim · IEEE Internet of Things Journal 2026 · https://doi.org/10.1109/jiot.2026.3698543 · relevance: high · basis: abstract

**Problem:** Dilithium signatures and certificates add more than 7 kB, fragmenting and delaying V2X.
**Approach:** Q-Auth5G: derive Dilithium3 and Kyber768 credentials from the 5G-AKA key chain; dual-beacon (FULL rarely, REF and BSM under symmetric session key). (summarised from the abstract only)
**Tools and scale:** Urban, highway, rural, dense city-centre scenarios.
**Results:** V2I bootstrap 2.28 ms (V2V figures truncated).
**Limitations and future work (as stated by the authors):**
- Not stated in abstract.
**Overlap with our engine:** partial — PQ V2X overhead mitigation via cellular credentials, related to direction 2.
**Novelty implications:** Alternatives to per-message PQ certificates exist; SCMS-native PQ remains distinct.
