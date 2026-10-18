# Toward Post-Quantum Digital Certificate for eSIM
Qaiser Khan, Sourav Purification, Rono Cheruiyot, Jinoh Kim et al. · Silicon Valley Cybersecurity Conference 2024 · https://doi.org/10.1109/svcc61185.2024.10637354 · relevance: low · basis: abstract

**Problem:** In mobile networking such as 5G, embedded SIM or eSIM does not involve a connectivity-provider-specific hardware chip to deliver the provider’s core network’s public key, in contrast to the traditional SIM. eSIM instead uses the remote digital certificate and PKI to deliver the public key.
**Approach:** We study the post-quantum transition by introducing post-quantum digital certificates for eSIM, including how the digital certificate gets used for the mutual authentication between the mobile client and the core network server. We investigate the eSIM authentication and post-quantum digital signature ciphers and outline the plan for the post-quantum design and transition in this paper.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
