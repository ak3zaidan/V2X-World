# Demo: Quantifying Trust in a Trust Assessment Framework
Artur Hermann, Nataša Trkulja, Echo Meißner et al. · IEEE Vehicular Networking Conference (VNC) 2025 · https://doi.org/10.1109/vnc64509.2025.11054127 · relevance: medium · basis: abstract

**Problem:** Vehicular communication via V2X networks increases road safety, but is vulnerable to data manipulation which can lead to serious incidents.
**Approach:** Existing security systems, such as misbehavior detection systems, have limitations in detecting and mitigating such threats.
**Tools and scale:** Not stated in the abstract.
**Results:** This interactive demonstration illustrates the quantification of trust for a smart traffic light system application. We demonstrate the impact of varying evidence coming from a misbehavior detection system and a security report generator on the trust assessment process.
**Limitations and future work (as stated by the authors):**
- "Existing security systems, such as misbehavior detection systems, have limitations in detecting and mitigating such threats."
- "To address these challenges, we have implemented a software prototype of a Trust Assessment Framework (TAF) that assesses the trustworthiness of received V2X data by integrating evidence from multiple trust sources."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
