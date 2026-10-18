# Battlefield 5G: Dual-PKI and TPM-Based UE Attestation for Tactical 5G Standalone Networks
Al Nahian Bin Emran, Rajendra Paudyal, Rajendra Upadhyay et al. · arXiv (Cornell University) 2026 · https://arxiv.org/abs/2608.11293 · relevance: low · basis: abstract

**Problem:** The standardized 5G Authentication and Key Agreement (5G-AKA) authenticates a subscriber credential stored on a Universal Subscriber Identity Model (USIM) but does not authenticate the physical device that holds that credential or verify its boot state.
**Approach:** We present Battlefield 5G, a pre-authentication framework for 5G Standalone networks that combines dual X.509 device-certificate checks with Trusted Platform Module (TPM) -based boot attestation before standard registration is accepted.
**Tools and scale:** We implement these capabilities by extending the Radio Access Network of the Software Radio System (srsRAN), gNB, User Equipment of the Software Radio System (srsUE) and Open5GS in a B210-based Universal Radio Peripheral (USRP) testbed with a hardware TPM 2.0 in the UE.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- The design places an outer certificate challenge on the 5G base-station called gNB, an independent inner certificate challenge on the Access and Mobility Management Function (AMF) in the 5G core network, and a TPM PCR (Platform Configuration Register) quote verified by an attestation proxy on the 5G
- A gNodeB (gNB) side Radio Resource Control (RRC) forwarding gate and an AMF-side save-and-replay mechanism enable multi-round certificate and attestation challenge-response exchanges to be inserted into the registration path without modifying any 3GPP Non-Access Stratum (NAS) message structures or a
**Overlap with our engine:** none — low-relevance adjacent work (general PKI/revocation background); no SCMS/1609.2 PKI backend, closed-loop revocation, joint ground truth or counterfactual reruns indicated.
**Novelty implications:** Does not rule out any candidate direction; background only.
