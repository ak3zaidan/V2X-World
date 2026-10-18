# An Efficient and Provably Secure Identity Authentication Scheme for VANET
Yanwei Zhou, Zhao-Long Wang, Zirui Qiao, Bo Yang et al. · IEEE Internet of Things Journal 2023 · https://doi.org/10.1109/jiot.2023.3273234 · relevance: low · basis: abstract

**Problem:** In recent years, many researchers have applied aggregated signature techniques to resource-constrained vehicular ad hoc networks (VANETs) authentication scenarios.
**Approach:** We reviewed two recent VANET authentication schemes based on certificateless aggregated signatures (CLASs) while demonstrating that neither of them is resistant to public key replacement attacks under their security models. To further address the above security flaws, we propose an improved CLAS scheme for VANET.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Considering that some researchers have briefly or even incorrectly used forking lemmas in their security proofs to prove an insecure CLAS scheme, we further improve and refine the security model and security proof method for CLAS, which make the proof process transparent by using general forking lemma. Based on these improvements, our scheme can resist attacks from two types of adversaries in the CLAS security model.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
