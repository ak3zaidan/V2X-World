# VPKIaaS
Hamid Noroozi, Mohammad Khodaei, Panos Papadimitratos · ACM Conference on Security and Privacy in Wireless and Mobile Networks (WiSec) 2018 · https://doi.org/10.1145/3212480.3226100 · relevance: high · basis: full text

**Problem:** Availability and scalability of VPKI under failures or depletion attacks.
**Approach:** VPKIaaS: highly-available, auto-scaling VPKI on Google Cloud.
**Tools and scale:** Full implementation on GCP; pseudonym lifetimes 0.5-10 min.
**Results:** Cost-effective large-scale deployment; average end-to-end pseudonym acquisition latency below about 300 ms across lifetimes (from figure).
**Limitations and future work (as stated by the authors):**
- None stated in the full text.
**Overlap with our engine:** strong — VPKI backend performance under load and pseudonym lifetime overlaps our SCMS backend and provisioning.
**Novelty implications:** Cloud VPKI scaling measured; coupling backend load with radio/traffic simulation remains open.
