# VANET TRAFFIC SIMULATION FOR BLACKHOLE ATTACK DETECTION USING AODV ROUTING PROTOCOL
Ahmad Yusri Dak, Musfira Mohd Azmir, Rafiza Ruslan et al. · Journal of Information System and Technology Management 2025 · https://doi.org/10.35631/jistm.1038009 · relevance: medium · basis: abstract

**Problem:** Vehicular Ad-hoc Networks (VANETs) play a pivotal role in modern intelligent transportation systems by enabling seamless Vehicle-to-Vehicle (V2V) and Vehicle-to-Infrastructure (V2I) communication.
**Approach:** However, their dynamic and decentralized nature exposes them to security vulnerabilities, particularly Blackhole attacks, where malicious nodes disrupt network operations by advertising false routes and dropping packets.
**Tools and scale:** Simulations were conducted in NS-2 with BonnMotion mobility models, varying node densities (20–60 nodes) within a 1000×1000 m² area over 140 seconds to emulate urban traffic congestion.
**Results:** Results revealed severe network degradation during attacks: EED surged by 63.43% (from baseline 175.05 ms), PLR exceeded 80% consistently, and PDR plummeted drastically (e.g., from 99.78% to 10.01% for 60 nodes). Throughput declined by up to 85% (e.g., 46.94 Kbps to 6.84 Kbps for 60 nodes), while Goodput exhibited similar deterioration due to malicious packet drops.
**Limitations and future work (as stated by the authors):**
- None stated in the abstract.
**Overlap with our engine:** partial — addresses misbehaviour/attack detection (direction 1 area), but per the abstract without an integrated SCMS backend, closed-loop revocation, per-message joint ground truth or counterfactual reruns.
**Novelty implications:** Does not rule out any candidate direction on the evidence of the abstract. Leaves open quantifying this in a closed loop with real SCMS revocation and traffic safety outcomes.
