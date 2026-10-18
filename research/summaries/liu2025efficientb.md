# An Efficient and Adaptive Privacy-Preserving Authentication Scheme for VANETs
Weihao Liu, Yaobo Xie, Runyu Yan · ICON 2025 · https://doi.org/10.1109/icnlp65360.2025.11108481 · relevance: low · basis: abstract

**Problem:** To tackle these issues, we introduce a certificateless efficient adaptive authentication scheme that preserves privacy.
**Approach:** We adopts a certificateless mechanism to resolve the issue of key escrow. We avoids complex bilinear pairings and supports batch authentication, thereby enhancing authentication efficiency.
**Tools and scale:** Not stated in the abstract beyond the above; no traffic simulator, radio stack or SCMS-scale deployment is described.
**Results:** Additionally, pseudonyms for vehicles are generated through hash chains, effectively reducing storage overhead. The security analysis demonstrates that the proposed scheme attains existential unforgeability within the framework of the random oracle model.
**Limitations and future work (as stated by the authors):**
- "However, due to the mobility and openness of VANETs, they face a range of question, including vehicle privacy protection, message authentication, limitations in computational and storage capacity, and constrained network bandwidth."
**Overlap with our engine:** none — a standalone authentication/crypto scheme evaluated in isolation (per the abstract); no closed-loop traffic, radio and SCMS co-simulation.
**Novelty implications:** Leaves open how such schemes behave inside a full SCMS with realistic traffic and radio load; it does not rule out any candidate direction.
