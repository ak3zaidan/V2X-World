# Efficient and Timely Revocation of V2X Credentials
Gianluca Scopelliti, Christoph Baumann, Fritz Alder, Eddy Truyen, Jan Tobias Mühlberg · Network and Distributed System Security Symposium 2024 · https://doi.org/10.14722/ndss.2024.24017 · relevance: high · basis: full text

**Problem:** Standards are vague on revocation; existing schemes have scalability and effectiveness limits.
**Approach:** Formally verified (Tamarin) self-revocation of pseudonymous credentials using a trusted processing element, no trusted time source, with a predictable upper bound on revocation time; compatible with standardisation.
**Tools and scale:** Prototype on a Kubernetes-simulated 5G-Edge V2X network with many vehicles, network malfunctions and delays.
**Results:** Formal guarantees hold in practice; small pending revocation list; low overhead; even 30-minute effective revocation time is a large improvement over passive (CRL) revocation.
**Limitations and future work (as stated by the authors):**
- Choice of Tv trades revocation speed vs resources and desynchronisation
- small Tv impractical in rural areas
- future: privacy-preserving blocking of re-joining after revocation without hurting honest vehicles that lose connectivity.
**Overlap with our engine:** strong — quantifies revocation time against passive CRL revocation under network delays, directly overlapping direction 1 (time to neutralise).
**Novelty implications:** Cannot claim first bounded revocation-time analysis; open: safety consequences of revocation delay with real SCMS linkage CRLs and traffic.
