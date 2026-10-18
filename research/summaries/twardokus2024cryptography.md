# When Cryptography Needs a Hand: Practical Post-Quantum Authentication for V2V Communications
Geoff Twardokus, N. Bindel, Hanif Rahbari, Sarah McCarthy · Network and Distributed System Security Symposium 2024 · https://doi.org/10.14722/ndss.2024.24267 · relevance: high · basis: full text

**Problem:** PQ signatures are too large for V2V spectrum and latency limits; current certificate transmission is wasteful.
**Approach:** Learning-based adaptive scheduling of signing-certificate transmissions (found up to 93% redundant) plus a Partially Hybrid (ECDSA + PQ) authentication protocol, and a Fully Hybrid variant with explicit hybrid certificates; analysis of Dilithium, Falcon, SPHINCS+ and XMSS.
**Tools and scale:** USRP software-defined radios, commercial V2V devices, road traffic and V2V simulators (SUMO/Veins-type); provable security proofs.
**Results:** Certificate transmissions up to 93% redundant; hybrid design fits within available spectrum; first alpha messages of each certificate cycle protected only by ECDSA in the hybrid design.
**Limitations and future work (as stated by the authors):**
- First alpha messages in each certificate cycle are only ECDSA-protected (transition-period trade-off)
- embedding PQ certificate and signature in one frame is too large
- fully PQ V2V left for future after transition
**Overlap with our engine:** strong — Already integrates PQ/hybrid signatures into 1609.2-style V2V and measures channel impact with traffic and V2V simulation; overlaps direction 2 (safety cost of PQ security) directly.
**Novelty implications:** We cannot claim first PQ/hybrid V2V integration or first spectrum-impact evaluation. Open: PQ for the SCMS backend (provisioning, CRLs, butterfly keys), closed-loop safety metrics, NR/LTE sidelink, city scale.
