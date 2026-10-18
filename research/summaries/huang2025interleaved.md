# Interleaved Pseudonym Distribution for Linkability Reduction in VANETs
Keyao Huang · KTH Publication Database DiVA (KTH Royal Institute of Technology) 2025 · https://openalex.org/W7128281058 · relevance: high · basis: abstract

**Problem:** Vehicular Ad-hoc Networks (VANETs) play a pivotal role in the evolution of Intelligent Transportation Systems (ITS), enabling seamless communication among vehicles and infrastructure.
**Approach:** Despite the advantages of vehicle public key infrastructures (VPKI) for secure communication, existing pseudonym-based privacy mechanisms in the system with multiple Pseudonym Certificate Authority (PCA) face significant challenges in mitigating pseudonym link attacks, particularly those exploiting PCAID. This thesis proposes two novel schemes, Interleaved Pseudonym Distribution (IPD) and Random Interleaved Pseudonym Distribution (RIPD) to address this issue and enhance vehicle privacy.
**Tools and scale:** Not stated in the available text unless given above (topic: pseudonym change/privacy).
**Results:** These two schemes utilize multiple PCAs with distinct signing keys and distributing interleaved pseudonyms across these PCAs, the scheme reduces the probability of pseudonym linkage, even in scenarios with adversarial tracking algorithms that utilize PCAID signatures. The experimental results demonstrate that RIPD effectively counters PCAID-based pseudonym link attacks and significantly increases the level of pseudonym confusion degree within the...
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** strong — pseudonym distribution across multiple PCAs to reduce linkability; overlaps multi-provider SCMS and cross-layer linkability
**Novelty implications:** Prior work on this exact mechanism; we cannot claim it as new. Open: integration with full radio/traffic stack, per-message ground truth, and safety outcomes in closed loop.
