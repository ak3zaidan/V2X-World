# Anomaly Detection in Real-Time Networks Using Asynchronous Traffic Shaping
Philipp Meyer, Teresa Lübeck, Timo Salomon et al. · IEEE Vehicular Technology Conference 2025 · https://doi.org/10.1109/vtc2025-fall65116.2025.11310112 · relevance: medium · basis: abstract

**Problem:** Connected vehicles are susceptible to safety-critical failures caused by malfunctioning components or intrusions, which calls for robust protection mechanisms of their backbone networks.
**Approach:** The IEEE Time-Sensitive Networking standard introduced Asynchronous Traffic Shaping (ATS) to guarantee deadlines for asynchronous communication in real-time networks. Extending previous work on monitoring synchronous real-time traffic, we propose an approach to detect network anomalies in real-time networks by surveying ATS.
**Tools and scale:** Not stated in the abstract.
**Results:** We show that the inherent configuration of ATS can serve as the definition of the normal behavior of individual streams.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
