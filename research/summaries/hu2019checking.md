# Checking certificate revocation efficiently using certificate revocation guard
Qinwen Hu, Muhammad Rizwan Asghar, Nevil Brownlee · Journal of Information Security and Applications 2019 · https://doi.org/10.1016/j.jisa.2019.06.012 · relevance: low · basis: abstract

**Problem:** Abstract In the Public Key Infrastructure (PKI) model, digital certificates play a vital role in securing online communication.
**Approach:** In this article, we propose a Certificate Revocation Guard (CRG) to efficiently check certificate revocation while minimising bandwidth, latency and storage overheads.
**Tools and scale:** Not stated in the abstract.
**Results:** Compared to a naive approach (where a client checks the revocation status of all certificates in the chain on every request), CRG decreases the bandwidth overheads and network latencies by 95%. Using CRG incurs 69% lower storage overheads compared to the CRL method.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
