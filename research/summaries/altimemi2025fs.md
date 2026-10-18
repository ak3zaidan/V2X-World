# FS-CLAS: A Forward-Secure Pairing-Free Certificateless Aggregate Signature Scheme for VANET Authentication
Jalal M.H. Altimemi, Iman A. Almarzooq, Nada Mohammed Hassan Moter, Shams A.S. Alfarttoosi et al. · Informatica 2025 · https://doi.org/10.31449/inf.v49i27.11696 · relevance: medium · basis: abstract

**Problem:** Overview: To accomplish forward secrecy, FS-CLAS uses a hash-chain-based design for key evolution that does not require bilinear pairings or secure hardware modules. It is secure against side-channel attacks due to the usage ofephemeral keys and constant-time scalar multiplication, and it features batch verification capability, conditional traceability, and decentralized revocation using Bloom filters.
**Approach:** We introduce a forward-secure, pairing-free, and certificateless aggregate signature scheme called FSCLAS to support efficient, lightweight, and scalable authentication in VANETs. We demonstrate through experimental evaluation that FS-CLAS achieves up to 50% reduction in verification latency and 42% saving of communication overhead as compared to more recent work, where the average signing and verifying times are ...
**Tools and scale:** A modular app and management architecture make our implementation compatible with real-world Intelligent Transportation System (ITS)deployments.
**Results:** Results from simulations with SUMO and NS-3 show that they work well under variable traffic density levels and high vehicle mobility. FS-CLAS complies with IEEE 1609.2 and ETSI ITS-G5 standards, providing an option for deployment with existing and future V2X networks.
**Limitations and future work (as stated by the authors):**
- FS-CLAS complies with IEEE 1609.2 and ETSI ITS-G5 standards, providing an option for deployment with existing and future V2X networks.
**Overlap with our engine:** none — cryptographic/authentication scheme or analysis without the closed-loop simulation, SCMS backend or counterfactual machinery of our engine.
**Novelty implications:** No effect on our novelty claims. 
