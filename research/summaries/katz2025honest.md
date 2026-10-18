# Honest-Majority Threshold ECDSA with Batch Generation of Key-Independent Presignatures
Jonathan Katz, Antoine Urban · IACR Communications in Cryptology 2025 · https://doi.org/10.62056/a0l5wol7 · relevance: low · basis: abstract

**Problem:** Several protocols have been proposed recently for threshold ECDSA signatures, mostly in the dishonest-majority setting.
**Approach:** With this in mind, we describe an efficient protocol for actively secure, honest-majority threshold ECDSA supporting batch generation of key-independent presignatures that allow for “non-interactive” online signing; these properties are not available in existing dishonest-majority protocols.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Our protocol offers low latency and high throughput, and runs at an amortized rate of roughly 1.3 ms per presignature (after which signatures can be generated in less than 100 microseconds).
**Limitations and future work (as stated by the authors):**
- "Yet in so-called key-management networks, where a fixed set of servers share a large number of keys on behalf of multiple users, it may be reasonable to assume that a majority of the servers remain uncompromised, and in that case there may be several advantages to using an honest-majority protocol."
**Overlap with our engine:** partial — threshold/distributed signing or CA background relevant to the engine's threshold authorities (FROST, CGGMP21, Umbrella); no V2X simulation or SCMS integration stated in the abstract.
**Novelty implications:** Does not rule out simulation-level claims; it is cryptographic background. Any claim of novelty in the threshold protocol itself would need to be checked against this work.
