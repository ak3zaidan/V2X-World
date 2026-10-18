# Deployment Feasibility Analysis of Post-Quantum Digital Signatures in Safety-Critical C-V2X Communication for Urban Mobility Scenario
Akid Abrar, Sagar Dasgupta, Md. Abdullah Al Mamun, Minhaj Uddin Ahmad et al. · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2608.05087 · relevance: high · basis: full text

**Problem:** PQ signatures enlarge C-V2X sidelink payloads; which fit the SAE J3161 profile and what is the communication cost?
**Approach:** Transport-block feasibility analysis with 1609.2 secured messages and J3161 radio parameters for ECDSA P-256, Falcon-512, Dilithium-2, SPHINCS+; full-stack C-V2X PC5 Mode 4 co-simulation of Falcon-512 vs ECDSA.
**Tools and scale:** Full-stack C-V2X Mode 4 co-simulation; 24 scenarios, six traffic levels of service, LOS/NLOS; RSU receiver; urban intersection.
**Results:** Dilithium-2 and SPHINCS+ exceed transport-block capacity; Falcon-512 fits (certificate SPDU 1,735 bytes needs 7 subchannels at MCS 11; digest SPDU 741 bytes needs 4); Falcon mean latency about 52 ms, 95th percentile 97-98 ms, but meets 90% PDR only at LOS A with LOS; ECDSA meets it at more levels.
**Limitations and future work (as stated by the authors):**
- Future work: Falcon-512 on NR-V2X Rel-16/17.
- Future work: hybrid certificate schemes switching Falcon/ECDSA by congestion (would need 1609.2 and J3161 changes).
- Future work: extend J3161 MCS range; multi-intersection corridors; hardware profiling on automotive platforms.
**Overlap with our engine:** strong — PQ signatures in 1609.2 over C-V2X Mode 4 with full-stack simulation, PDR and latency vs traffic density, exactly the core of direction 2 and our LTE-V2X stack.
**Novelty implications:** We cannot claim the first full-stack PQ C-V2X PDR/latency study or the Falcon-only-fits finding; open (their future work): NR-V2X, congestion-adaptive hybrid switching, corridor scale, and safety outcomes rather than PDR thresholds, plus SCMS provisioning impact.
