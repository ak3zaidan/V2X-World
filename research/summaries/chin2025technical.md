# A Technical Review of FROST Threshold Signatures: Variants, Extensions, Security, and RFC9591
Ji-Jian Chin, Syh-Yuan Tan, Sook-Chin Yip, Nur Hanis Sabrina Suhaimi · 2025 3rd International Conference on Cyber Resilience (ICCR) 2025 · https://doi.org/10.1109/iccr67387.2025.11292498 · relevance: low · basis: abstract

**Problem:** Threshold signature schemes enable a group of participants to collaboratively generate a signature if a subset of t out of n participants willingly participate in the signing process. Among recent developments, FROST (Flexible Round-Optimised Schnorr Threshold signatures) stands out for its efficiency, flexibility, and strong security guarantees in both theoretical and applied ...
**Approach:** Among recent developments, FROST (Flexible Round-Optimised Schnorr Threshold signatures) stands out for its efficiency, flexibility, and strong security guarantees in both theoretical and applied settings. We examine its two- round signing protocol, distributed key generation (DKG), and integration with secure multiparty computation frameworks.
**Tools and scale:** Non-vehicular setting. Not detailed in the catalogued abstract (no integrated traffic, radio and SCMS simulation identified); see the paper for setup and scale.
**Results:** The abstract gives no specific quantitative results.
**Limitations and future work (as stated by the authors):**
- "By mapping out current research and highlighting open challenges, this survey aims to guide future work in robust, scalable threshold signature protocols anchored in the FROST paradigm."
**Overlap with our engine:** none: threshold/multi-party signatures (general, non-vehicular). Our threshold authorities (FROST, CGGMP21) build on this family of schemes; nothing here applies them to an SCMS.
**Novelty implications:** Rules out nothing specific in our candidate directions. Open: the vehicular case (SCMS scale, broadcast channel load, safety cost) is not addressed.
