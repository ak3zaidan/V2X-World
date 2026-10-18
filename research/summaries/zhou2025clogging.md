# Clogging DoS Resilient Bootstrapping of Efficient V2V Validation
Zhichao Zhou, Hongyu Jin, Panos Papadimitratos · ACM/SIGAPP Symposium on Applied Computing (SAC) 2025 · https://doi.org/10.1145/3672608.3707914 · relevance: medium · basis: abstract

**Problem:** In Vehicular Communication (VC) systems, neighboring vehicles exchange authenticated transportation safety messages, informing about own mobility and the environment.
**Approach:** Verifying all received messages in a dense neighborhood introduces significant cryptographic computation overhead for resource-constrained vehicular On-Board Units (OBUs). Attackers can exploit this to launch Denial of Service (DoS) attacks to clog OBUs by broadcasting bogus messages at a high rate.
**Tools and scale:** Not stated in the abstract.
**Results:** We show experimental results confirming our scheme can effectively mitigate unsophisticated clogging DoS attacks that do not attempt to solve puzzles. We further show our scheme also significantly raises the bar for sophisticated adversaries: it can be configured to force attackers to solve puzzles for their bogus messages actively - something possible only by investing in significantly higher (hundreds of times more) computational power than that of the targeted benign vehicles.
**Limitations and future work (as stated by the authors):**
- We address this challenge, proposing a mechanism that thwarts such attacks: puzzle-based pre-validation that prioritizes verification of potentially valid messages with yet unknown (i.e., unverified) Pseudonymous Certificates (PCs).
- Last but not least, our scheme can be adaptive while remaining compatible to standardized V2V security.
**Overlap with our engine:** partial — same area (V2X/vehicular security or networking) with a narrower, mechanism-level focus.
**Novelty implications:** Does not remove any of our claims beyond its specific mechanism; we cannot claim to be first on that mechanism. The joint closed-loop traffic/radio/PKI evaluation with counterfactual ground truth remains open as far as this abstract shows.
