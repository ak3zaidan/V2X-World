# A Lightweight Post-Quantum Anonymous Attestation Framework for Traceable and Comprehensive Privacy Preservation in VANETs
Esti Rahmawati Agustina, Kalamullah Ramli, Ruki Harwahyu, Teddy Surya Gunawan et al. · Journal of Cybersecurity and Privacy 2026 · https://doi.org/10.3390/jcp6020044 · relevance: high · basis: full text

**Problem:** VANET authentication must balance privacy, scalability and post-quantum security; lattice DAA (V-LDAA) has large proofs and weak traceability.
**Approach:** PQ-TDAA: Dilithium2/Falcon-512 with Beullens-style blind signatures and Schnorr-style proofs, ETSI TS 102 941-compliant traceability via RSU-assisted verification; hybrid ML-KEM-512 + AES-256 KEM-DEM; manages pseudonym issuance, update and revocation.
**Tools and scale:** SageMath, Python 3.11, NS-3 with IEEE 802.11p; 10-100 vehicles; Raspberry Pi 5 (ARM Cortex-A76) for OBU emulation.
**Results:** Proof size 8 kB vs 26 kB (-69.2%); Falcon-512 end-to-end delay 8.1 ms (10 veh) and 49.7 ms (20 veh); 64.7 Mbps vs 24.95 Mbps goodput (Falcon vs Dilithium) at N=100; sub-0.5 ms V2V cycles on Pi 5; viable for <=50 vehicles. Notes IP fragmentation all-or-nothing loss degrades performance.
**Limitations and future work (as stated by the authors):**
- Future work: standardisation for 5G NR-V2X.
- Future work: NS-3/SUMO-coupled realistic traffic scenarios (current evaluation lacks them).
- Future work: side-channel countermeasures for production.
- Scales only to about 50 vehicles.
**Overlap with our engine:** partial — PQ signature size vs 802.11p channel load and fragmentation, pseudonym lifecycle; we do this at city scale with real traffic.
**Novelty implications:** Signature compactness dominating PQ channel performance is already shown; open: city-scale, traffic-coupled PQ evaluation, which the authors list as future work.
