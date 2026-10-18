# Post-Quantum Identity-Based TLS for 5G Service-Based Architecture and Cloud-Native Infrastructure
V. Rathi, Lakshya Chopra, N. Rajput · arXiv.org 2026 · https://arxiv.org/abs/2602.04238 · relevance: low · basis: abstract

**Problem:** Cloud-native application platforms and latency-sensitive systems such as 5G Core networks rely heavily on certificate-based Public Key Infrastructure (PKI) and mutual TLS to secure service-to-service communication.
**Approach:** While effective, this model introduces significant operational and performance overhead, which is further amplified in the post-quantum setting due to large certificates and expensive signature verification. In this paper, we present a certificate-free authentication framework for private distributed systems based on post-quantum Identity-Based Encryption(IBE).
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** We apply this framework to cloud-native application deployments and latency-sensitive 5G Core networks. In particular, we demonstrate how identity-based TLS integrates with the 5G Service-Based Architecture while preserving security semantics and 3GPP requirements, and we show how the same architecture can replace private PKI in Kubernetes, including its control plane, without disrupting existing trust domains or deployment models.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
