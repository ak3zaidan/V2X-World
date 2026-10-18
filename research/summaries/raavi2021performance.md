# Performance Characterization of Post-Quantum Digital Certificates
Manohar Raavi, Pranav Chandramouli, Simeon Wuthier, Xiaobo Zhou et al. · 2021 International Conference on Computer Communications and Networks (ICCCN) 2021 · https://doi.org/10.1109/icccn52240.2021.9522179 · relevance: low · basis: abstract

**Problem:** Public Key Infrastructure (PKI) generates and distributes digital certificates to provide the root of trust for securing digital networking systems.
**Approach:** The cryptography community is developing quantum-resistant primitives/algorithms, studying, and analyzing them for cryptanalysis and improvements. We study and analyze the feasibility and the processing performance of these algorithms in memory/size and time/speed when used for PKI, including the key generation from the PKI end entities (e.g., a HTTPS/TLS server), the signing, and the certificate generation by the certificate authority within the PKI.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** The transition to post-quantum from the classical ciphers incur changes in the parameters in the PKI, for example, Rainbow I significantly increases the certificate size by 163 times when compared with RSA 3072. Nevertheless, we learn that the current X.509 supports the NIST post-quantum digital signature ciphers and that the ciphers can be modularly adapted for PKI.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — post-quantum/hybrid authentication scheme; the engine implements PQ and hybrid signatures, but the abstract states no SCMS-scale or traffic-coupled evaluation.
**Novelty implications:** We cannot claim PQ authentication for vehicles is new; the end-to-end safety cost of PQ in a full SCMS remains open as far as this abstract shows.
