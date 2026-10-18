# A Comprehensive Review of Attacks and Defense Mechanisms in Federated Learning for Autonomous Driving and VANETs
Riya Kumbhare, Vasudev Dehalwar, R. K. Pateriya · 2026 Fourth International Conference on Secure Cyber Computing and Communications (ICSCCC) 2026 · https://doi.org/10.1109/icsccc69031.2026.11600221 · relevance: medium · basis: abstract

**Problem:** Federated Learning (FL) enables privacy preserving collaborative model training for autonomous driving and Vehicular Ad Hoc Networks (VANETs); however, its decentralized nature introduces significant security vulnerabilities.
**Approach:** This paper presents a comprehensive review of attacks and defense mechanisms in FL-based vehicular systems. Common FL architectures and adversarial threat models are analyzed, and a structured taxonomy is proposed that encompasses data and model poisoning attacks, stealthy and persistent backdoor attacks, Sybil and jamming attacks, as well as poisoning attacks targeting collaborative intrusion detection systems.The effects of these attacks on safety critical applications, including traffic sign recognition, lane detection, steering control, and network security, are examined.
**Tools and scale:** Not stated in the abstract.
**Results:** No quantitative results stated in the abstract.
**Limitations and future work (as stated by the authors):**
- "Furthermore, state-of-the-art defense strategies such as robust aggregation, trust-based and blockchain assisted mechanisms, backdoor detection techniques, and explainable artificial intelligence (XAI)-based intrusion detection frameworks are reviewed, followed by a comparative analysis that highlig"
- "The findings indicate that while existing defenses can mitigate un-targeted poisoning attacks, targeted and stealthy attacks remain largely unresolved under non-IID data distributions and dynamic vehicular conditions."
- "Finally, key open challenges are identified, and future research directions are outlined toward secure, adaptive, and scalable FL frameworks for autonomous driving and VANET applications."
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); pseudonym privacy/linkability (runner-up area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
