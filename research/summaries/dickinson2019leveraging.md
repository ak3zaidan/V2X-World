# Leveraging locality of reference for certificate revocation
Luke Dickinson, Trevor Smith, Kent Seamons · Proceedings of the 35th Annual Computer Security Applications Conference 2019 · https://doi.org/10.1145/3359789.3359819 · relevance: low · basis: abstract

**Problem:** X.509 certificate revocation defends against man-in-the-middle attacks involving a compromised certificate.
**Approach:** We propose Certificate Revocation Table (CRT), a new revocation strategy that is competitive with or exceeds alternative state-of-the-art solutions in effectiveness, efficiency, certificate growth scalability, mass revocation event scalability, revocation timeliness, privacy, and deployment requirements.
**Tools and scale:** To validate both the effectiveness and efficiency of our approach, we simulated a CRT using 60 days of TLS traffic logs from Brigham Young University to measure the effects of actively refreshing revocation status information for various certificate working set window lengths.
**Results:** A working set window size of 45 days resulted in an average of 99.86% of the TLS handshakes having revocation information cached in advance.
**Limitations and future work (as stated by the authors):**
- Certificate revocation strategies face scalability, effectiveness, and deployment challenges as HTTPS adoption rates have soared.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
