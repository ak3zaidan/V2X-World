# Post-Quantum Key Exchange in TLS 1.3: Further Analysis on Performance of New Cryptographic Standards
Konstantina Souvatzidaki, Konstantinos Limniotis · Cryptography 2025 · https://doi.org/10.3390/cryptography9040073 · relevance: low · basis: abstract

**Problem:** As conventional key exchange mechanisms will become increasingly vulnerable in the post-quantum era, the integration of post-quantum cryptographic (PQC) algorithms into existing security protocols is of utmost importance.
**Approach:** The findings offer useful insights into the performance trade-offs introduced by PQC integration and further highlight the necessity of a timely transition to post-quantum cryptographic standards.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** This study investigates the impact of incorporating PQC key encapsulation mechanisms—specifically, the recent standards CRYSTALS-Kyber and HQC, in conjunction with the candidate standard BIKE—into the TLS 1.3 handshake.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
