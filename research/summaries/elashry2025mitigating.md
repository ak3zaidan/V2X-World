# Mitigating DDoS Threats in VANETs: A Real-Time Simulation Approach Using SUMO and NS-3
Abdelgwad Elashry, Ahmed Elasmar, Yasmeen Khalifa et al. · Novel Intelligent and Leading Emerging Sciences Conference (NILES) 2025 · https://doi.org/10.1109/niles68063.2025.11231899 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad Hoc Networks (VANETs) play a crucial role in Intelligent Transportation Systems (ITS) by enabling communication between vehicles (V2V) and infrastructure (V2I).
**Approach:** This paper proposes a multi-layered defense framework that integrates an Intrusion Prevention System (IPS), Trust Score System, Token Bucket mechanism, and Blacklist System.
**Tools and scale:** The framework is evaluated using a realistic co-simulation environment combining NS-3, SUMO, and TraCI, simulating urban VANET scenarios with flooding-based DDoS attacks.
**Results:** The system detects malicious traffic with less than 1-second latency, maintains a false positive rate below 0.1%, and restores service within 5 seconds, achieving over 95% rerouting success for legitimate vehicles. Comparative analysis with existing works highlights improvements in detection accuracy, response speed, and mitigation breadth.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area); security overhead or cryptography (direction 2 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
