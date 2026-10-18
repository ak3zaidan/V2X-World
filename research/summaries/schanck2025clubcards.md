# Clubcards for the WebPKI: Smaller Certificate Revocation Tests in Theory and Practice
John M. Schanck · 2025 IEEE Symposium on Security and Privacy (SP) 2025 · https://doi.org/10.1109/sp61157.2025.00128 · relevance: low · basis: abstract

**Problem:** CRLite is a low-bandwidth, low-latency, privacy-preserving mechanism for distributing certificate revocation data.
**Approach:** We present a novel data-structure for membership tests, which we call a clubcard, and we evaluate the encoding efficiency of clubcards using data from Mozilla's CRLite infrastructure.
**Tools and scale:** A sequence of clubcards can encode a dynamic dataset like the WebPKI revocation set.
**Results:** This is 54% smaller than the original instantiation of CRLite presented at the 2017 IEEE Symposium on Security and Privacy, and it is 21% smaller than the lower bound claimed in that work. We report on some performance aspects of our implementation, which is currently the default revocation checking mechanism in Firefox Nightly, and we propose strategies for further reducing the bandwidth requirements of CRLite.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
